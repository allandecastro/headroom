//! Credentials wrapper around the OS keychain.
//!
//! All values live under the service name `headroom`. Per-account keys:
//!
//! - `claude.session`   — the `sessionKey` cookie value
//! - `claude.orgId`     — cached Claude organization UUID
//! - `copilot.token`    — GitHub Bearer token (any classic/OAuth token)
//!
//! `copilot.username` / `copilot.plan` are legacy keys from the old billing API;
//! they're no longer written but stay in [`service_keys`] so sign-out clears
//! them from upgraded installs.

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

/// The keychain keys owned by a service, used to clear every credential under
/// that service's prefix. Returns an error for an unknown service name. The
/// legacy `copilot.username` / `copilot.plan` keys are listed so sign-out wipes
/// them from installs created before the `copilot_internal/user` migration.
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
