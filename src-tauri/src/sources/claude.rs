//! Claude Code quota source.
//!
//! Endpoint: `GET https://claude.ai/api/organizations/{orgId}/usage` with the
//! user's `sessionKey` cookie. See SPEC.md § "Data sources / Claude Code".
//!
//! This is an undocumented endpoint that mirrors what `claude.ai/settings/usage`
//! itself uses. Stable in practice but not officially supported by Anthropic.

use async_trait::async_trait;
use reqwest::{header, Client};
use serde::Deserialize;
use tracing::warn;

use super::{Quota, QuotaSource, QuotaUnit, QuotaWindow, ServiceState, ServiceStatus, SourceError};
use crate::credentials::Credentials;

const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
     (KHTML, like Gecko) Version/17.0 Safari/605.1.15";

const DEFAULT_BASE_URL: &str = "https://claude.ai";

pub struct ClaudeSource {
    client: Client,
    base_url: String,
}

impl Default for ClaudeSource {
    fn default() -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("reqwest client builds");
        Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }
}

impl ClaudeSource {
    /// Override the base URL — used only in tests to point at a [`wiremock`] server.
    #[cfg(test)]
    pub(crate) fn with_base_url(base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("reqwest client builds");
        Self {
            client,
            base_url: base_url.into(),
        }
    }

    /// Core logic: fetch usage with already-resolved credentials.
    ///
    /// Accepts an explicit `org_id` so callers (including tests) can skip the
    /// org-lookup round-trip when the ID is already known. If `org_id` is `None`
    /// the method performs the lookup itself.
    ///
    /// Extracted so tests can supply in-memory values without touching the
    /// OS keychain (which `Credentials` exclusively reads from).
    pub(crate) async fn fetch_with(
        &self,
        session_key: &str,
        org_id: Option<&str>,
    ) -> Result<ServiceStatus, SourceError> {
        let resolved_org_id = match org_id {
            Some(id) => id.to_string(),
            None => self.fetch_org_id(session_key).await?,
        };

        let url = format!(
            "{}/api/organizations/{}/usage",
            self.base_url, resolved_org_id
        );
        let response = self
            .client
            .get(&url)
            .header(header::COOKIE, format!("sessionKey={session_key}"))
            .header(header::ACCEPT, "application/json")
            .send()
            .await?;

        if response.status() == 401 || response.status() == 403 {
            let body = response.text().await.unwrap_or_default();
            if body.contains("Just a moment") || body.contains("cloudflare") {
                return Err(SourceError::CloudflareChallenge);
            }
            return Err(SourceError::AuthRejected(body));
        }

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(SourceError::HttpStatus { status, body });
        }

        let usage: UsageResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        Ok(usage.into_status())
    }

    /// Fetch the raw `/usage` JSON for diagnostics, pretty-printed, with the
    /// `sessionKey` redacted defensively. Mirrors the Copilot diagnostics path —
    /// this endpoint is undocumented too, so a raw dump is the only ground truth
    /// when the shape changes. Returns the HTTP status line on a non-200 so the
    /// reason is visible (auth/Cloudflare/etc.).
    pub(crate) async fn fetch_raw(
        &self,
        session_key: &str,
        org_id: Option<&str>,
    ) -> Result<String, SourceError> {
        let resolved_org_id = match org_id {
            Some(id) => id.to_string(),
            None => self.fetch_org_id(session_key).await?,
        };
        let url = format!(
            "{}/api/organizations/{}/usage",
            self.base_url, resolved_org_id
        );
        let response = self
            .client
            .get(&url)
            .header(header::COOKIE, format!("sessionKey={session_key}"))
            .header(header::ACCEPT, "application/json")
            .send()
            .await?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        let pretty = serde_json::from_str::<serde_json::Value>(&text)
            .and_then(|v| serde_json::to_string_pretty(&v))
            .unwrap_or(text);
        let redacted = pretty.replace(session_key, "<redacted>");
        Ok(format!(
            "GET /api/organizations/{resolved_org_id}/usage → HTTP {status}\n\n{redacted}"
        ))
    }

    async fn fetch_org_id(&self, session_key: &str) -> Result<String, SourceError> {
        let response = self
            .client
            .get(format!("{}/api/organizations", self.base_url))
            .header(header::COOKIE, format!("sessionKey={session_key}"))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SourceError::HttpStatus {
                status: response.status().as_u16(),
                body: response.text().await.unwrap_or_default(),
            });
        }

        let orgs: Vec<OrgEntry> = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        orgs.into_iter()
            .next()
            .map(|o| o.uuid)
            .ok_or_else(|| SourceError::Parse("no organizations returned".into()))
    }
}

#[async_trait]
impl QuotaSource for ClaudeSource {
    fn id(&self) -> &str {
        "claude"
    }

    fn name(&self) -> &str {
        "Claude"
    }

    async fn fetch(&self, creds: &Credentials) -> Result<ServiceStatus, SourceError> {
        let session_key = creds
            .claude_session()
            .ok_or(SourceError::MissingCredentials("claude.session"))?;

        // The org UUID is stable for the account, so resolve it once and cache it
        // in the keychain — every subsequent poll then skips the lookup round-trip.
        // Cleared on sign-out via `service_keys("claude")`.
        let org_id = match creds.claude_org_id() {
            Some(id) => id,
            None => {
                let id = self.fetch_org_id(&session_key).await?;
                if let Err(e) = creds.set("claude.orgId", &id) {
                    warn!(?e, "failed to cache claude org id");
                }
                id
            }
        };

        self.fetch_with(&session_key, Some(&org_id)).await
    }
}

#[derive(Deserialize)]
struct OrgEntry {
    uuid: String,
}

/// Shape of `GET /api/organizations/{org}/usage` (confirmed against a live
/// response). Each window reports a `utilization` percentage (0–100) and an
/// optional reset time. The endpoint exposes many windows; we surface the
/// rolling 5-hour, the overall 7-day cap, and the 7-day Opus cap (Max plans).
/// It does NOT include the plan name.
#[derive(Deserialize)]
struct UsageResponse {
    five_hour: Option<WindowEntry>,
    seven_day: Option<WindowEntry>,
    seven_day_sonnet: Option<WindowEntry>,
    seven_day_opus: Option<WindowEntry>,
    /// "Claude Design" on the usage page (opt-in via settings).
    seven_day_omelette: Option<WindowEntry>,
}

#[derive(Deserialize)]
struct WindowEntry {
    utilization: Option<f64>,
    #[serde(default)]
    resets_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl UsageResponse {
    fn into_status(self) -> ServiceStatus {
        let mut quotas = Vec::new();

        // (window kind, label, entry, is_opus) — utilization is already a
        // percentage, so used = utilization out of 100.
        let windows = [
            (
                QuotaWindow::FiveHour,
                "Current session",
                self.five_hour,
                false,
            ),
            (QuotaWindow::WeeklyAll, "Weekly · 7d", self.seven_day, false),
            (
                QuotaWindow::WeeklySonnet,
                "Sonnet · 7d",
                self.seven_day_sonnet,
                false,
            ),
            (
                QuotaWindow::WeeklyOpus,
                "Opus · 7d",
                self.seven_day_opus,
                true,
            ),
            (
                QuotaWindow::ClaudeDesign,
                "Claude Design",
                self.seven_day_omelette,
                false,
            ),
        ];

        for (window, label, entry, is_opus) in windows {
            let Some(w) = entry else { continue };
            let Some(util) = w.utilization else { continue };
            let resets_at = w.resets_at.unwrap_or_else(chrono::Utc::now);
            let mut quota = Quota::new(window, label, util, 100.0, QuotaUnit::Percent, resets_at);
            if is_opus && util >= 95.0 {
                quota = quota.with_advice("use Sonnet for the rest of the week");
            }
            quotas.push(quota);
        }

        ServiceStatus {
            id: "claude".into(),
            name: "Claude".into(),
            // The usage endpoint doesn't carry the plan name; left blank for now.
            plan: String::new(),
            state: ServiceState::Active,
            quotas,
            error_detail: None,
            copilot_usage: None,
            codex_meta: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const ORG_ID: &str = "org-abc123";

    /// Minimal usage JSON body with one quota window per type.
    fn usage_body() -> serde_json::Value {
        serde_json::json!({
            "five_hour": { "utilization": 40.0, "resets_at": "2099-01-01T00:00:00Z" },
            "seven_day": { "utilization": 60.0, "resets_at": "2099-01-07T00:00:00Z" },
            "seven_day_sonnet": { "utilization": 5.0, "resets_at": "2099-01-07T00:00:00Z" },
            "seven_day_opus": { "utilization": 20.0, "resets_at": "2099-01-07T00:00:00Z" }
        })
    }

    #[tokio::test]
    async fn successful_fetch_returns_active_status() {
        let server = MockServer::start().await;

        // Mount the usage endpoint; supply org_id directly so no org-lookup needed.
        Mock::given(method("GET"))
            .and(path(format!("/api/organizations/{ORG_ID}/usage")))
            .respond_with(ResponseTemplate::new(200).set_body_json(usage_body()))
            .mount(&server)
            .await;

        let source = ClaudeSource::with_base_url(server.uri());
        let status = source.fetch_with("sess-token", Some(ORG_ID)).await.unwrap();

        assert_eq!(status.id, "claude");
        assert!(
            matches!(status.state, ServiceState::Active),
            "expected Active state"
        );
        // The /usage endpoint carries no plan name.
        assert_eq!(status.plan, "");
        // five_hour + seven_day + seven_day_sonnet + seven_day_opus are surfaced.
        assert_eq!(status.quotas.len(), 4);
    }

    #[tokio::test]
    async fn fetch_without_org_id_performs_org_lookup() {
        let server = MockServer::start().await;

        // Org-lookup endpoint
        Mock::given(method("GET"))
            .and(path("/api/organizations"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{"uuid": ORG_ID}])),
            )
            .mount(&server)
            .await;

        // Usage endpoint resolved from lookup
        Mock::given(method("GET"))
            .and(path(format!("/api/organizations/{ORG_ID}/usage")))
            .respond_with(ResponseTemplate::new(200).set_body_json(usage_body()))
            .mount(&server)
            .await;

        let source = ClaudeSource::with_base_url(server.uri());
        let status = source.fetch_with("sess-token", None).await.unwrap();

        assert!(matches!(status.state, ServiceState::Active));
    }

    #[tokio::test]
    async fn usage_403_non_cloudflare_returns_auth_rejected() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(format!("/api/organizations/{ORG_ID}/usage")))
            .respond_with(
                ResponseTemplate::new(403).set_body_string("Forbidden — not a robot page"),
            )
            .mount(&server)
            .await;

        let source = ClaudeSource::with_base_url(server.uri());
        let err = source
            .fetch_with("bad-sess", Some(ORG_ID))
            .await
            .unwrap_err();

        assert!(
            matches!(err, SourceError::AuthRejected(_)),
            "expected AuthRejected, got {err:?}"
        );
    }

    #[tokio::test]
    async fn cloudflare_challenge_detected_via_body() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(format!("/api/organizations/{ORG_ID}/usage")))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_string("Just a moment... Checking your browser"),
            )
            .mount(&server)
            .await;

        let source = ClaudeSource::with_base_url(server.uri());
        let err = source.fetch_with("sess", Some(ORG_ID)).await.unwrap_err();

        assert!(
            matches!(err, SourceError::CloudflareChallenge),
            "expected CloudflareChallenge, got {err:?}"
        );
    }

    #[tokio::test]
    async fn cloudflare_challenge_detected_via_cloudflare_keyword() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(format!("/api/organizations/{ORG_ID}/usage")))
            .respond_with(ResponseTemplate::new(403).set_body_string("cloudflare ray id: 1234"))
            .mount(&server)
            .await;

        let source = ClaudeSource::with_base_url(server.uri());
        let err = source.fetch_with("sess", Some(ORG_ID)).await.unwrap_err();

        assert!(
            matches!(err, SourceError::CloudflareChallenge),
            "expected CloudflareChallenge, got {err:?}"
        );
    }

    #[tokio::test]
    async fn opus_advice_appears_at_95_percent() {
        let server = MockServer::start().await;

        let body = serde_json::json!({
            "seven_day_opus": { "utilization": 96.0, "resets_at": "2099-01-07T00:00:00Z" }
        });

        Mock::given(method("GET"))
            .and(path(format!("/api/organizations/{ORG_ID}/usage")))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let source = ClaudeSource::with_base_url(server.uri());
        let status = source.fetch_with("sess", Some(ORG_ID)).await.unwrap();

        let opus = status
            .quotas
            .iter()
            .find(|q| matches!(q.window, QuotaWindow::WeeklyOpus))
            .expect("WeeklyOpus quota should be present");

        assert!(
            opus.advice.is_some(),
            "advice should appear when Opus is ≥95 % used"
        );
    }
}
