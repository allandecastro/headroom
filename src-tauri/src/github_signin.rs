//! GitHub OAuth **device flow** — the "Sign in with GitHub" path for Copilot.
//!
//! The device flow needs no client secret, so the public `client_id` ships in
//! the app and is shared by every install. We request a device code, the user
//! enters it at `verification_uri`, and we poll until GitHub mints a user
//! access token. That token is exactly what `copilot_internal/user` accepts, so
//! it replaces the manual token paste.
//!
//! The OAuth App must have **device flow enabled** (Settings → Developer
//! settings → OAuth Apps → Enable Device Flow).

use std::time::Duration;

use reqwest::{header, Client};
use serde::Deserialize;

const DEFAULT_BASE_URL: &str = "https://github.com";
/// Minimal scope: identifies the user without granting repo/write access.
const SCOPE: &str = "read:user";

/// Public OAuth App client id (device flow needs no secret, so this is safe to
/// ship and is shared by every install). The OAuth App must have device flow
/// enabled.
pub const GITHUB_CLIENT_ID: &str = "Ov23li0GQonRoripvQcM";

#[derive(Clone)]
pub struct GithubSignin {
    client: Client,
    base_url: String,
}

/// The device code the user must enter, returned to the renderer for display.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: u64,
    pub expires_in: u64,
}

impl Default for GithubSignin {
    fn default() -> Self {
        Self::build(DEFAULT_BASE_URL.to_string())
    }
}

impl GithubSignin {
    fn build(base_url: String) -> Self {
        let client = Client::builder()
            .user_agent("Headroom/0.1")
            .timeout(Duration::from_secs(15))
            .build()
            .expect("reqwest client builds");
        Self { client, base_url }
    }

    #[cfg(test)]
    pub(crate) fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::build(base_url.into())
    }

    /// Step 1: ask GitHub for a device + user code.
    pub async fn request_device_code(&self, client_id: &str) -> anyhow::Result<DeviceCode> {
        let resp = self
            .client
            .post(format!("{}/login/device/code", self.base_url))
            .header(header::ACCEPT, "application/json")
            .form(&[("client_id", client_id), ("scope", SCOPE)])
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("device code request failed: HTTP {}", resp.status());
        }

        let body: DeviceCodeResponse = resp.json().await?;
        Ok(DeviceCode {
            user_code: body.user_code,
            verification_uri: body.verification_uri,
            device_code: body.device_code,
            // GitHub may omit/zero these; fall back to documented defaults.
            interval: body.interval.filter(|&i| i > 0).unwrap_or(5),
            expires_in: body.expires_in.filter(|&e| e > 0).unwrap_or(900),
        })
    }

    /// Step 2: poll for the access token until the user authorizes, the code
    /// expires, or they deny. Honors GitHub's `slow_down` backoff.
    pub async fn poll_for_token(
        &self,
        client_id: &str,
        device: &DeviceCode,
    ) -> anyhow::Result<String> {
        let mut interval = device.interval;
        let deadline = device.expires_in;
        let mut elapsed = 0u64;

        loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            elapsed += interval;
            if elapsed >= deadline {
                anyhow::bail!("sign-in timed out — the code expired before you authorized");
            }

            let resp = self
                .client
                .post(format!("{}/login/oauth/access_token", self.base_url))
                .header(header::ACCEPT, "application/json")
                .form(&[
                    ("client_id", client_id),
                    ("device_code", device.device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await?;

            let body: TokenResponse = resp.json().await?;
            match token_outcome(&body) {
                TokenOutcome::Token(t) => return Ok(t),
                TokenOutcome::Pending => continue,
                TokenOutcome::SlowDown => {
                    interval += 5; // GitHub asks us to back off by ≥5s
                    continue;
                }
                TokenOutcome::Failed(msg) => anyhow::bail!("{msg}"),
            }
        }
    }
}

const API_BASE_URL: &str = "https://api.github.com";

/// The identity behind a GitHub token — used to key a Copilot account so several
/// accounts can coexist. `name` falls back to the login when GitHub has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubIdentity {
    pub id: String,
    pub login: String,
    pub name: String,
}

/// Resolve the GitHub identity for a token via `GET /user`. Distinct from the
/// device-flow host (`github.com`) — the user API lives on `api.github.com`.
pub async fn resolve_identity(token: &str) -> anyhow::Result<GithubIdentity> {
    resolve_identity_at(API_BASE_URL, token).await
}

async fn resolve_identity_at(base_url: &str, token: &str) -> anyhow::Result<GithubIdentity> {
    let client = Client::builder()
        .user_agent(concat!("Headroom/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(15))
        .build()?;
    let resp = client
        .get(format!("{base_url}/user"))
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!("GitHub /user failed: HTTP {}", resp.status());
    }
    let body: UserResponse = resp.json().await?;
    let login = body.login;
    Ok(GithubIdentity {
        id: body.id.to_string(),
        name: body
            .name
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| login.clone()),
        login,
    })
}

#[derive(Deserialize)]
struct UserResponse {
    id: u64,
    login: String,
    #[serde(default)]
    name: Option<String>,
}

/// Classify a token-poll response. Pure so it can be unit-tested without timing.
fn token_outcome(body: &TokenResponse) -> TokenOutcome {
    if let Some(token) = body.access_token.as_deref() {
        if !token.is_empty() {
            return TokenOutcome::Token(token.to_string());
        }
    }
    match body.error.as_deref() {
        Some("authorization_pending") => TokenOutcome::Pending,
        Some("slow_down") => TokenOutcome::SlowDown,
        Some("access_denied") => {
            TokenOutcome::Failed("sign-in was cancelled on GitHub".to_string())
        }
        Some("expired_token") => {
            TokenOutcome::Failed("the code expired before you authorized".to_string())
        }
        Some(other) => TokenOutcome::Failed(format!("GitHub rejected the sign-in: {other}")),
        None => TokenOutcome::Failed("unexpected empty response from GitHub".to_string()),
    }
}

#[derive(Debug, PartialEq)]
enum TokenOutcome {
    Token(String),
    Pending,
    SlowDown,
    Failed(String),
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    interval: Option<u64>,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn request_device_code_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "device_code": "dev123",
                "user_code": "WDJB-MJHT",
                "verification_uri": "https://github.com/login/device",
                "interval": 5,
                "expires_in": 900
            })))
            .mount(&server)
            .await;

        let signin = GithubSignin::with_base_url(server.uri());
        let code = signin.request_device_code("client").await.unwrap();
        assert_eq!(code.user_code, "WDJB-MJHT");
        assert_eq!(code.device_code, "dev123");
        assert_eq!(code.interval, 5);
    }

    #[tokio::test]
    async fn request_device_code_defaults_missing_interval() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "device_code": "d",
                "user_code": "AAAA-BBBB",
                "verification_uri": "https://github.com/login/device"
            })))
            .mount(&server)
            .await;

        let signin = GithubSignin::with_base_url(server.uri());
        let code = signin.request_device_code("client").await.unwrap();
        assert_eq!(code.interval, 5, "missing interval falls back to 5s");
        assert_eq!(code.expires_in, 900);
    }

    fn token(body: serde_json::Value) -> TokenResponse {
        serde_json::from_value(body).unwrap()
    }

    #[test]
    fn outcome_returns_token_on_success() {
        let r = token(json!({ "access_token": "gho_abc" }));
        assert_eq!(token_outcome(&r), TokenOutcome::Token("gho_abc".into()));
    }

    #[test]
    fn outcome_pending_and_slow_down() {
        assert_eq!(
            token_outcome(&token(json!({ "error": "authorization_pending" }))),
            TokenOutcome::Pending
        );
        assert_eq!(
            token_outcome(&token(json!({ "error": "slow_down" }))),
            TokenOutcome::SlowDown
        );
    }

    #[test]
    fn outcome_failures_carry_a_message() {
        assert!(matches!(
            token_outcome(&token(json!({ "error": "access_denied" }))),
            TokenOutcome::Failed(_)
        ));
        assert!(matches!(
            token_outcome(&token(json!({ "error": "expired_token" }))),
            TokenOutcome::Failed(_)
        ));
        assert!(matches!(
            token_outcome(&token(json!({}))),
            TokenOutcome::Failed(_)
        ));
    }

    #[test]
    fn outcome_treats_empty_token_as_failure() {
        assert!(matches!(
            token_outcome(&token(json!({ "access_token": "" }))),
            TokenOutcome::Failed(_)
        ));
    }

    #[tokio::test]
    async fn resolve_identity_reads_user_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 12345,
                "login": "alice",
                "name": "Alice Example"
            })))
            .mount(&server)
            .await;

        let id = resolve_identity_at(&server.uri(), "tok").await.unwrap();
        assert_eq!(id.id, "12345");
        assert_eq!(id.login, "alice");
        assert_eq!(id.name, "Alice Example");
    }

    #[tokio::test]
    async fn resolve_identity_falls_back_to_login_when_name_absent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 7,
                "login": "bob"
            })))
            .mount(&server)
            .await;

        let id = resolve_identity_at(&server.uri(), "tok").await.unwrap();
        assert_eq!(id.name, "bob");
    }
}
