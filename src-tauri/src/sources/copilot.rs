//! GitHub Copilot quota source.
//!
//! Endpoint: `GET https://api.github.com/copilot_internal/user` with a Bearer
//! GitHub token. Any classic/OAuth token works — no billing-specific
//! permission — so onboarding is a plain token paste.
//!
//! The response carries the plan, per-feature quota entitlements, and the reset
//! date directly, so caps come from the wire rather than a hardcoded table.
//! Under GitHub's token-based billing the `premium_interactions` quota is the
//! AI Credits allowance. We surface a single headline quota, preferring premium
//! interactions and falling back to whichever feature quota is actually bounded.
//!
//! See SPEC.md § "Data sources / GitHub Copilot".

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use reqwest::{header, Client};
use serde::Deserialize;

use super::{Quota, QuotaSource, QuotaUnit, QuotaWindow, ServiceState, ServiceStatus, SourceError};
use crate::credentials::Credentials;

const DEFAULT_BASE_URL: &str = "https://api.github.com";

/// Quotas we surface as the headline, most-relevant first. The first one that
/// is bounded (not unlimited, entitlement > 0) wins.
const HEADLINE_PRIORITY: [&str; 3] = ["premium_interactions", "chat", "completions"];

pub struct CopilotSource {
    client: Client,
    base_url: String,
}

impl Default for CopilotSource {
    fn default() -> Self {
        Self::build(DEFAULT_BASE_URL.to_string())
    }
}

impl CopilotSource {
    fn build(base_url: String) -> Self {
        let client = Client::builder()
            .user_agent("Headroom/0.1")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("reqwest client builds");
        Self { client, base_url }
    }

    /// Override the base URL — used only in tests to point at a [`wiremock`] server.
    #[cfg(test)]
    pub(crate) fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::build(base_url.into())
    }

    /// Core logic: fetch the Copilot user payload with an already-resolved token.
    ///
    /// Extracted so tests can supply an in-memory token without touching the OS
    /// keychain (which `Credentials` exclusively reads from).
    pub(crate) async fn fetch_with(&self, token: &str) -> Result<ServiceStatus, SourceError> {
        let url = format!("{}/copilot_internal/user", self.base_url);

        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::ACCEPT, "application/json")
            .send()
            .await?;

        if response.status() == 401 || response.status() == 403 {
            return Err(SourceError::AuthRejected(
                response.text().await.unwrap_or_default(),
            ));
        }

        if !response.status().is_success() {
            return Err(SourceError::HttpStatus {
                status: response.status().as_u16(),
                body: response.text().await.unwrap_or_default(),
            });
        }

        let body: UserResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let now = Utc::now();
        let resets_at = parse_reset(body.quota_reset_date_utc.as_deref(), now);

        let quotas = headline_quota(&body.quota_snapshots)
            .map(|(id, snap)| {
                let used = (snap.entitlement - snap.quota_remaining).max(0.0);
                Quota::new(
                    QuotaWindow::Monthly,
                    quota_label(id),
                    used,
                    snap.entitlement,
                    QuotaUnit::Requests,
                    resets_at,
                )
            })
            .into_iter()
            .collect();

        Ok(ServiceStatus {
            id: "copilot".into(),
            name: "GitHub Copilot".into(),
            plan: plan_label(&body.copilot_plan),
            state: ServiceState::Active,
            quotas,
            error_detail: None,
        })
    }
}

#[async_trait]
impl QuotaSource for CopilotSource {
    fn id(&self) -> &'static str {
        "copilot"
    }

    fn name(&self) -> &'static str {
        "GitHub Copilot"
    }

    async fn fetch(&self, creds: &Credentials) -> Result<ServiceStatus, SourceError> {
        let token = creds
            .copilot_token()
            .ok_or(SourceError::MissingCredentials("copilot.token"))?;
        self.fetch_with(&token).await
    }
}

/// Pick the headline quota: the first bounded entry in [`HEADLINE_PRIORITY`],
/// else any other bounded quota (deterministic by id). `None` when every quota
/// is unlimited or has no allowance.
fn headline_quota(snapshots: &HashMap<String, QuotaSnapshot>) -> Option<(&str, &QuotaSnapshot)> {
    for id in HEADLINE_PRIORITY {
        if let Some(snap) = snapshots.get(id) {
            if snap.is_bounded() {
                return Some((id, snap));
            }
        }
    }
    snapshots
        .iter()
        .filter(|(_, snap)| snap.is_bounded())
        .min_by(|a, b| a.0.cmp(b.0))
        .map(|(id, snap)| (id.as_str(), snap))
}

/// User-facing label for a Copilot quota. `premium_interactions` is AI Credits
/// under token-based billing.
fn quota_label(quota_id: &str) -> String {
    match quota_id {
        "premium_interactions" => "AI Credits".to_string(),
        "chat" => "Chat".to_string(),
        "completions" => "Completions".to_string(),
        other => titlecase(other),
    }
}

/// User-facing plan name. Keeps the brand casing for the common tiers and
/// title-cases anything else GitHub returns (e.g. `individual`, `business`).
fn plan_label(plan: &str) -> String {
    match plan {
        "" => "—".to_string(),
        "pro" => "Pro".to_string(),
        "pro_plus" => "Pro+".to_string(),
        other => titlecase(other),
    }
}

/// Title-case a snake/kebab/space separated identifier: `free_limited` → `Free Limited`.
fn titlecase(s: &str) -> String {
    s.split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse the API's RFC 3339 reset timestamp, falling back to the start of next
/// month if it's missing or unparseable.
fn parse_reset(reset_utc: Option<&str>, now: DateTime<Utc>) -> DateTime<Utc> {
    reset_utc
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| next_month_start_utc(now))
}

fn next_month_start_utc(now: DateTime<Utc>) -> DateTime<Utc> {
    let (year, month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    chrono::NaiveDate::from_ymd_opt(year, month, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc())
        .unwrap_or(now)
}

#[derive(Deserialize)]
struct UserResponse {
    #[serde(default)]
    copilot_plan: String,
    #[serde(default)]
    quota_snapshots: HashMap<String, QuotaSnapshot>,
    #[serde(default)]
    quota_reset_date_utc: Option<String>,
}

#[derive(Deserialize)]
struct QuotaSnapshot {
    #[serde(default)]
    entitlement: f64,
    #[serde(default)]
    quota_remaining: f64,
    #[serde(default)]
    unlimited: bool,
}

impl QuotaSnapshot {
    /// A quota worth showing as a budget: a finite, non-zero allowance.
    fn is_bounded(&self) -> bool {
        !self.unlimited && self.entitlement > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn serve(body: serde_json::Value) -> (MockServer, ServiceStatus) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/copilot_internal/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let source = CopilotSource::with_base_url(server.uri());
        let status = source.fetch_with("tok").await.unwrap();
        (server, status)
    }

    fn snap(entitlement: f64, remaining: f64, unlimited: bool) -> serde_json::Value {
        json!({
            "entitlement": entitlement,
            "quota_remaining": remaining,
            "unlimited": unlimited,
        })
    }

    #[tokio::test]
    async fn premium_interactions_is_the_headline() {
        let (_s, status) = serve(json!({
            "copilot_plan": "pro",
            "quota_reset_date_utc": "2026-07-01T00:00:00.000Z",
            "quota_snapshots": {
                "premium_interactions": snap(300.0, 120.0, false),
                "chat": snap(200.0, 50.0, false),
            }
        }))
        .await;

        assert_eq!(status.plan, "Pro");
        assert_eq!(status.quotas.len(), 1);
        let q = &status.quotas[0];
        assert_eq!(q.label, "AI Credits");
        assert_eq!(q.total, 300.0);
        assert_eq!(q.used, 180.0, "used = entitlement - remaining");
        assert_eq!(q.resets_at, "2026-07-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap());
    }

    #[tokio::test]
    async fn falls_back_past_zero_entitlement_premium() {
        // Free/individual accounts report premium_interactions with entitlement 0;
        // the headline should fall through to the next bounded quota.
        let (_s, status) = serve(json!({
            "copilot_plan": "individual",
            "quota_reset_date_utc": "2026-06-30T22:00:00.000Z",
            "quota_snapshots": {
                "premium_interactions": snap(0.0, 0.0, false),
                "chat": snap(200.0, 184.0, false),
                "completions": snap(2000.0, 2000.0, false),
            }
        }))
        .await;

        assert_eq!(status.plan, "Individual");
        assert_eq!(status.quotas.len(), 1);
        let q = &status.quotas[0];
        assert_eq!(q.label, "Chat");
        assert_eq!(q.total, 200.0);
        assert_eq!(q.used, 16.0);
    }

    #[tokio::test]
    async fn unlimited_quotas_are_skipped() {
        let (_s, status) = serve(json!({
            "copilot_plan": "enterprise",
            "quota_snapshots": {
                "premium_interactions": snap(0.0, 0.0, true),
                "chat": snap(0.0, 0.0, true),
            }
        }))
        .await;

        assert_eq!(status.plan, "Enterprise");
        assert!(matches!(status.state, ServiceState::Active));
        assert!(status.quotas.is_empty(), "all-unlimited plan shows no budget row");
    }

    #[tokio::test]
    async fn missing_reset_date_falls_back_to_next_month() {
        let (_s, status) = serve(json!({
            "copilot_plan": "pro",
            "quota_snapshots": { "premium_interactions": snap(300.0, 300.0, false) }
        }))
        .await;
        // Just assert it parsed to a future-of-now first-of-month (day == 1).
        assert_eq!(status.quotas[0].resets_at.day(), 1);
    }

    #[tokio::test]
    async fn http_401_returns_auth_rejected() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/copilot_internal/user"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;
        let source = CopilotSource::with_base_url(server.uri());
        let err = source.fetch_with("bad").await.unwrap_err();
        assert!(matches!(err, SourceError::AuthRejected(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn http_403_returns_auth_rejected() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/copilot_internal/user"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
            .mount(&server)
            .await;
        let source = CopilotSource::with_base_url(server.uri());
        let err = source.fetch_with("bad").await.unwrap_err();
        assert!(matches!(err, SourceError::AuthRejected(_)), "got {err:?}");
    }

    #[test]
    fn plan_label_mapping() {
        assert_eq!(plan_label("pro"), "Pro");
        assert_eq!(plan_label("pro_plus"), "Pro+");
        assert_eq!(plan_label("individual"), "Individual");
        assert_eq!(plan_label("free_limited_copilot"), "Free Limited Copilot");
        assert_eq!(plan_label(""), "—");
    }

    #[test]
    fn quota_label_mapping() {
        assert_eq!(quota_label("premium_interactions"), "AI Credits");
        assert_eq!(quota_label("chat"), "Chat");
        assert_eq!(quota_label("code_review"), "Code Review");
    }
}
