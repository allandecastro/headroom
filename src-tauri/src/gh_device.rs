//! GitHub OAuth device-flow client. Used by the Copilot onboarding "Sign in
//! with GitHub" path so the user doesn't have to create + paste a PAT.
//!
//! No client secret is required: device flow uses only the public `client_id`,
//! so baking it into the binary is safe (the SPEC notes the same).

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The OAuth/GitHub App client_id for Headroom. Device flow does **not** use a
/// client secret, so this is safe to hardcode.
pub const GITHUB_CLIENT_ID: &str = "Ov23li0GQonRoripvQcM";

/// Scopes requested. `read:user` lets us fetch the authenticated user's
/// username via `GET /user` after sign-in.
const SCOPE: &str = "read:user";

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const USER_URL: &str = "https://api.github.com/user";

/// What the renderer sees: enough to display + open the verification UI.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// What the backend keeps: the same plus the secret `device_code` it uses to
/// poll. Never serialized to the renderer.
#[derive(Debug, Clone)]
pub struct DeviceCodeFull {
    pub device_code: String,
    pub public: DeviceCode,
}

#[derive(Debug, Error)]
pub enum DeviceFlowError {
    #[error("client_id is not configured (placeholder still in code)")]
    UnconfiguredClient,
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("response parse error: {0}")]
    Parse(String),
    #[error("user denied the authorization")]
    AccessDenied,
    #[error("the device code expired before the user authorized")]
    Expired,
    #[error("GitHub error: {0}")]
    Other(String),
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct UserResponse {
    login: String,
}

/// Build the reqwest client used for every device-flow call. Centralizing the
/// UA + redirect/timeout policy keeps the auth surface consistent.
fn build_client() -> Result<Client, DeviceFlowError> {
    Client::builder()
        .user_agent("Headroom/0.1 (+https://github.com/allandecastro/headroom)")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(DeviceFlowError::from)
}

/// Step 1: request a device + user code.
pub async fn start(client_id: &str) -> Result<DeviceCodeFull, DeviceFlowError> {
    if client_id.starts_with("REPLACE_") {
        return Err(DeviceFlowError::UnconfiguredClient);
    }
    let client = build_client()?;
    let resp = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", SCOPE)])
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(DeviceFlowError::Other(format!(
            "device/code returned HTTP {status}: {body}"
        )));
    }
    let parsed: DeviceCodeResponse = serde_json::from_str(&body)
        .map_err(|e| DeviceFlowError::Parse(format!("device/code: {e}")))?;
    Ok(DeviceCodeFull {
        device_code: parsed.device_code,
        public: DeviceCode {
            user_code: parsed.user_code,
            verification_uri: parsed.verification_uri,
            expires_in: parsed.expires_in,
            interval: parsed.interval,
        },
    })
}

/// Step 2: poll until the user authorizes (or the code expires/denies). Returns
/// the OAuth access token on success.
pub async fn poll_for_token(
    client_id: &str,
    device_code: &str,
    initial_interval: u64,
    expires_in: u64,
) -> Result<String, DeviceFlowError> {
    let client = build_client()?;
    let mut interval = initial_interval.max(1);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(expires_in);

    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if tokio::time::Instant::now() >= deadline {
            return Err(DeviceFlowError::Expired);
        }

        let resp = client
            .post(ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;

        let body = resp.text().await?;
        let parsed: AccessTokenResponse = serde_json::from_str(&body)
            .map_err(|e| DeviceFlowError::Parse(format!("access_token: {e}")))?;

        if let Some(token) = parsed.access_token {
            return Ok(token);
        }
        // GitHub's per-spec polling errors:
        match parsed.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval += 5;
                continue;
            }
            Some("expired_token") => return Err(DeviceFlowError::Expired),
            Some("access_denied") => return Err(DeviceFlowError::AccessDenied),
            Some(other) => return Err(DeviceFlowError::Other(other.to_string())),
            None => {
                return Err(DeviceFlowError::Parse(
                    "access_token response had neither token nor error".into(),
                ))
            }
        }
    }
}

/// Step 3 (post-auth): fetch the authenticated user's login (= GitHub username).
pub async fn fetch_username(token: &str) -> Result<String, DeviceFlowError> {
    let client = build_client()?;
    let resp = client
        .get(USER_URL)
        .header("Accept", "application/vnd.github+json")
        .bearer_auth(token)
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(DeviceFlowError::Other(format!(
            "GET /user returned HTTP {status}: {body}"
        )));
    }
    let parsed: UserResponse =
        serde_json::from_str(&body).map_err(|e| DeviceFlowError::Parse(format!("/user: {e}")))?;
    Ok(parsed.login)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn placeholder_client_id_is_rejected() {
        // start() refuses to even hit the network when the placeholder is set.
        let err = start("REPLACE_ME_NOT_REAL").await.unwrap_err();
        assert!(matches!(err, DeviceFlowError::UnconfiguredClient));
    }

    #[test]
    fn parses_a_pending_then_success_polling_response() {
        // A direct parse sanity check on the two response shapes we care about.
        let pending: AccessTokenResponse =
            serde_json::from_str(r#"{"error":"authorization_pending"}"#).unwrap();
        assert_eq!(pending.error.as_deref(), Some("authorization_pending"));
        assert!(pending.access_token.is_none());

        let ok: AccessTokenResponse = serde_json::from_str(
            r#"{"access_token":"gho_abc","token_type":"bearer","scope":"read:user"}"#,
        )
        .unwrap();
        assert_eq!(ok.access_token.as_deref(), Some("gho_abc"));
        assert!(ok.error.is_none());
    }

    #[test]
    fn parses_device_code_response() {
        let raw = r#"{
            "device_code":"abc123",
            "user_code":"WDJB-MJHT",
            "verification_uri":"https://github.com/login/device",
            "expires_in":900,
            "interval":5
        }"#;
        let parsed: DeviceCodeResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.user_code, "WDJB-MJHT");
        assert_eq!(parsed.interval, 5);
    }
}
