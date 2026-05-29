//! Source adapters. Each implements [`QuotaSource`] and produces a [`ServiceStatus`]
//! shaped for the renderer.
//!
//! See SPEC.md § "Data sources" and § "Modules" for the contract.

pub mod claude;
pub mod copilot;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::credentials::Credentials;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Active,
    /// Credentials were never entered — prompt the user to connect, don't alarm
    /// them with an error.
    NeedsSetup,
    AuthRequired,
    Unreachable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaWindow {
    FiveHour,
    WeeklyAll,
    WeeklySonnet,
    WeeklyOpus,
    ClaudeDesign,
    Monthly,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaUnit {
    Messages,
    Hours,
    Requests,
    UsdCredits,
    /// A 0–100 utilization percentage (used == percent, total == 100).
    Percent,
}

#[derive(Debug, Clone, Serialize)]
pub struct Quota {
    pub window: QuotaWindow,
    pub label: String,
    pub used: f64,
    pub total: f64,
    pub unit: QuotaUnit,
    pub resets_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advice: Option<String>,
    /// Burndown projection, filled in by the orchestrator after fetch (sources
    /// leave it `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection: Option<crate::projection::Projection>,
    /// Downsampled recent utilization for the sparkline, filled in by the
    /// orchestrator from persisted history (sources leave it empty).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sparkline: Vec<f64>,
}

impl Quota {
    /// Build a quota with derived fields defaulted — sources only specify the
    /// measurement; `projection`, `sparkline`, and `advice` are filled later.
    pub fn new(
        window: QuotaWindow,
        label: impl Into<String>,
        used: f64,
        total: f64,
        unit: QuotaUnit,
        resets_at: DateTime<Utc>,
    ) -> Self {
        Self {
            window,
            label: label.into(),
            used,
            total,
            unit,
            resets_at,
            advice: None,
            projection: None,
            sparkline: vec![],
        }
    }

    /// Attach a plan-specific recommendation (e.g. "use Sonnet for the rest of
    /// the week" on a critical Opus quota).
    pub fn with_advice(mut self, advice: impl Into<String>) -> Self {
        self.advice = Some(advice.into());
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatus {
    pub id: String,
    pub name: String,
    pub plan: String,
    pub state: ServiceState,
    pub quotas: Vec<Quota>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<String>,
}

impl ServiceStatus {
    pub fn unreachable(id: &str, name: &str, detail: String) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            plan: String::new(),
            state: ServiceState::Unreachable,
            quotas: vec![],
            error_detail: Some(detail),
        }
    }

    pub fn not_configured(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            plan: String::new(),
            state: ServiceState::NeedsSetup,
            quotas: vec![],
            error_detail: None,
        }
    }

    pub fn auth_required(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            plan: String::new(),
            state: ServiceState::AuthRequired,
            quotas: vec![],
            error_detail: Some("Authentication required".to_string()),
        }
    }
}

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("missing credentials for {0}")]
    MissingCredentials(&'static str),

    #[error("authentication rejected (401/403): {0}")]
    AuthRejected(String),

    #[error("cloudflare challenge — manual refresh required")]
    CloudflareChallenge,

    #[error("HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("response parse error: {0}")]
    Parse(String),
}

#[async_trait]
pub trait QuotaSource: Send + Sync + 'static {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    async fn fetch(&self, creds: &Credentials) -> Result<ServiceStatus, SourceError>;
}
