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

const DEFAULT_BASE_URL: &str = "https://api.github.com";

pub struct CopilotSource {
    client: Client,
    base_url: String,
}

impl Default for CopilotSource {
    fn default() -> Self {
        let client = Client::builder()
            .user_agent("Headroom/0.1")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("reqwest client builds");
        Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }
}

impl CopilotSource {
    /// Override the base URL — used only in tests to point at a [`wiremock`] server.
    #[cfg(test)]
    pub(crate) fn with_base_url(base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .user_agent("Headroom/0.1")
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
    /// Extracted so tests can supply in-memory values without touching the
    /// OS keychain (which `Credentials` exclusively reads from).
    pub(crate) async fn fetch_with(
        &self,
        token: &str,
        username: &str,
        plan: &str,
    ) -> Result<ServiceStatus, SourceError> {
        let now = Utc::now();
        let url = format!(
            "{}/users/{}/settings/billing/premium_request/usage?year={}&month={}",
            self.base_url,
            username,
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

        let total = plan_cap(plan);

        // Monthly window resets on the 1st of next month, 00:00 UTC
        let resets_at = next_month_start_utc(now);

        let plan_label = match plan {
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
                projection: None,
            }],
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
        let username = creds
            .copilot_username()
            .ok_or(SourceError::MissingCredentials("copilot.username"))?;
        let plan = creds.copilot_plan().unwrap_or_else(|| "pro".to_string());

        self.fetch_with(&token, &username, &plan).await
    }
}

/// Monthly request cap for a given Copilot plan tier.
fn plan_cap(plan: &str) -> f64 {
    match plan {
        "free" => 50.0,
        "pro" => 300.0,
        "pro_plus" => 1_500.0,
        _ => 300.0,
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Helper: build a minimal usageItems JSON body.
    fn usage_body(items: &[(&str, f64)]) -> serde_json::Value {
        let items_json: Vec<serde_json::Value> = items
            .iter()
            .map(|(product, qty)| {
                serde_json::json!({
                    "product": product,
                    "grossQuantity": qty
                })
            })
            .collect();
        serde_json::json!({ "usageItems": items_json })
    }

    #[tokio::test]
    async fn successful_fetch_sums_copilot_rows() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex(
                r"^/users/testuser/settings/billing/premium_request/usage",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(usage_body(&[("Copilot", 42.0), ("Copilot", 18.0)])),
            )
            .mount(&server)
            .await;

        let source = CopilotSource::with_base_url(server.uri());
        let status = source.fetch_with("tok", "testuser", "pro").await.unwrap();

        assert_eq!(status.id, "copilot");
        assert!(matches!(status.state, ServiceState::Active));
        assert_eq!(status.quotas.len(), 1);

        let quota = &status.quotas[0];
        assert_eq!(quota.used, 60.0, "grossQuantity rows should be summed");
        assert_eq!(quota.total, 300.0, "pro plan cap is 300");
    }

    #[tokio::test]
    async fn non_copilot_rows_are_excluded_from_sum() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex(
                r"^/users/u/settings/billing/premium_request/usage",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(usage_body(&[
                ("Copilot", 10.0),
                ("Actions", 99.0),
                ("Copilot", 5.0),
            ])))
            .mount(&server)
            .await;

        let source = CopilotSource::with_base_url(server.uri());
        let status = source.fetch_with("tok", "u", "pro").await.unwrap();

        let quota = &status.quotas[0];
        assert_eq!(
            quota.used, 15.0,
            "only Copilot rows should be summed, not Actions"
        );
    }

    #[tokio::test]
    async fn plan_cap_free_is_50() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex(
                r"^/users/u/settings/billing/premium_request/usage",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(usage_body(&[("Copilot", 5.0)])))
            .mount(&server)
            .await;

        let source = CopilotSource::with_base_url(server.uri());
        let status = source.fetch_with("tok", "u", "free").await.unwrap();
        assert_eq!(status.quotas[0].total, 50.0);
        assert_eq!(status.plan, "Free");
    }

    #[tokio::test]
    async fn plan_cap_pro_plus_is_1500() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex(
                r"^/users/u/settings/billing/premium_request/usage",
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(usage_body(&[("Copilot", 100.0)])),
            )
            .mount(&server)
            .await;

        let source = CopilotSource::with_base_url(server.uri());
        let status = source.fetch_with("tok", "u", "pro_plus").await.unwrap();
        assert_eq!(status.quotas[0].total, 1_500.0);
        assert_eq!(status.plan, "Pro+");
    }

    #[tokio::test]
    async fn unknown_plan_defaults_to_300() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex(
                r"^/users/u/settings/billing/premium_request/usage",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(usage_body(&[("Copilot", 1.0)])))
            .mount(&server)
            .await;

        let source = CopilotSource::with_base_url(server.uri());
        let status = source.fetch_with("tok", "u", "enterprise").await.unwrap();
        assert_eq!(status.quotas[0].total, 300.0);
    }

    #[tokio::test]
    async fn http_401_returns_auth_rejected() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex(
                r"^/users/u/settings/billing/premium_request/usage",
            ))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;

        let source = CopilotSource::with_base_url(server.uri());
        let err = source
            .fetch_with("bad-token", "u", "pro")
            .await
            .unwrap_err();

        assert!(
            matches!(err, SourceError::AuthRejected(_)),
            "expected AuthRejected, got {err:?}"
        );
    }

    #[tokio::test]
    async fn http_403_returns_auth_rejected() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex(
                r"^/users/u/settings/billing/premium_request/usage",
            ))
            .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
            .mount(&server)
            .await;

        let source = CopilotSource::with_base_url(server.uri());
        let err = source
            .fetch_with("bad-token", "u", "pro")
            .await
            .unwrap_err();

        assert!(
            matches!(err, SourceError::AuthRejected(_)),
            "expected AuthRejected, got {err:?}"
        );
    }

    // Unit tests for plan_cap that don't need a network mock.
    #[test]
    fn plan_cap_mapping() {
        assert_eq!(plan_cap("free"), 50.0);
        assert_eq!(plan_cap("pro"), 300.0);
        assert_eq!(plan_cap("pro_plus"), 1_500.0);
        assert_eq!(plan_cap("unknown"), 300.0);
    }
}
