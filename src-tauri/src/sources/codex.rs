//! OpenAI Codex usage source.
//!
//! Unlike Claude/Copilot, Codex usage needs **no separate sign-in and no token
//! paste**. We read it from two complementary local sources:
//!
//! 1. **Active query** — `GET https://chatgpt.com/backend-api/wham/usage` with the
//!    ChatGPT-plan token already stored by the Codex CLI in `$CODEX_HOME/auth.json`
//!    (`CODEX_HOME` defaults to `~/.codex`). This is Codex's own zero-cost
//!    rate-limit endpoint (no model turn consumed), so it's fresh and works even
//!    when sessions ran in exec mode. Gated by the `codex_live_query` setting.
//! 2. **Local rollout logs** — `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-*.jsonl`.
//!    Each `token_count` event may carry a `rate_limits` snapshot (primary = 5h,
//!    secondary = weekly). Exec-mode sessions log `rate_limits: null`
//!    (openai/codex#14728), so we scan newest-first for the most recent non-null
//!    one. Used as the fallback when the active query is off/unavailable, and the
//!    sole source of the token-consumption stats.
//!
//! Both shapes are mirrored from openai/codex `protocol.rs` (rollout) and the
//! `/codex/usage` response. The endpoints are undocumented and may change, so
//! parsing is defensive and never panics; missing data degrades to guidance copy.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use tracing::warn;

use super::{Quota, QuotaSource, QuotaUnit, QuotaWindow, ServiceState, ServiceStatus, SourceError};
use crate::credentials::Credentials;

/// Production ChatGPT backend root. The usage path is derived per Codex's own
/// `PathStyle` (see [`usage_url`]): `/wham/usage` for this `…/backend-api` host.
const DEFAULT_BASE_URL: &str = "https://chatgpt.com/backend-api";

/// Row labels — distinct from Claude's so a popover showing both reads clearly.
const PRIMARY_LABEL: &str = "Current session · 5h";
const SECONDARY_LABEL: &str = "Weekly · 7d";

/// Upper bound on rollout files read per poll, so a huge history can't stall the
/// scan. We only need the most recent snapshot + the last 24h for token stats.
const MAX_FILES_SCANNED: usize = 200;

// ─── Public types carried on ServiceStatus.codex_meta ────────────────────────

/// Where the rate-limit numbers came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexSourceKind {
    /// Fresh from the `/codex/usage` endpoint.
    Live,
    /// Last-observed from a local rollout log.
    Logs,
    /// No rate-limit numbers available (only token stats, or nothing yet).
    Empty,
}

/// Cumulative token consumption, aggregated from local `token_count` events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodexTokenStats {
    pub input: i64,
    pub cached_input: i64,
    pub output: i64,
    pub reasoning: i64,
    pub total: i64,
    /// Human label for the aggregation window (e.g. "last 24h").
    pub window_label: String,
}

/// Codex-specific extras the renderer shows alongside (or instead of) the % rows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodexMeta {
    pub source: CodexSourceKind,
    /// When the snapshot was captured (live = poll time; logs = the event's time).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captured_at: Option<DateTime<Utc>>,
    /// A log snapshot whose window already reset — shown as "may be stale".
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stale: bool,
    /// The live token was rejected (expired) — surface "run `codex` to refresh".
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub token_expired: bool,
    /// Credits balance string from the snapshot, when present (e.g. "$12.34").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credits_balance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_stats: Option<CodexTokenStats>,
    /// Guidance copy shown when there are no % rows to draw.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

// ─── The source ──────────────────────────────────────────────────────────────

pub struct CodexSource {
    client: Client,
    base_url: String,
    /// Resolved `$CODEX_HOME` (or `~/.codex`); `None` if no home dir is known.
    home: Option<PathBuf>,
    /// Whether to hit the live `/codex/usage` endpoint (the `codex_live_query`
    /// setting). When off, only local logs are read.
    live: bool,
}

impl Default for CodexSource {
    fn default() -> Self {
        Self::new(true)
    }
}

impl CodexSource {
    pub fn new(live: bool) -> Self {
        Self {
            client: build_client(),
            base_url: DEFAULT_BASE_URL.to_string(),
            home: codex_home(),
            live,
        }
    }

    /// Point the reader at an explicit home (test-only).
    #[cfg(test)]
    pub(crate) fn with_home(home: impl Into<PathBuf>, live: bool) -> Self {
        Self {
            client: build_client(),
            base_url: DEFAULT_BASE_URL.to_string(),
            home: Some(home.into()),
            live,
        }
    }

    /// Point the live endpoint at a mock server (test-only).
    #[cfg(test)]
    pub(crate) fn with_base_url(home: impl Into<PathBuf>, base_url: impl Into<String>) -> Self {
        Self {
            client: build_client(),
            base_url: base_url.into(),
            home: Some(home.into()),
            live: true,
        }
    }

    /// Hit the live `/codex/usage` endpoint and normalize its windows.
    pub(crate) async fn fetch_usage(
        &self,
        token: &str,
        account_id: Option<&str>,
    ) -> Result<Vec<WindowRow>, SourceError> {
        let url = usage_url(&self.base_url);
        let mut req = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::ACCEPT, "application/json");
        if let Some(acc) = account_id {
            req = req.header("ChatGPT-Account-Id", acc);
        }
        let response = req.send().await?;

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

        let body: UsageResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let mut rows = Vec::new();
        if let Some(rl) = body.chosen() {
            if let Some(p) = rl.primary_window.as_ref() {
                rows.push(WindowRow::new(QuotaWindow::FiveHour, PRIMARY_LABEL, p));
            }
            if let Some(s) = rl.secondary_window.as_ref() {
                rows.push(WindowRow::new(QuotaWindow::WeeklyAll, SECONDARY_LABEL, s));
            }
        }
        Ok(rows)
    }

    /// Raw `/codex/usage` payload for diagnostics (token redacted, status line).
    async fn fetch_usage_raw(
        &self,
        token: &str,
        account_id: Option<&str>,
    ) -> Result<String, SourceError> {
        let url = usage_url(&self.base_url);
        let mut req = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::ACCEPT, "application/json");
        if let Some(acc) = account_id {
            req = req.header("ChatGPT-Account-Id", acc);
        }
        let response = req.send().await?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        let pretty = serde_json::from_str::<serde_json::Value>(&text)
            .and_then(|v| serde_json::to_string_pretty(&v))
            .unwrap_or(text);
        let redacted = pretty.replace(token, "<redacted>");
        Ok(format!("GET {url} → HTTP {status}\n\n{redacted}"))
    }

    /// Human-readable diagnostics dump: home, CLI version, auth mode, the live
    /// payload (if reachable), and the latest local rate_limits line. No secrets.
    pub(crate) async fn diagnostics(&self) -> Result<String, String> {
        let Some(home) = self.home.clone() else {
            return Err("Codex home (~/.codex or $CODEX_HOME) not found.".to_string());
        };
        if !home.exists() {
            return Err(format!(
                "{} does not exist — is Codex installed?",
                home.display()
            ));
        }

        let auth = read_auth(&home);
        let mut out = String::new();
        out.push_str(&format!("CODEX_HOME = {}\n", home.display()));
        let home_v = home.clone();
        let version = tokio::task::spawn_blocking(move || codex_cli_version(&home_v))
            .await
            .ok()
            .flatten();
        out.push_str(&format!(
            "cli version = {}\n",
            version.unwrap_or_else(|| "unknown".to_string())
        ));
        out.push_str(&format!(
            "auth = {}\n\n",
            match &auth {
                Some(a) if a.access_token.is_some() => "ChatGPT sign-in (token present)",
                Some(_) => "auth.json present (no access token)",
                None => "no ChatGPT auth.json (local logs only)",
            }
        ));

        if let Some(token) = auth.as_ref().and_then(|a| a.access_token.clone()) {
            let acc = auth.as_ref().and_then(|a| a.account_id.clone());
            match self.fetch_usage_raw(&token, acc.as_deref()).await {
                Ok(raw) => {
                    out.push_str("— live usage query —\n");
                    out.push_str(&raw);
                    out.push_str("\n\n");
                }
                Err(e) => out.push_str(&format!("— live usage query failed: {e} —\n\n")),
            }
        }

        let home_v = home.clone();
        let raw_line = tokio::task::spawn_blocking(move || latest_raw_rate_limits_line(&home_v))
            .await
            .map_err(|e| e.to_string())?;
        match raw_line {
            Some(line) => {
                out.push_str("— latest local rate_limits (rollout log) —\n");
                out.push_str(&line);
            }
            None => out.push_str(
                "— no local rate_limits found (exec-mode sessions log null; \
                 run an interactive `codex` session) —",
            ),
        }
        Ok(out)
    }
}

#[async_trait]
impl QuotaSource for CodexSource {
    fn id(&self) -> &str {
        "codex"
    }

    fn name(&self) -> &str {
        "Codex"
    }

    async fn fetch(&self, _creds: &Credentials) -> Result<ServiceStatus, SourceError> {
        // No `~/.codex` → not installed; treat like "needs setup" so the popover
        // hides it rather than alarming a user who doesn't use Codex.
        let Some(home) = self.home.clone() else {
            return Err(SourceError::MissingCredentials("codex.home"));
        };
        if !home.exists() {
            return Err(SourceError::MissingCredentials("codex.home"));
        }

        // auth.json (best effort) — drives the live query + plan tier.
        let auth = read_auth(&home);
        let signed_in = auth
            .as_ref()
            .map(|a| a.access_token.is_some())
            .unwrap_or(false);

        // Local logs: latest snapshot (+ credits/plan) and token-consumption stats.
        let home_scan = home.clone();
        let ScanOutcome {
            snapshot: snap,
            token_stats,
        } = tokio::task::spawn_blocking(move || scan_logs(&home_scan))
            .await
            .map_err(|e| SourceError::Parse(format!("codex log scan panicked: {e}")))?;

        let now = Utc::now();

        // Active, authoritative endpoint first (when enabled and signed in).
        let mut token_expired = false;
        let live_windows = if self.live {
            if let Some(token) = auth.as_ref().and_then(|a| a.access_token.clone()) {
                let acc = auth.as_ref().and_then(|a| a.account_id.clone());
                match self.fetch_usage(&token, acc.as_deref()).await {
                    Ok(rows) if !rows.is_empty() => Some(rows),
                    Ok(_) => None, // reachable but no windows — fall back to logs
                    Err(SourceError::AuthRejected(_)) => {
                        token_expired = true;
                        None
                    }
                    Err(e) => {
                        warn!(error = ?e, "codex live usage fetch failed; using local logs");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // Choose the window source: live first, else the latest log snapshot.
        let mut stale = false;
        let (source, rows, captured_at) = if let Some(rows) = live_windows {
            (CodexSourceKind::Live, rows, Some(now))
        } else if let Some(s) = &snap {
            let rows = s.windows(now, &mut stale);
            (CodexSourceKind::Logs, rows, Some(s.captured_at))
        } else {
            (CodexSourceKind::Empty, Vec::new(), None)
        };

        let credits_balance = snap
            .as_ref()
            .and_then(|s| s.rate_limits.credits.as_ref())
            .and_then(|c| c.balance.clone());
        let log_plan = snap
            .as_ref()
            .and_then(|s| plan_value_to_string(&s.rate_limits.plan_type));

        let quotas = windows_to_quotas(&rows, now);

        let plan = auth
            .as_ref()
            .and_then(|a| a.plan.clone())
            .or(log_plan)
            .map(|p| plan_label(&p))
            .unwrap_or_default();

        let note = quotas
            .is_empty()
            .then(|| empty_note(token_expired, signed_in));

        Ok(ServiceStatus {
            id: "codex".into(),
            name: "Codex".into(),
            plan,
            state: ServiceState::Active,
            quotas,
            error_detail: None,
            copilot_usage: None,
            codex_meta: Some(CodexMeta {
                source,
                captured_at,
                stale,
                token_expired,
                credits_balance,
                token_stats,
                note,
            }),
        })
    }
}

// ─── Mapping helpers ─────────────────────────────────────────────────────────

/// A normalized rate-limit window, source-agnostic (live or logs).
#[derive(Debug)]
pub(crate) struct WindowRow {
    window: QuotaWindow,
    label: &'static str,
    used_percent: f64,
    resets_at: Option<i64>,
}

impl WindowRow {
    fn new(window: QuotaWindow, label: &'static str, w: &UsageWindow) -> Self {
        Self {
            window,
            label,
            used_percent: w.used_percent,
            resets_at: w.reset_at,
        }
    }
}

fn windows_to_quotas(rows: &[WindowRow], now: DateTime<Utc>) -> Vec<Quota> {
    rows.iter()
        .map(|r| {
            let used = r.used_percent.clamp(0.0, 100.0);
            // Absolute unix reset → DateTime. A past/missing reset is re-based to
            // now + window length so history/projection stay valid (and the row
            // reads "resetting…"). Staleness is flagged separately on the meta.
            let resets_at = r
                .resets_at
                .and_then(|s| Utc.timestamp_opt(s, 0).single())
                .filter(|t| *t > now)
                .unwrap_or_else(|| now + r.window.duration());
            Quota::new(
                r.window,
                r.label,
                used,
                100.0,
                QuotaUnit::Percent,
                resets_at,
            )
        })
        .collect()
}

fn empty_note(token_expired: bool, signed_in: bool) -> String {
    if token_expired {
        "Codex sign-in expired — run `codex` to refresh; your limits reappear once it reconnects."
            .to_string()
    } else if signed_in {
        "No rate-limit data yet — run an interactive `codex` session to record your 5-hour and \
         weekly limits."
            .to_string()
    } else {
        "No rate-limit data yet. Sign in to ChatGPT in Codex, or run an interactive `codex` \
         session, to see your 5-hour and weekly limits."
            .to_string()
    }
}

/// User-facing plan name, brand-cased for the common ChatGPT tiers.
fn plan_label(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "" => String::new(),
        "free" => "Free".to_string(),
        "plus" => "Plus".to_string(),
        "pro" => "Pro".to_string(),
        "team" => "Team".to_string(),
        "business" => "Business".to_string(),
        "enterprise" => "Enterprise".to_string(),
        "edu" => "Edu".to_string(),
        other => titlecase(other),
    }
}

/// Title-case a snake/kebab/space separated identifier: `pro_plus` → `Pro Plus`.
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

fn plan_value_to_string(v: &Option<serde_json::Value>) -> Option<String> {
    match v {
        Some(serde_json::Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(serde_json::Value::Object(map)) => {
            map.values().find_map(|x| x.as_str()).map(str::to_string)
        }
        _ => None,
    }
}

// ─── auth.json ───────────────────────────────────────────────────────────────

/// Resolved ChatGPT auth pulled from `$CODEX_HOME/auth.json` (read-only — we
/// never refresh or rewrite it, to avoid racing the Codex CLI).
struct CodexAuth {
    access_token: Option<String>,
    account_id: Option<String>,
    plan: Option<String>,
}

fn read_auth(home: &Path) -> Option<CodexAuth> {
    let raw = std::fs::read_to_string(home.join("auth.json")).ok()?;
    let parsed: AuthDotJson = serde_json::from_str(&raw).ok()?;
    let tokens = parsed.tokens?;
    // account_id / plan: prefer the explicit token fields, then JWT claims.
    let claims = tokens.id_token.as_deref().and_then(decode_jwt_claims);
    let account_id = tokens
        .account_id
        .clone()
        .or_else(|| claims.as_ref().and_then(|c| c.account_id.clone()));
    let plan = claims.and_then(|c| c.plan);
    Some(CodexAuth {
        access_token: tokens.access_token,
        account_id,
        plan,
    })
}

#[derive(Deserialize)]
struct AuthDotJson {
    #[serde(default)]
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    account_id: Option<String>,
}

struct DecodedClaims {
    account_id: Option<String>,
    plan: Option<String>,
}

/// Decode the middle (payload) segment of a JWT and pull the ChatGPT auth claim.
/// We never verify the signature — this is the user's own already-trusted token,
/// read only to display the account id / plan tier.
fn decode_jwt_claims(jwt: &str) -> Option<DecodedClaims> {
    let payload_b64 = jwt.split('.').nth(1)?;
    let bytes = b64url_decode(payload_b64)?;
    let claims: JwtClaims = serde_json::from_slice(&bytes).ok()?;
    let auth = claims.auth?;
    Some(DecodedClaims {
        account_id: auth.chatgpt_account_id,
        plan: auth.chatgpt_plan_type,
    })
}

#[derive(Deserialize)]
struct JwtClaims {
    #[serde(rename = "https://api.openai.com/auth", default)]
    auth: Option<OpenAiAuthClaim>,
}

#[derive(Deserialize)]
struct OpenAiAuthClaim {
    #[serde(default)]
    chatgpt_account_id: Option<String>,
    #[serde(default)]
    chatgpt_plan_type: Option<String>,
}

/// Minimal base64url (no padding) decoder — avoids pulling in a crate just to
/// read a JWT payload.
fn b64url_decode(input: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }
    let input = input.trim_end_matches('=');
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &c in input.as_bytes() {
        buf = (buf << 6) | val(c)? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

// ─── Live usage response ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct UsageResponse {
    /// The default/primary limit (Codex resolves this as `snapshots[0]`).
    #[serde(default)]
    rate_limit: Option<UsageRateLimit>,
    /// Extra per-limit snapshots; Codex prefers the one tagged `limit_id: "codex"`.
    #[serde(default)]
    additional_rate_limits: Vec<UsageAdditional>,
}

impl UsageResponse {
    /// Mirror Codex's `get_rate_limits`: prefer the `"codex"`-tagged limit, else
    /// the default top-level one.
    fn chosen(&self) -> Option<&UsageRateLimit> {
        self.additional_rate_limits
            .iter()
            .find(|a| a.limit_id.as_deref() == Some("codex"))
            .and_then(|a| a.rate_limit.as_ref())
            .or(self.rate_limit.as_ref())
    }
}

#[derive(Deserialize)]
struct UsageAdditional {
    #[serde(default)]
    limit_id: Option<String>,
    #[serde(default)]
    rate_limit: Option<UsageRateLimit>,
}

#[derive(Deserialize)]
struct UsageRateLimit {
    #[serde(default)]
    primary_window: Option<UsageWindow>,
    #[serde(default)]
    secondary_window: Option<UsageWindow>,
}

#[derive(Deserialize)]
struct UsageWindow {
    #[serde(default)]
    used_percent: f64,
    /// Unix seconds. The endpoint sends this as a number *or* an ISO-8601 string.
    #[serde(default, deserialize_with = "de_opt_unix")]
    reset_at: Option<i64>,
}

fn de_opt_unix<'de, D>(d: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(unix_from_value))
}

fn unix_from_value(v: serde_json::Value) -> Option<i64> {
    match v {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        serde_json::Value::String(s) => s
            .parse::<i64>()
            .ok()
            .or_else(|| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.timestamp())),
        _ => None,
    }
}

// ─── Local rollout logs ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RolloutLine {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(rename = "type", default)]
    line_type: String,
    #[serde(default)]
    payload: Option<RolloutPayload>,
}

#[derive(Deserialize)]
struct RolloutPayload {
    #[serde(rename = "type", default)]
    payload_type: String,
    #[serde(default)]
    rate_limits: Option<LogRateLimits>,
    #[serde(default)]
    info: Option<LogTokenInfo>,
}

#[derive(Deserialize, Clone)]
struct LogRateLimits {
    #[serde(default)]
    primary: Option<LogWindow>,
    #[serde(default)]
    secondary: Option<LogWindow>,
    #[serde(default)]
    credits: Option<LogCredits>,
    #[serde(default)]
    plan_type: Option<serde_json::Value>,
}

#[derive(Deserialize, Clone)]
struct LogWindow {
    #[serde(default)]
    used_percent: f64,
    #[serde(default)]
    resets_at: Option<i64>,
}

#[derive(Deserialize, Clone)]
struct LogCredits {
    #[serde(default)]
    balance: Option<String>,
}

#[derive(Deserialize)]
struct LogTokenInfo {
    #[serde(default)]
    total_token_usage: Option<LogTokenUsage>,
}

#[derive(Deserialize, Clone, Default)]
struct LogTokenUsage {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    cached_input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    reasoning_output_tokens: i64,
    #[serde(default)]
    total_tokens: i64,
}

/// The most recent usable rate-limit snapshot from the logs.
struct LogSnapshot {
    rate_limits: LogRateLimits,
    captured_at: DateTime<Utc>,
}

impl LogSnapshot {
    fn windows(&self, now: DateTime<Utc>, stale: &mut bool) -> Vec<WindowRow> {
        let mut rows = Vec::new();
        if let Some(p) = &self.rate_limits.primary {
            rows.push(WindowRow {
                window: QuotaWindow::FiveHour,
                label: PRIMARY_LABEL,
                used_percent: p.used_percent,
                resets_at: p.resets_at,
            });
        }
        if let Some(s) = &self.rate_limits.secondary {
            rows.push(WindowRow {
                window: QuotaWindow::WeeklyAll,
                label: SECONDARY_LABEL,
                used_percent: s.used_percent,
                resets_at: s.resets_at,
            });
        }
        let now_secs = now.timestamp();
        if rows
            .iter()
            .any(|r| r.resets_at.map(|t| t <= now_secs).unwrap_or(false))
        {
            *stale = true;
        }
        rows
    }
}

struct ScanOutcome {
    snapshot: Option<LogSnapshot>,
    token_stats: Option<CodexTokenStats>,
}

#[derive(Default)]
struct FileParse {
    snapshot: Option<LogSnapshot>,
    session_total: Option<LogTokenUsage>,
}

#[derive(Default)]
struct TokenAcc {
    input: i64,
    cached_input: i64,
    output: i64,
    reasoning: i64,
    total: i64,
}

impl TokenAcc {
    fn add(&mut self, u: &LogTokenUsage) {
        self.input += u.input_tokens;
        self.cached_input += u.cached_input_tokens;
        self.output += u.output_tokens;
        self.reasoning += u.reasoning_output_tokens;
        self.total += u.total_tokens;
    }
    fn into_stats(self, label: &str) -> CodexTokenStats {
        CodexTokenStats {
            input: self.input,
            cached_input: self.cached_input,
            output: self.output,
            reasoning: self.reasoning,
            total: self.total,
            window_label: label.to_string(),
        }
    }
}

/// Walk the newest rollout files: find the most recent non-null `rate_limits`
/// snapshot, and sum the per-session token totals over the last 24h.
fn scan_logs(home: &Path) -> ScanOutcome {
    let sessions = home.join("sessions");
    if !sessions.exists() {
        return ScanOutcome {
            snapshot: None,
            token_stats: None,
        };
    }

    let files = newest_rollout_files(&sessions, MAX_FILES_SCANNED);
    let now = Utc::now();
    let cutoff = now - Duration::hours(24);

    let mut snapshot: Option<LogSnapshot> = None;
    let mut acc = TokenAcc::default();
    let mut any_stats = false;

    for (mtime, path) in files {
        let within = system_time_to_utc(mtime)
            .map(|t| t >= cutoff)
            .unwrap_or(false);
        // Files are newest-first; once we have a snapshot and we're past the 24h
        // window there's nothing left to gather.
        if snapshot.is_some() && !within {
            break;
        }
        let parsed = read_file(&path);
        if snapshot.is_none() {
            snapshot = parsed.snapshot;
        }
        if within {
            if let Some(t) = parsed.session_total {
                acc.add(&t);
                any_stats = true;
            }
        }
    }

    ScanOutcome {
        snapshot,
        token_stats: any_stats.then(|| acc.into_stats("last 24h")),
    }
}

/// Scan one file bottom-up (append-only ⇒ newest last): the bottom-most
/// `token_count` carries the session's cumulative tokens; the first non-null
/// `rate_limits` from the bottom is the most recent snapshot.
fn read_file(path: &Path) -> FileParse {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return FileParse::default();
    };
    let mut out = FileParse::default();
    for line in raw.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<RolloutLine>(line) else {
            continue; // tolerate a partial/corrupt trailing write
        };
        if parsed.line_type != "event_msg" {
            continue;
        }
        let Some(payload) = parsed.payload else {
            continue;
        };
        if payload.payload_type != "token_count" {
            continue;
        }
        if out.session_total.is_none() {
            out.session_total = payload.info.and_then(|i| i.total_token_usage);
        }
        if out.snapshot.is_none() {
            if let Some(rl) = payload.rate_limits {
                if rl.primary.is_some() || rl.secondary.is_some() {
                    let captured_at = parsed
                        .timestamp
                        .as_deref()
                        .and_then(parse_iso8601)
                        .unwrap_or_else(Utc::now);
                    out.snapshot = Some(LogSnapshot {
                        rate_limits: rl,
                        captured_at,
                    });
                }
            }
        }
        if out.snapshot.is_some() && out.session_total.is_some() {
            break;
        }
    }
    out
}

/// Latest non-null `rate_limits` line, pretty-printed, for diagnostics.
fn latest_raw_rate_limits_line(home: &Path) -> Option<String> {
    let sessions = home.join("sessions");
    if !sessions.exists() {
        return None;
    }
    for (_, path) in newest_rollout_files(&sessions, MAX_FILES_SCANNED) {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in raw.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(parsed) = serde_json::from_str::<RolloutLine>(line) else {
                continue;
            };
            if parsed.line_type != "event_msg" {
                continue;
            }
            let Some(payload) = &parsed.payload else {
                continue;
            };
            if payload.payload_type != "token_count" {
                continue;
            }
            let has_window = payload
                .rate_limits
                .as_ref()
                .map(|r| r.primary.is_some() || r.secondary.is_some())
                .unwrap_or(false);
            if has_window {
                return serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|v| serde_json::to_string_pretty(&v).ok())
                    .or_else(|| Some(line.to_string()));
            }
        }
    }
    None
}

/// Best-effort Codex CLI version from the newest session's `session_meta` line.
fn codex_cli_version(home: &Path) -> Option<String> {
    let sessions = home.join("sessions");
    let (_, path) = newest_rollout_files(&sessions, 1).into_iter().next()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    for line in raw.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let payload = v.get("payload").unwrap_or(&v);
        if let Some(ver) = payload.get("cli_version").and_then(|x| x.as_str()) {
            return Some(ver.to_string());
        }
    }
    None
}

// ─── Filesystem walking ──────────────────────────────────────────────────────

/// Collect rollout files from the date-partitioned `sessions/` tree (plus any
/// flat legacy ones), sorted newest mtime first, capped at `limit`.
fn newest_rollout_files(sessions: &Path, limit: usize) -> Vec<(SystemTime, PathBuf)> {
    let mut out: Vec<(SystemTime, PathBuf)> = Vec::new();
    // Descend YYYY → MM → DD newest-first, bounded so a huge history can't make
    // us read thousands of day directories. We re-sort by mtime below.
    'outer: for year in subdirs_desc(sessions) {
        for month in subdirs_desc(&year) {
            for day in subdirs_desc(&month) {
                out.extend(read_rollouts(&day));
                if out.len() >= limit.saturating_mul(4) {
                    break 'outer;
                }
            }
        }
    }
    out.extend(read_rollouts(sessions)); // flat legacy layout
    out.sort_by_key(|f| std::cmp::Reverse(f.0)); // newest mtime first
    out.truncate(limit);
    out
}

fn read_rollouts(dir: &Path) -> Vec<(SystemTime, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_rollout_jsonl(p))
        .map(|p| (mtime(&p), p))
        .collect()
}

fn subdirs_desc(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    dirs
}

fn is_rollout_jsonl(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
        .unwrap_or(false)
}

fn mtime(p: &Path) -> SystemTime {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

// ─── Misc ────────────────────────────────────────────────────────────────────

fn build_client() -> Client {
    Client::builder()
        .user_agent("Headroom/0.1")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("reqwest client builds")
}

/// Build the usage URL, mirroring Codex's own `PathStyle::from_base_url`
/// (openai/codex `backend-client`): the ChatGPT backend (`…/backend-api`) serves
/// it at `/wham/usage`; a bare Codex API host uses `/api/codex/usage`. Getting
/// this wrong 404s in production, so it must match Codex exactly.
fn usage_url(base: &str) -> String {
    if base.contains("/backend-api") {
        format!("{base}/wham/usage")
    } else {
        format!("{base}/api/codex/usage")
    }
}

fn codex_home() -> Option<PathBuf> {
    if let Some(h) = std::env::var_os("CODEX_HOME") {
        let p = PathBuf::from(h);
        if !p.as_os_str().is_empty() {
            return Some(p);
        }
    }
    dirs::home_dir().map(|h| h.join(".codex"))
}

fn parse_iso8601(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn system_time_to_utc(t: SystemTime) -> Option<DateTime<Utc>> {
    let dur = t.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Utc.timestamp_opt(dur.as_secs() as i64, 0).single()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;
    use tempfile::tempdir;
    use wiremock::matchers::{method, path as mock_path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn future_unix() -> i64 {
        (Utc::now() + Duration::hours(3)).timestamp()
    }

    fn rate_limits(primary: f64, secondary: f64, reset: i64) -> serde_json::Value {
        json!({
            "primary": { "used_percent": primary, "window_minutes": 300, "resets_at": reset },
            "secondary": { "used_percent": secondary, "window_minutes": 10080, "resets_at": reset },
            "credits": { "has_credits": true, "unlimited": false, "balance": "$12.34" },
            "plan_type": "pro"
        })
    }

    fn token_total(total: i64) -> serde_json::Value {
        json!({
            "input_tokens": total / 2,
            "cached_input_tokens": 0,
            "output_tokens": total / 4,
            "reasoning_output_tokens": total / 4,
            "total_tokens": total
        })
    }

    fn tc_line(ts: &str, rate_limits: serde_json::Value, total: i64) -> String {
        json!({
            "timestamp": ts,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": { "total_token_usage": token_total(total) },
                "rate_limits": rate_limits
            }
        })
        .to_string()
    }

    fn write_session(home: &Path, day: &str, name: &str, lines: &[String]) {
        let dir = home.join("sessions").join(day);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), lines.join("\n")).unwrap();
    }

    async fn fetch(home: &Path) -> ServiceStatus {
        CodexSource::with_home(home, false)
            .fetch(&Credentials::new())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn interactive_snapshot_maps_two_percent_windows() {
        let dir = tempdir().unwrap();
        let reset = future_unix();
        write_session(
            dir.path(),
            "2026/06/09",
            "rollout-a.jsonl",
            &[tc_line(
                "2026-06-09T12:00:00Z",
                rate_limits(42.0, 17.0, reset),
                1800,
            )],
        );

        let status = fetch(dir.path()).await;
        assert!(matches!(status.state, ServiceState::Active));
        assert_eq!(status.plan, "Pro");
        assert_eq!(status.quotas.len(), 2);

        let five = status
            .quotas
            .iter()
            .find(|q| matches!(q.window, QuotaWindow::FiveHour))
            .unwrap();
        assert_eq!(five.used, 42.0);
        assert_eq!(five.total, 100.0);
        assert!(matches!(five.unit, QuotaUnit::Percent));
        assert_eq!(five.resets_at.timestamp(), reset);

        let weekly = status
            .quotas
            .iter()
            .find(|q| matches!(q.window, QuotaWindow::WeeklyAll))
            .unwrap();
        assert_eq!(weekly.used, 17.0);

        let meta = status.codex_meta.unwrap();
        assert!(matches!(meta.source, CodexSourceKind::Logs));
        assert_eq!(meta.credits_balance.as_deref(), Some("$12.34"));
        assert!(!meta.stale);
        let stats = meta.token_stats.unwrap();
        assert_eq!(stats.total, 1800);
    }

    #[tokio::test]
    async fn exec_null_rate_limits_is_empty_with_note_but_keeps_token_stats() {
        let dir = tempdir().unwrap();
        let line = json!({
            "timestamp": "2026-06-09T12:00:00Z",
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": { "total_token_usage": token_total(900) },
                "rate_limits": null
            }
        })
        .to_string();
        write_session(dir.path(), "2026/06/09", "rollout-a.jsonl", &[line]);

        let status = fetch(dir.path()).await;
        assert!(status.quotas.is_empty(), "null rate_limits → no % rows");
        let meta = status.codex_meta.unwrap();
        assert!(matches!(meta.source, CodexSourceKind::Empty));
        assert!(meta.note.is_some(), "guidance shown when no windows");
        assert_eq!(
            meta.token_stats.unwrap().total,
            900,
            "token stats still work"
        );
    }

    #[tokio::test]
    async fn missing_codex_home_is_needs_setup() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        let err = CodexSource::with_home(&missing, false)
            .fetch(&Credentials::new())
            .await
            .unwrap_err();
        assert!(matches!(err, SourceError::MissingCredentials("codex.home")));
    }

    #[tokio::test]
    async fn stale_snapshot_is_rebased_and_flagged() {
        let dir = tempdir().unwrap();
        let past = (Utc::now() - Duration::hours(1)).timestamp();
        write_session(
            dir.path(),
            "2026/06/09",
            "rollout-a.jsonl",
            &[tc_line(
                "2026-06-09T12:00:00Z",
                rate_limits(73.0, 9.0, past),
                500,
            )],
        );

        let status = fetch(dir.path()).await;
        let five = status
            .quotas
            .iter()
            .find(|q| matches!(q.window, QuotaWindow::FiveHour))
            .unwrap();
        assert_eq!(five.used, 73.0, "used is preserved");
        assert!(
            five.resets_at > Utc::now(),
            "past reset re-based to the future"
        );
        assert!(status.codex_meta.unwrap().stale, "flagged stale");
    }

    #[tokio::test]
    async fn newest_session_across_days_wins() {
        let dir = tempdir().unwrap();
        let reset = future_unix();
        // Older day, written first → older mtime.
        write_session(
            dir.path(),
            "2026/06/08",
            "rollout-old.jsonl",
            &[tc_line(
                "2026-06-08T12:00:00Z",
                rate_limits(10.0, 10.0, reset),
                100,
            )],
        );
        // Newer day, written last → newer mtime → should win.
        write_session(
            dir.path(),
            "2026/06/09",
            "rollout-new.jsonl",
            &[tc_line(
                "2026-06-09T12:00:00Z",
                rate_limits(88.0, 55.0, reset),
                200,
            )],
        );

        let five = fetch(dir.path())
            .await
            .quotas
            .into_iter()
            .find(|q| matches!(q.window, QuotaWindow::FiveHour))
            .unwrap();
        assert_eq!(five.used, 88.0);
    }

    #[tokio::test]
    async fn last_event_in_file_wins() {
        let dir = tempdir().unwrap();
        let reset = future_unix();
        write_session(
            dir.path(),
            "2026/06/09",
            "rollout-a.jsonl",
            &[
                tc_line("2026-06-09T12:00:00Z", rate_limits(20.0, 5.0, reset), 100),
                tc_line("2026-06-09T13:00:00Z", rate_limits(64.0, 30.0, reset), 250),
            ],
        );

        let five = fetch(dir.path())
            .await
            .quotas
            .into_iter()
            .find(|q| matches!(q.window, QuotaWindow::FiveHour))
            .unwrap();
        assert_eq!(five.used, 64.0, "the later (bottom) event wins");
    }

    #[tokio::test]
    async fn corrupt_trailing_line_is_tolerated() {
        let dir = tempdir().unwrap();
        let reset = future_unix();
        let good = tc_line("2026-06-09T12:00:00Z", rate_limits(33.0, 7.0, reset), 100);
        write_session(
            dir.path(),
            "2026/06/09",
            "rollout-a.jsonl",
            &[good, "{ this is not valid json".to_string()],
        );

        let status = fetch(dir.path()).await;
        assert_eq!(status.quotas.len(), 2, "valid snapshot still found");
    }

    #[tokio::test]
    async fn token_stats_sum_across_sessions() {
        let dir = tempdir().unwrap();
        let reset = future_unix();
        write_session(
            dir.path(),
            "2026/06/09",
            "rollout-a.jsonl",
            &[tc_line(
                "2026-06-09T12:00:00Z",
                rate_limits(10.0, 1.0, reset),
                1000,
            )],
        );
        write_session(
            dir.path(),
            "2026/06/09",
            "rollout-b.jsonl",
            &[tc_line(
                "2026-06-09T13:00:00Z",
                rate_limits(20.0, 2.0, reset),
                500,
            )],
        );

        let stats = fetch(dir.path())
            .await
            .codex_meta
            .unwrap()
            .token_stats
            .unwrap();
        assert_eq!(stats.total, 1500, "summed across both sessions");
        assert_eq!(stats.window_label, "last 24h");
    }

    // ── Active /codex/usage endpoint ──

    #[tokio::test]
    async fn live_usage_parses_int_and_iso_reset_at() {
        let server = MockServer::start().await;
        let iso = (Utc::now() + Duration::days(3)).to_rfc3339();
        let body = json!({
            "rate_limit": {
                "primary_window": { "used_percent": 42, "window_minutes": 300, "reset_at": 1893456000 },
                "secondary_window": { "used_percent": 5, "window_minutes": 10080, "reset_at": iso }
            }
        });
        Mock::given(method("GET"))
            .and(mock_path("/api/codex/usage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let dir = tempdir().unwrap();
        let source = CodexSource::with_base_url(dir.path(), server.uri());
        let rows = source.fetch_usage("tok", Some("acct-1")).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].used_percent, 42.0);
        assert_eq!(rows[0].resets_at, Some(1893456000));
        assert!(rows[1].resets_at.is_some(), "ISO reset_at parsed");
    }

    #[tokio::test]
    async fn live_usage_401_is_auth_rejected() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(mock_path("/api/codex/usage"))
            .respond_with(ResponseTemplate::new(401).set_body_string("expired"))
            .mount(&server)
            .await;

        let dir = tempdir().unwrap();
        let source = CodexSource::with_base_url(dir.path(), server.uri());
        let err = source.fetch_usage("tok", None).await.unwrap_err();
        assert!(matches!(err, SourceError::AuthRejected(_)));
    }

    #[tokio::test]
    async fn live_usage_prefers_codex_tagged_limit() {
        let server = MockServer::start().await;
        let body = json!({
            "rate_limit": { "primary_window": { "used_percent": 10, "reset_at": 1893456000 } },
            "additional_rate_limits": [
                { "limit_id": "other", "rate_limit": { "primary_window": { "used_percent": 99, "reset_at": 1893456000 } } },
                { "limit_id": "codex", "rate_limit": {
                    "primary_window": { "used_percent": 55, "reset_at": 1893456000 },
                    "secondary_window": { "used_percent": 7, "reset_at": 1893456000 } } }
            ]
        });
        Mock::given(method("GET"))
            .and(mock_path("/api/codex/usage"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let dir = tempdir().unwrap();
        let source = CodexSource::with_base_url(dir.path(), server.uri());
        let rows = source.fetch_usage("tok", None).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].used_percent, 55.0,
            "the codex-tagged limit wins over the default/other limits"
        );
    }

    // ── Pure helpers ──

    #[test]
    fn usage_url_matches_codex_path_style() {
        // Production ChatGPT backend → /wham/usage (NOT /codex/usage).
        assert_eq!(
            usage_url("https://chatgpt.com/backend-api"),
            "https://chatgpt.com/backend-api/wham/usage"
        );
        // A bare Codex API host → /api/codex/usage.
        assert_eq!(
            usage_url("https://example.com"),
            "https://example.com/api/codex/usage"
        );
    }

    #[test]
    fn plan_label_brand_cases() {
        assert_eq!(plan_label("pro"), "Pro");
        assert_eq!(plan_label("plus"), "Plus");
        assert_eq!(plan_label("team"), "Team");
        assert_eq!(plan_label("chatgpt_paid"), "Chatgpt Paid");
        assert_eq!(plan_label(""), "");
    }

    #[test]
    fn jwt_claims_decode_account_and_plan() {
        // Hand-built unsigned JWT: header.payload.signature (payload base64url).
        let payload = json!({
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct-xyz",
                "chatgpt_plan_type": "plus"
            }
        })
        .to_string();
        let b64 = b64url_encode(payload.as_bytes());
        let jwt = format!("aaa.{b64}.bbb");
        let claims = decode_jwt_claims(&jwt).unwrap();
        assert_eq!(claims.account_id.as_deref(), Some("acct-xyz"));
        assert_eq!(claims.plan.as_deref(), Some("plus"));
    }

    #[test]
    fn unix_from_value_handles_int_and_string() {
        assert_eq!(unix_from_value(json!(1893456000)), Some(1893456000));
        assert_eq!(unix_from_value(json!("1893456000")), Some(1893456000));
        assert_eq!(
            unix_from_value(json!("2030-01-01T00:00:00Z")),
            Some(1893456000)
        );
        assert_eq!(unix_from_value(json!(null)), None);
    }

    /// base64url-encode (no padding) — test helper to build a JWT payload.
    fn b64url_encode(bytes: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            let chars = [
                ALPHABET[((n >> 18) & 63) as usize],
                ALPHABET[((n >> 12) & 63) as usize],
                ALPHABET[((n >> 6) & 63) as usize],
                ALPHABET[(n & 63) as usize],
            ];
            let take = chunk.len() + 1;
            for &c in chars.iter().take(take) {
                out.push(c as char);
            }
        }
        out
    }
}
