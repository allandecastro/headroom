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

/// The quota that holds the metered budget across regimes. `chat`/`completions`
/// are deliberately ignored as headline candidates — they're always `unlimited`.
/// When `premium_interactions` is absent we fall back to any `has_quota` snapshot
/// (never chat/completions). No quota_id is ever guessed.
const HEADLINE_QUOTA_ID: &str = "premium_interactions";

/// Normalized Copilot usage, tagged on billing regime so the UI can render every
/// case and we degrade gracefully when GitHub changes the shape again. Regime is
/// read from the observed `token_based_billing` flag — never guessed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CopilotUsage {
    /// Legacy request-based billing (`token_based_billing` absent/false) with a
    /// real cap — grandfathered annual Pro/Pro+ plans.
    PremiumRequests {
        entitlement: f64,
        remaining: f64,
        used: f64,
        percent_remaining: Option<f64>,
        overage_permitted: bool,
        reset_date: DateTime<Utc>,
    },
    /// AI-Credits regime (`token_based_billing: true`) with a per-seat cap — a
    /// user-level budget is set, or an individual plan carries personal credits
    /// (Pro 1000 / Pro+ 3900). 1 credit = $0.01.
    AiCreditsCapped {
        entitlement: f64,
        remaining: f64,
        used: f64,
        percent_remaining: Option<f64>,
        overage_permitted: bool,
        reset_date: DateTime<Utc>,
    },
    /// AI-Credits regime with **no per-seat cap** — the credits live at the org
    /// level (pooled Business/Enterprise seat, no user budget) and are NOT exposed
    /// per-user in this payload. Renders "Org-managed (pooled) — no individual
    /// quota": never blank, never "Unlimited", never a 100%/0 bar. Carries no
    /// number precisely because `percent_remaining: 100 / remaining: 0` here is
    /// meaningless.
    AiCreditsPooled { reset_date: DateTime<Utc> },
    /// Nothing we recognize — carries the raw snapshot ids so the user can report
    /// the new shape (and the UI offers "copy raw payload").
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
        let usage = normalize_usage(body.token_based_billing, &body.quota_snapshots, resets_at);
        let quotas = usage_to_quotas(&usage);

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

/// Classify the snapshots into a normalized, regime-tagged usage. Regime is read
/// from the observed `token_based_billing` flag — never guessed. Never panics and
/// never returns "nothing": an unrecognized shape becomes [`CopilotUsage::Unknown`]
/// carrying its raw ids so the user can report it.
fn normalize_usage(
    token_based_billing: bool,
    snapshots: &HashMap<String, QuotaSnapshot>,
    reset_date: DateTime<Utc>,
) -> CopilotUsage {
    let headline = headline_quota(snapshots);
    if token_based_billing {
        // AI-Credits regime: capped if the headline carries a real per-seat cap,
        // otherwise pooled (org credits, not exposed per-user — the Business case).
        match headline {
            Some(snap) if snap.is_capped() => CopilotUsage::AiCreditsCapped {
                entitlement: snap.entitlement,
                remaining: snap.effective_remaining(),
                used: (snap.entitlement - snap.effective_remaining()).max(0.0),
                percent_remaining: snap.percent_remaining,
                overage_permitted: snap.overage_permitted,
                reset_date,
            },
            _ => CopilotUsage::AiCreditsPooled { reset_date },
        }
    } else {
        // Legacy request-based regime: a real cap → PremiumRequests; else Unknown
        // (we never promote the always-unlimited chat/completions to a headline).
        match headline {
            Some(snap) if snap.is_capped() => CopilotUsage::PremiumRequests {
                entitlement: snap.entitlement,
                remaining: snap.effective_remaining(),
                used: (snap.entitlement - snap.effective_remaining()).max(0.0),
                percent_remaining: snap.percent_remaining,
                overage_permitted: snap.overage_permitted,
                reset_date,
            },
            _ => unknown(snapshots),
        }
    }
}

/// The headline quota holder: `premium_interactions` by priority, else any
/// `has_quota` snapshot — but **never** `chat`/`completions` (always unlimited).
/// Selected by observed fields, no quota_id guessing.
fn headline_quota(snapshots: &HashMap<String, QuotaSnapshot>) -> Option<&QuotaSnapshot> {
    if let Some(snap) = snapshots.get(HEADLINE_QUOTA_ID) {
        return Some(snap);
    }
    snapshots
        .iter()
        .filter(|(id, _)| id.as_str() != "chat" && id.as_str() != "completions")
        .map(|(_, snap)| snap)
        .find(|snap| snap.has_quota)
}

fn unknown(snapshots: &HashMap<String, QuotaSnapshot>) -> CopilotUsage {
    let mut ids: Vec<String> = snapshots.keys().cloned().collect();
    ids.sort();
    CopilotUsage::Unknown {
        raw_snapshot_ids: ids,
    }
}

/// Map the normalized usage to renderable quota rows. The two capped regimes
/// produce a single Monthly row (so history/sparkline/projection keep working);
/// pooled and unknown produce no row — the UI renders those from `copilot_usage`,
/// deliberately without a percentage or count.
fn usage_to_quotas(usage: &CopilotUsage) -> Vec<Quota> {
    match usage {
        CopilotUsage::PremiumRequests {
            entitlement,
            used,
            reset_date,
            ..
        } => vec![Quota::new(
            QuotaWindow::Monthly,
            "Premium requests",
            *used,
            *entitlement,
            QuotaUnit::Requests,
            *reset_date,
        )],
        CopilotUsage::AiCreditsCapped {
            entitlement,
            used,
            reset_date,
            ..
        } => vec![Quota::new(
            QuotaWindow::Monthly,
            "AI Credits",
            *used,
            *entitlement,
            QuotaUnit::Requests,
            *reset_date,
        )],
        CopilotUsage::AiCreditsPooled { .. } | CopilotUsage::Unknown { .. } => vec![],
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
    /// `true` once the account is on usage-based AI-Credits billing (post
    /// 2026-06-01). The regime discriminator — observed, not guessed.
    #[serde(default)]
    token_based_billing: bool,
}

/// One `quota_snapshots` entry. Every field is optional/defaulted — the endpoint
/// is undocumented and its shape shifts between regimes, so we parse defensively.
/// We declare only the fields we actually read; serde silently ignores the rest
/// the payload carries (`quota_id`, `quota_reset_at`, per-snapshot
/// `token_based_billing`, `timestamp_utc`, …) — declaring unread fields would
/// only trip `-D warnings`.
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
    overage_permitted: bool,
    #[serde(default)]
    unlimited: bool,
    /// Marks the metered quota under token-based billing (observed `true` on
    /// `premium_interactions`, `false` on chat/completions).
    #[serde(default)]
    has_quota: bool,
}

impl QuotaSnapshot {
    /// A real per-seat cap worth showing as a budget: finite, non-zero, not
    /// flagged unlimited. The pooled Business case (`unlimited, entitlement 0`)
    /// is NOT capped — even though it reports `percent_remaining: 100`.
    fn is_capped(&self) -> bool {
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
    async fn legacy_pro_plus_is_premium_requests() {
        // Pre-migration grandfathered annual: no token_based_billing,
        // premium_interactions is a real cap. (The zed #44499 contrast payload.)
        let (_s, status) = serve(json!({
            "copilot_plan": "individual_pro",
            "quota_reset_date_utc": "2026-07-01T00:00:00.000Z",
            "quota_snapshots": {
                "premium_interactions": { "entitlement": 1500, "remaining": 1327, "quota_remaining": 1327.0, "percent_remaining": 88.5, "unlimited": false },
                "chat": snap(0.0, 0.0, true),
                "completions": snap(0.0, 0.0, true)
            }
        }))
        .await;

        assert_eq!(status.quotas.len(), 1);
        let q = &status.quotas[0];
        assert_eq!(q.label, "Premium requests");
        assert_eq!(q.total, 1500.0);
        assert_eq!(q.used, 173.0, "used = entitlement - remaining");
        assert!(matches!(
            status.copilot_usage,
            Some(CopilotUsage::PremiumRequests { .. })
        ));
    }

    #[tokio::test]
    async fn migrated_business_seat_is_pooled_not_blank() {
        // Real migrated Business payload (login AdrienDOS78): token_based_billing,
        // premium_interactions unlimited + entitlement 0 + has_quota:true. Credits
        // live at the org level — no per-seat balance here. TRAP: percent_remaining
        // is 100 with remaining 0 / entitlement 0 — we must render NEITHER a bar
        // NOR a count, and never blank.
        let (_s, status) = serve(json!({
            "copilot_plan": "business",
            "token_based_billing": true,
            "quota_reset_date": "2026-07-01",
            "quota_reset_date_utc": "2026-07-01T00:00:00.000Z",
            "quota_snapshots": {
                "chat": { "unlimited": true, "has_quota": false, "entitlement": 0, "remaining": 0, "percent_remaining": 100.0, "token_based_billing": true },
                "completions": { "unlimited": true, "has_quota": false, "entitlement": 0, "remaining": 0, "percent_remaining": 100.0, "token_based_billing": true },
                "premium_interactions": { "unlimited": true, "has_quota": true, "entitlement": 0, "remaining": 0, "percent_remaining": 100.0, "overage_permitted": true, "token_based_billing": true }
            }
        }))
        .await;

        assert_eq!(status.plan, "Business");
        assert!(status.quotas.is_empty(), "pooled: no bar, no count");
        assert!(
            matches!(
                status.copilot_usage,
                Some(CopilotUsage::AiCreditsPooled { .. })
            ),
            "expected pooled Business seat, got {:?}",
            status.copilot_usage
        );
    }

    #[tokio::test]
    async fn migrated_individual_pro_plus_is_ai_credits_capped() {
        // SYNTHETIC (no real sample yet): an individual migrated seat is NOT pooled
        // — it carries personal included credits, so token_based_billing is true
        // WITH a real entitlement. Must classify as capped + render a numeric row.
        let (_s, status) = serve(json!({
            "copilot_plan": "individual_pro_plus",
            "token_based_billing": true,
            "quota_reset_date_utc": "2026-07-01T00:00:00.000Z",
            "quota_snapshots": {
                "premium_interactions": { "entitlement": 3900, "remaining": 3510, "quota_remaining": 3510.0, "percent_remaining": 90.0, "unlimited": false, "has_quota": true, "token_based_billing": true },
                "chat": snap(0.0, 0.0, true),
                "completions": snap(0.0, 0.0, true)
            }
        }))
        .await;

        assert_eq!(status.quotas.len(), 1);
        let q = &status.quotas[0];
        assert_eq!(q.label, "AI Credits");
        assert_eq!(q.total, 3900.0);
        assert_eq!(q.used, 390.0);
        assert!(matches!(
            status.copilot_usage,
            Some(CopilotUsage::AiCreditsCapped { .. })
        ));
    }

    #[tokio::test]
    async fn chat_and_completions_never_become_the_headline() {
        // Legacy with no premium cap, only chat/completions bounded — we must NOT
        // promote them to a headline; classify as Unknown (surfaces ids).
        let (_s, status) = serve(json!({
            "copilot_plan": "free",
            "quota_snapshots": {
                "chat": snap(50.0, 40.0, false),
                "completions": snap(2000.0, 1800.0, false)
            }
        }))
        .await;
        assert!(status.quotas.is_empty());
        match status.copilot_usage {
            Some(CopilotUsage::Unknown { raw_snapshot_ids }) => {
                assert_eq!(raw_snapshot_ids, vec!["chat", "completions"]);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unrecognized_shape_is_unknown_with_raw_ids() {
        // A shape we don't recognize at all → surface ids, never blank.
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
