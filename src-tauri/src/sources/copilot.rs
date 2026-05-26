//! GitHub Copilot quota source.
//!
//! Endpoint: `GET https://api.github.com/users/{username}/settings/billing/premium_request/usage`
//! with a Bearer token (fine-grained PAT with `Account → Plan → Read-only`, or
//! OAuth access token from device flow with the equivalent scope).
//!
//! See SPEC.md § "Data sources / GitHub Copilot".

use async_trait::async_trait;
use chrono::{Datelike, Utc};
use reqwest::{header, Client};
use serde::Deserialize;

use super::{Quota, QuotaSource, QuotaUnit, QuotaWindow, ServiceState, ServiceStatus, SourceError};
use crate::credentials::Credentials;

pub struct CopilotSource {
    client: Client,
}

impl Default for CopilotSource {
    fn default() -> Self {
        let client = Client::builder()
            .user_agent("Headroom/0.1")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("reqwest client builds");
        Self { client }
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
        let username = creds
            .copilot_username()
            .ok_or(SourceError::MissingCredentials("copilot.username"))?;
        let plan = creds.copilot_plan().unwrap_or_else(|| "pro".to_string());

        let now = Utc::now();
        let url = format!(
            "https://api.github.com/users/{username}/settings/billing/premium_request/usage?year={}&month={}",
            now.year(),
            now.month()
        );

        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
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

        let body: UsageResponse = response
            .json()
            .await
            .map_err(|e| SourceError::Parse(e.to_string()))?;

        let used: f64 = body
            .usage_items
            .iter()
            .filter(|i| i.product == "Copilot")
            .map(|i| i.gross_quantity)
            .sum();

        let total = match plan.as_str() {
            "free" => 50.0,
            "pro" => 300.0,
            "pro_plus" => 1_500.0,
            _ => 300.0,
        };

        // Monthly window resets on the 1st of next month, 00:00 UTC
        let resets_at = next_month_start_utc(now);

        let plan_label = match plan.as_str() {
            "free" => "Free",
            "pro" => "Pro",
            "pro_plus" => "Pro+",
            _ => "—",
        };

        Ok(ServiceStatus {
            id: "copilot".into(),
            name: "GitHub Copilot".into(),
            plan: plan_label.to_string(),
            state: ServiceState::Active,
            quotas: vec![Quota {
                window: QuotaWindow::Monthly,
                label: "Monthly".into(),
                used,
                total,
                unit: QuotaUnit::Requests,
                resets_at,
                advice: None,
            }],
            error_detail: None,
        })
    }
}

fn next_month_start_utc(now: chrono::DateTime<Utc>) -> chrono::DateTime<Utc> {
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
struct UsageResponse {
    #[serde(rename = "usageItems", default)]
    usage_items: Vec<UsageItem>,
}

#[derive(Deserialize)]
struct UsageItem {
    product: String,
    #[serde(rename = "grossQuantity", default)]
    gross_quantity: f64,
}
