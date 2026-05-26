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

    #[allow(dead_code)] // used by the (forthcoming) onboarding IPC handlers
    pub fn set(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let entry = Entry::new(SERVICE, key)?;
        entry.set_password(value)?;
        Ok(())
    }

    #[allow(dead_code)]
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
