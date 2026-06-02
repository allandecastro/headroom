//! GitHub Copilot quota source.
//!
//! Endpoint: `GET https://api.github.com/copilot_internal/user` with a Bearer
//! GitHub token.
//!
//! ⚠️ **Undocumented / unsupported.** This is the same internal endpoint the
//! VS Code and Zed integrations use; GitHub provides no official individual
//! usage API, and this one can change or vanish without notice. It DID change
//! shape when all Copilot plans moved from "premium requests" to usage-based
//! **AI Credits** on 2026-06-01. Parsing is therefore deliberately
//! regime-aware and degrades gracefully — it never renders a blank card for an
//! account it can't classify. **Do not treat pre-2026-06-01 sample payloads as
//! current truth** (e.g. the Pro+ sample in zed-industries/zed #44499 is dated
//! 2026-01-30 and shows the legacy `premium_interactions` *request* counter).
//!
//! See SPEC.md § "Data sources / GitHub Copilot" and docs/copilot-billing-research.md.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

use super::{Quota, QuotaSource, QuotaUnit, QuotaWindow, ServiceState, ServiceStatus, SourceError};
use crate::credentials::Credentials;

const DEFAULT_BASE_URL: &str = "https://api.github.com";

/// Billing regime a quota snapshot represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Regime {
    /// New usage-based AI Credits (default after 2026-06-01). Intentionally NOT
    /// wired to a quota id yet — we have never observed a migrated payload, and
    /// we do not guess the id. It joins [`HEADLINE_QUOTAS`] only once a real
    /// payload confirms it (via Copy diagnostics). Constructed in tests to prove
    /// the path works.
    #[allow(dead_code)]
    Credits,
    /// Request-counting quota: legacy premium requests (grandfathered annual
    /// plans) and the Free plan's chat/completions allowances.
    Requests,
}

/// Quota ids we know how to surface, **headline priority first**, each tagged
/// with the billing regime it represents and the label to show.
///
/// ⚠️ Every id here is one we have **actually observed** in a real payload — we
/// do **not** guess. The endpoint is undocumented and changed with the
/// 2026-06-01 AI-Credits migration; we have not yet seen a migrated payload, so
/// the new AI-Credits id is **deliberately absent**. Until a real one is
/// captured (via the "Copy Copilot diagnostics" action — see
/// [`CopilotSource::fetch_raw`]), a credits-only seat falls through to
/// [`CopilotUsage::Unknown`] carrying its raw ids — surfaced for reporting,
/// never mis-shown under a guessed label. Add the confirmed id (with
/// [`Regime::Credits`]) here once the real payload lands.
const HEADLINE_QUOTAS: &[(&str, Regime, &str)] = &[
    // Legacy premium-request counter (grandfathered annual plans).
    ("premium_interactions", Regime::Requests, "Premium requests"),
    // Free-plan request allowances.
    ("chat", Regime::Requests, "Chat"),
    ("completions", Regime::Requests, "Completions"),
];

/// Normalized Copilot usage, tagged on billing regime so the UI can render any
/// case and we degrade gracefully when GitHub changes the shape again.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CopilotUsage {
    /// Request-counting quota (legacy premium requests, or Free chat/completions).
    PremiumRequests {
        label: String,
        entitlement: f64,
        remaining: f64,
        used: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        percent_remaining: Option<f64>,
        overage_permitted: bool,
        overage_count: f64,
    },
    /// New usage-based AI Credits (1 credit = $0.01).
    AiCredits {
        label: String,
        included_credits: f64,
        credits_remaining: f64,
        used: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        percent_remaining: Option<f64>,
        overage_permitted: bool,
        overage_count: f64,
    },
    /// A recognized quota that has no enforced cap.
    Unlimited { label: String },
    /// Nothing we recognize — carries the raw snapshot ids so the user can
    /// report the new shape (and we extend [`HEADLINE_QUOTAS`]).
    Unknown { raw_snapshot_ids: Vec<String> },
}

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
        let resets_at = parse_reset(&body, now);
        let usage = normalize_usage(&body.quota_snapshots);
        let quotas = usage_to_quotas(&usage, resets_at);

        Ok(ServiceStatus {
            id: "copilot".into(),
            name: "GitHub Copilot".into(),
            plan: plan_label(&body.copilot_plan),
            state: ServiceState::Active,
            quotas,
            error_detail: None,
            copilot_usage: Some(usage),
        })
    }

    /// Fetch the raw `copilot_internal/user` JSON for diagnostics, pretty-printed,
    /// with the bearer token redacted defensively (the response body carries no
    /// token, but we never want to leak it if a field ever echoed it). Drives the
    /// "Copy Copilot diagnostics" action so a user can paste their real
    /// post-migration payload — the only ground truth for an undocumented,
    /// recently-changed endpoint.
    pub(crate) async fn fetch_raw(&self, token: &str) -> Result<String, SourceError> {
        let url = format!("{}/copilot_internal/user", self.base_url);
        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::ACCEPT, "application/json")
            .send()
            .await?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        let pretty = serde_json::from_str::<serde_json::Value>(&text)
            .and_then(|v| serde_json::to_string_pretty(&v))
            .unwrap_or(text);
        let redacted = pretty.replace(token, "<redacted>");
        Ok(format!(
            "GET /copilot_internal/user → HTTP {status}\n\n{redacted}"
        ))
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

/// Classify the snapshots into a normalized, regime-tagged usage against the
/// production [`HEADLINE_QUOTAS`] table.
fn normalize_usage(snapshots: &HashMap<String, QuotaSnapshot>) -> CopilotUsage {
    classify(snapshots, HEADLINE_QUOTAS)
}

/// Core classification, parameterized on the quota table so tests can exercise
/// the AI-Credits path with a table that includes a credits id — WITHOUT baking
/// a guessed id into the shipped [`HEADLINE_QUOTAS`]. Never panics and never
/// returns "nothing": an unrecognized shape becomes [`CopilotUsage::Unknown`]
/// carrying its raw ids so the user can report it.
fn classify(
    snapshots: &HashMap<String, QuotaSnapshot>,
    table: &[(&str, Regime, &str)],
) -> CopilotUsage {
    // 1) First recognized id with a *bounded* quota wins — show real numbers.
    for (id, regime, label) in table {
        let Some(snap) = snapshots.get(*id) else {
            continue;
        };
        if !snap.is_bounded() {
            continue;
        }
        let entitlement = snap.entitlement;
        let remaining = snap.effective_remaining();
        let used = (entitlement - remaining).max(0.0);
        return match regime {
            Regime::Credits => CopilotUsage::AiCredits {
                label: (*label).to_string(),
                included_credits: entitlement,
                credits_remaining: remaining,
                used,
                percent_remaining: snap.percent_remaining,
                overage_permitted: snap.overage_permitted,
                overage_count: snap.overage_count,
            },
            Regime::Requests => CopilotUsage::PremiumRequests {
                label: (*label).to_string(),
                entitlement,
                remaining,
                used,
                percent_remaining: snap.percent_remaining,
                overage_permitted: snap.overage_permitted,
                overage_count: snap.overage_count,
            },
        };
    }

    // 2) A recognized id exists but is unlimited → say "Unlimited", not blank.
    for (id, _regime, label) in table {
        if snapshots.get(*id).map(|s| s.unlimited).unwrap_or(false) {
            return CopilotUsage::Unlimited {
                label: (*label).to_string(),
            };
        }
    }

    // 3) Nothing recognized. If every present snapshot is unlimited, it's a
    //    genuinely uncapped plan; otherwise surface the raw ids to report.
    if !snapshots.is_empty() && snapshots.values().all(|s| s.unlimited) {
        return CopilotUsage::Unlimited {
            label: "Copilot".to_string(),
        };
    }
    let mut ids: Vec<String> = snapshots.keys().cloned().collect();
    ids.sort();
    CopilotUsage::Unknown {
        raw_snapshot_ids: ids,
    }
}

/// Map the normalized usage to renderable quota rows. Numeric regimes produce a
/// single Monthly row (so history/sparkline/projection keep working); Unlimited
/// and Unknown produce no row — the UI renders them from `copilot_usage`.
fn usage_to_quotas(usage: &CopilotUsage, resets_at: DateTime<Utc>) -> Vec<Quota> {
    match usage {
        CopilotUsage::AiCredits {
            label,
            included_credits,
            used,
            ..
        } => vec![Quota::new(
            QuotaWindow::Monthly,
            label.clone(),
            *used,
            *included_credits,
            QuotaUnit::Requests,
            resets_at,
        )],
        CopilotUsage::PremiumRequests {
            label,
            entitlement,
            used,
            ..
        } => vec![Quota::new(
            QuotaWindow::Monthly,
            label.clone(),
            *used,
            *entitlement,
            QuotaUnit::Requests,
            resets_at,
        )],
        CopilotUsage::Unlimited { .. } | CopilotUsage::Unknown { .. } => vec![],
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

/// Resolve the reset timestamp with a defensive fallback chain: the RFC 3339
/// `quota_reset_date_utc`, then the date-only `quota_reset_date`, then the start
/// of next month. The 2026-01-30 #44499 sample carried BOTH date fields, so we
/// prefer `_utc` but tolerate either being dropped in a future shape.
fn parse_reset(body: &UserResponse, now: DateTime<Utc>) -> DateTime<Utc> {
    body.quota_reset_date_utc
        .as_deref()
        .and_then(parse_date_or_datetime)
        .or_else(|| {
            body.quota_reset_date
                .as_deref()
                .and_then(parse_date_or_datetime)
        })
        .unwrap_or_else(|| next_month_start_utc(now))
}

/// Parse either an RFC 3339 datetime ("…T00:00:00Z") or a bare date ("2026-02-01").
fn parse_date_or_datetime(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc())
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
    #[serde(default)]
    quota_reset_date: Option<String>,
}

/// One `quota_snapshots` entry. Every field is optional/defaulted — the endpoint
/// is undocumented and its shape shifts between regimes, so we parse defensively.
#[derive(Deserialize)]
struct QuotaSnapshot {
    #[serde(default)]
    entitlement: f64,
    /// Fractional remaining (preferred). Optional so we can fall back to `remaining`.
    #[serde(default)]
    quota_remaining: Option<f64>,
    /// Integer remaining (fallback when `quota_remaining` is absent).
    #[serde(default)]
    remaining: Option<f64>,
    #[serde(default)]
    percent_remaining: Option<f64>,
    #[serde(default)]
    overage_count: f64,
    #[serde(default)]
    overage_permitted: bool,
    #[serde(default)]
    unlimited: bool,
}

impl QuotaSnapshot {
    /// A quota worth showing as a budget: a finite, non-zero allowance.
    fn is_bounded(&self) -> bool {
        !self.unlimited && self.entitlement > 0.0
    }

    /// Remaining, preferring the fractional `quota_remaining` over integer
    /// `remaining`, defaulting to 0.0 when neither is present.
    fn effective_remaining(&self) -> f64 {
        self.quota_remaining.or(self.remaining).unwrap_or(0.0)
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
    async fn legacy_premium_interactions_is_the_headline() {
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
        // premium_interactions is the LEGACY request counter, not AI Credits.
        assert_eq!(q.label, "Premium requests");
        assert_eq!(q.total, 300.0);
        assert_eq!(q.used, 180.0, "used = entitlement - remaining");
        assert!(matches!(
            status.copilot_usage,
            Some(CopilotUsage::PremiumRequests { .. })
        ));
    }

    #[test]
    fn credits_regime_classifies_as_ai_credits_when_the_id_is_known() {
        // Proves the AI-Credits path works once we add the *real* id — without
        // shipping a guessed id in HEADLINE_QUOTAS. The id here is a test
        // fixture, NOT a claim about GitHub's actual field name.
        let table: &[(&str, Regime, &str)] =
            &[("unconfirmed-credits-id", Regime::Credits, "AI Credits")];
        let mut snaps = HashMap::new();
        snaps.insert(
            "unconfirmed-credits-id".to_string(),
            QuotaSnapshot {
                entitlement: 7000.0,
                quota_remaining: Some(6500.0),
                remaining: None,
                percent_remaining: None,
                overage_count: 0.0,
                overage_permitted: false,
                unlimited: false,
            },
        );
        match classify(&snaps, table) {
            CopilotUsage::AiCredits {
                included_credits,
                used,
                ..
            } => {
                assert_eq!(included_credits, 7000.0);
                assert_eq!(used, 500.0);
            }
            other => panic!("expected AiCredits, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unconfirmed_credits_shape_is_unknown_not_guessed() {
        // We have NOT confirmed the migrated credits id, so a credits-shaped
        // payload must surface as Unknown (with raw ids) — never guessed into a
        // headline number under an "AI Credits" label.
        let (_s, status) = serve(json!({
            "copilot_plan": "pro_plus",
            "quota_snapshots": { "ai_credits": snap(7000.0, 6500.0, false) }
        }))
        .await;
        match status.copilot_usage {
            Some(CopilotUsage::Unknown { raw_snapshot_ids }) => {
                assert_eq!(raw_snapshot_ids, vec!["ai_credits"]);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
        assert!(status.quotas.is_empty());
    }

    #[tokio::test]
    async fn falls_back_past_zero_entitlement_premium_to_chat() {
        // Free/individual: premium_interactions entitlement 0 → fall through to chat.
        let (_s, status) = serve(json!({
            "copilot_plan": "individual",
            "quota_snapshots": {
                "premium_interactions": snap(0.0, 0.0, false),
                "chat": snap(200.0, 184.0, false),
                "completions": snap(2000.0, 2000.0, false),
            }
        }))
        .await;
        let q = &status.quotas[0];
        assert_eq!(q.label, "Chat");
        assert_eq!(q.total, 200.0);
        assert_eq!(q.used, 16.0);
    }

    #[tokio::test]
    async fn all_unlimited_renders_unlimited_not_blank() {
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
        assert!(status.quotas.is_empty(), "no numeric row for unlimited");
        assert!(
            matches!(status.copilot_usage, Some(CopilotUsage::Unlimited { .. })),
            "unlimited plan must classify as Unlimited, not blank"
        );
    }

    #[tokio::test]
    async fn unrecognized_shape_is_unknown_with_raw_ids() {
        // The migration could rename the credits object to something we don't
        // probe yet — must surface ids, never blank.
        let (_s, status) = serve(json!({
            "copilot_plan": "pro_plus",
            "quota_snapshots": {
                "monthly_credit_budget": snap(7000.0, 6500.0, false),
                "some_new_meter": snap(100.0, 100.0, false),
            }
        }))
        .await;

        assert!(status.quotas.is_empty());
        match status.copilot_usage {
            Some(CopilotUsage::Unknown { raw_snapshot_ids }) => {
                assert_eq!(
                    raw_snapshot_ids,
                    vec!["monthly_credit_budget", "some_new_meter"]
                );
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn reset_falls_back_to_date_only_then_next_month() {
        // Only the date-only field present → parsed to midnight UTC of that date.
        let (_s, status) = serve(json!({
            "copilot_plan": "pro",
            "quota_reset_date": "2026-07-01",
            "quota_snapshots": { "premium_interactions": snap(300.0, 300.0, false) }
        }))
        .await;
        assert_eq!(
            status.quotas[0].resets_at,
            "2026-07-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }

    #[tokio::test]
    async fn missing_reset_date_falls_back_to_next_month() {
        let (_s, status) = serve(json!({
            "copilot_plan": "pro",
            "quota_snapshots": { "premium_interactions": snap(300.0, 300.0, false) }
        }))
        .await;
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
    async fn fetch_raw_redacts_token_and_pretty_prints() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/copilot_internal/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"copilot_plan": "pro"})))
            .mount(&server)
            .await;
        let source = CopilotSource::with_base_url(server.uri());
        let raw = source.fetch_raw("secret-token").await.unwrap();
        assert!(raw.contains("copilot_plan"));
        assert!(!raw.contains("secret-token"), "token must be redacted");
    }

    #[test]
    fn plan_label_mapping() {
        assert_eq!(plan_label("pro"), "Pro");
        assert_eq!(plan_label("pro_plus"), "Pro+");
        assert_eq!(plan_label("business"), "Business");
        assert_eq!(plan_label(""), "—");
    }
}
