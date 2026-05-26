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

use super::{Quota, QuotaSource, QuotaUnit, QuotaWindow, ServiceState, ServiceStatus, SourceError};
use crate::credentials::Credentials;

const USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
     (KHTML, like Gecko) Version/17.0 Safari/605.1.15";

pub struct ClaudeSource {
    client: Client,
}

impl Default for ClaudeSource {
    fn default() -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("reqwest client builds");
        Self { client }
    }
}

#[async_trait]
impl QuotaSource for ClaudeSource {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn name(&self) -> &'static str {
        "Claude Code"
    }

    async fn fetch(&self, creds: &Credentials) -> Result<ServiceStatus, SourceError> {
        let session_key = creds
            .claude_session()
            .ok_or(SourceError::MissingCredentials("claude.session"))?;

        let org_id = match creds.claude_org_id() {
            Some(id) => id,
            None => self.fetch_org_id(&session_key).await?,
        };

        let url = format!("https://claude.ai/api/organizations/{org_id}/usage");
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
}

impl ClaudeSource {
    async fn fetch_org_id(&self, session_key: &str) -> Result<String, SourceError> {
        let response = self
            .client
            .get("https://claude.ai/api/organizations")
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

#[derive(Deserialize)]
struct OrgEntry {
    uuid: String,
}

/// Tentative shape — the real one will be confirmed by inspecting an
/// authenticated response during implementation. Fields and nesting may
/// need adjustment.
#[derive(Deserialize)]
struct UsageResponse {
    #[serde(rename = "fiveHour")]
    five_hour: Option<WindowEntry>,
    weekly: Option<WindowEntry>,
    #[serde(rename = "weeklyOpus")]
    weekly_opus: Option<WindowEntry>,
    plan: Option<String>,
}

#[derive(Deserialize)]
struct WindowEntry {
    #[serde(rename = "percentUsed")]
    percent_used: f64,
    #[serde(rename = "resetsAt")]
    resets_at: chrono::DateTime<chrono::Utc>,
    #[serde(default, rename = "totalHours")]
    total_hours: Option<f64>,
}

impl UsageResponse {
    fn into_status(self) -> ServiceStatus {
        let mut quotas = Vec::new();

        if let Some(w) = self.five_hour {
            quotas.push(Quota {
                window: QuotaWindow::FiveHour,
                label: "5h".into(),
                used: w.percent_used,
                total: 100.0,
                unit: QuotaUnit::Messages,
                resets_at: w.resets_at,
                advice: None,
            });
        }

        if let Some(w) = self.weekly {
            let total = w.total_hours.unwrap_or(100.0);
            let used = (w.percent_used / 100.0) * total;
            quotas.push(Quota {
                window: QuotaWindow::WeeklySonnet,
                label: "Sonnet · 7d".into(),
                used,
                total,
                unit: QuotaUnit::Hours,
                resets_at: w.resets_at,
                advice: None,
            });
        }

        if let Some(w) = self.weekly_opus {
            let total = w.total_hours.unwrap_or(100.0);
            let used = (w.percent_used / 100.0) * total;
            let advice = if w.percent_used >= 95.0 {
                Some("use Sonnet for the rest of the week".to_string())
            } else {
                None
            };
            quotas.push(Quota {
                window: QuotaWindow::WeeklyOpus,
                label: "Opus · 7d".into(),
                used,
                total,
                unit: QuotaUnit::Hours,
                resets_at: w.resets_at,
                advice,
            });
        }

        ServiceStatus {
            id: "claude".into(),
            name: "Claude Code".into(),
            plan: self.plan.unwrap_or_else(|| "—".into()),
            state: ServiceState::Active,
            quotas,
            error_detail: None,
        }
    }
}
