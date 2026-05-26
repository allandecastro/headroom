//! Credentials wrapper around the OS keychain.
//!
//! All values live under the service name `headroom`. Per-account keys:
//!
//! - `claude.session`   — the `sessionKey` cookie value
//! - `claude.orgId`     — cached Claude organization UUID
//! - `copilot.token`    — Bearer token (OAuth or PAT)
//! - `copilot.username` — GitHub username (used in the API path)
//! - `copilot.plan`     — plan tier: `free` | `pro` | `pro_plus`

use keyring::Entry;
use tracing::warn;

const SERVICE: &str = "headroom";

pub struct Credentials;

impl Credentials {
    pub fn new() -> Self {
        Self
    }

    pub fn claude_session(&self) -> Option<String> {
        get("claude.session")
    }

    pub fn claude_org_id(&self) -> Option<String> {
        get("claude.orgId")
    }

    pub fn copilot_token(&self) -> Option<String> {
        get("copilot.token")
    }

    pub fn copilot_username(&self) -> Option<String> {
        get("copilot.username")
    }

    pub fn copilot_plan(&self) -> Option<String> {
        get("copilot.plan")
    }

    pub fn set(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let entry = Entry::new(SERVICE, key)?;
        entry.set_password(value)?;
        Ok(())
    }

    pub fn delete(&self, key: &str) -> anyhow::Result<()> {
        let entry = Entry::new(SERVICE, key)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

fn get(key: &str) -> Option<String> {
    match Entry::new(SERVICE, key).and_then(|e| e.get_password()) {
        Ok(v) => Some(v),
        Err(keyring::Error::NoEntry) => None,
        Err(e) => {
            warn!(key, error = ?e, "keychain read failed");
            None
        }
    }
}

/// Copilot plan tiers accepted by the onboarding flow. Must stay in sync with
/// the monthly-cap lookup in `sources::copilot`.
pub const VALID_COPILOT_PLANS: [&str; 3] = ["free", "pro", "pro_plus"];

/// Whether `plan` is a recognized Copilot plan tier.
pub fn is_valid_copilot_plan(plan: &str) -> bool {
    VALID_COPILOT_PLANS.contains(&plan)
}

/// The keychain keys owned by a service, used to clear every credential under
/// that service's prefix. Returns an error for an unknown service name.
pub fn service_keys(service: &str) -> Result<&'static [&'static str], String> {
    match service {
        "claude" => Ok(&["claude.session", "claude.orgId"]),
        "copilot" => Ok(&["copilot.token", "copilot.username", "copilot.plan"]),
        other => Err(format!("unknown service: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_plans_are_accepted() {
        for plan in ["free", "pro", "pro_plus"] {
            assert!(is_valid_copilot_plan(plan), "{plan} should be valid");
        }
    }

    #[test]
    fn invalid_plans_are_rejected() {
        for plan in ["", "Pro", "pro+", "max", "pro_plus_plus", "enterprise"] {
            assert!(!is_valid_copilot_plan(plan), "{plan} should be rejected");
        }
    }

    #[test]
    fn service_keys_cover_each_prefix() {
        let claude = service_keys("claude").unwrap();
        assert_eq!(claude, &["claude.session", "claude.orgId"]);
        assert!(claude.iter().all(|k| k.starts_with("claude.")));

        let copilot = service_keys("copilot").unwrap();
        assert_eq!(
            copilot,
            &["copilot.token", "copilot.username", "copilot.plan"]
        );
        assert!(copilot.iter().all(|k| k.starts_with("copilot.")));
    }

    #[test]
    fn service_keys_rejects_unknown_service() {
        let err = service_keys("cursor").unwrap_err();
        assert!(err.contains("cursor"), "error should name the service");
    }
}
