//! Credentials wrapper around the OS keychain.
//!
//! All values live under the service name `headroom`. Per-account keys:
//!
//! - `claude.session`   — the `sessionKey` cookie value
//! - `claude.orgId`     — cached Claude organization UUID
//! - `copilot.token`    — GitHub Bearer token (any classic/OAuth token)
//!
//! Multiple GitHub Copilot accounts are supported. Each connected account is
//! keyed by its GitHub user id:
//!
//! - `copilot.accounts`     — JSON registry: `[{ id, login, label }, …]`
//! - `copilot.token.<id>`   — that account's GitHub Bearer token
//!
//! `copilot.token` (no id) is the pre-multi-account single-token key; it's read
//! only for one-time migration into the registry. `copilot.username` /
//! `copilot.plan` are older legacy keys from the original billing API; all three
//! stay in the sign-out cleanup so upgraded installs are wiped clean.

use keyring::Entry;
use serde::{Deserialize, Serialize};
use tracing::warn;

const SERVICE: &str = "headroom";
const COPILOT_ACCOUNTS_KEY: &str = "copilot.accounts";

/// A connected GitHub Copilot account. `id` is the GitHub user id (stable across
/// login renames); `label` is the user-facing name shown on the card, defaulting
/// to the login but renameable (e.g. to the org it belongs to).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotAccount {
    pub id: String,
    pub login: String,
    pub label: String,
}

fn copilot_token_key(id: &str) -> String {
    format!("copilot.token.{id}")
}

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

    /// The legacy single-token value (pre-multi-account). Read only for migration
    /// and as a fallback source for diagnostics.
    pub fn copilot_token(&self) -> Option<String> {
        get("copilot.token")
    }

    /// The connected Copilot accounts, newest last. Empty when none are connected
    /// (or only a not-yet-migrated legacy token exists).
    pub fn copilot_accounts(&self) -> Vec<CopilotAccount> {
        get(COPILOT_ACCOUNTS_KEY)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn copilot_token_for(&self, id: &str) -> Option<String> {
        get(&copilot_token_key(id))
    }

    fn write_copilot_accounts(&self, accounts: &[CopilotAccount]) -> anyhow::Result<()> {
        self.set(COPILOT_ACCOUNTS_KEY, &serde_json::to_string(accounts)?)
    }

    /// Add or update a Copilot account and store its token. Re-adding an existing
    /// id refreshes the token and login but keeps any user-set label.
    pub fn add_copilot_account(&self, account: CopilotAccount, token: &str) -> anyhow::Result<()> {
        self.set(&copilot_token_key(&account.id), token)?;
        let mut accounts = self.copilot_accounts();
        upsert_account(&mut accounts, account);
        self.write_copilot_accounts(&accounts)
    }

    /// Remove a Copilot account and delete its token.
    pub fn remove_copilot_account(&self, id: &str) -> anyhow::Result<()> {
        self.delete(&copilot_token_key(id))?;
        let mut accounts = self.copilot_accounts();
        accounts.retain(|a| a.id != id);
        self.write_copilot_accounts(&accounts)
    }

    /// Rename a Copilot account's display label.
    pub fn set_copilot_label(&self, id: &str, label: &str) -> anyhow::Result<()> {
        let mut accounts = self.copilot_accounts();
        if let Some(a) = accounts.iter_mut().find(|a| a.id == id) {
            a.label = label.to_string();
        }
        self.write_copilot_accounts(&accounts)
    }

    /// Per-account token keys currently in the registry — for sign-out cleanup.
    pub fn copilot_account_token_keys(&self) -> Vec<String> {
        self.copilot_accounts()
            .iter()
            .map(|a| copilot_token_key(&a.id))
            .collect()
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

/// Insert `account`, or update the matching id in place. A re-add refreshes the
/// login and only fills the label when the existing one is blank, so a user's
/// rename survives re-authentication.
fn upsert_account(accounts: &mut Vec<CopilotAccount>, account: CopilotAccount) {
    match accounts.iter_mut().find(|a| a.id == account.id) {
        Some(existing) => {
            existing.login = account.login;
            if existing.label.trim().is_empty() {
                existing.label = account.label;
            }
        }
        None => accounts.push(account),
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

/// The *static* keychain keys owned by a service, used to clear credentials on
/// sign-out. For Copilot the per-account `copilot.token.<id>` keys are dynamic —
/// enumerate them via [`Credentials::copilot_account_token_keys`] in addition to
/// these. Returns an error for an unknown service name. The legacy
/// `copilot.token` / `copilot.username` / `copilot.plan` keys are listed so
/// sign-out wipes them from pre-migration installs.
pub fn service_keys(service: &str) -> Result<&'static [&'static str], String> {
    match service {
        "claude" => Ok(&["claude.session", "claude.orgId"]),
        "copilot" => Ok(&[
            "copilot.accounts",
            "copilot.token",
            "copilot.username",
            "copilot.plan",
        ]),
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
            &[
                "copilot.accounts",
                "copilot.token",
                "copilot.username",
                "copilot.plan"
            ]
        );
        assert!(copilot.iter().all(|k| k.starts_with("copilot.")));
    }

    #[test]
    fn service_keys_rejects_unknown_service() {
        let err = service_keys("cursor").unwrap_err();
        assert!(err.contains("cursor"), "error should name the service");
    }

    fn acct(id: &str, login: &str, label: &str) -> CopilotAccount {
        CopilotAccount {
            id: id.into(),
            login: login.into(),
            label: label.into(),
        }
    }

    #[test]
    fn upsert_appends_new_accounts() {
        let mut accounts = vec![acct("1", "alice", "alice")];
        upsert_account(&mut accounts, acct("2", "bob", "bob"));
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[1].id, "2");
    }

    #[test]
    fn upsert_refreshes_login_but_keeps_user_label() {
        // alice renamed her account to "Acme Corp"; a re-auth must not clobber it,
        // but should pick up a changed GitHub login.
        let mut accounts = vec![acct("1", "alice", "Acme Corp")];
        upsert_account(&mut accounts, acct("1", "alice-acme", "alice-acme"));
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].login, "alice-acme");
        assert_eq!(accounts[0].label, "Acme Corp");
    }

    #[test]
    fn upsert_fills_blank_label_on_reauth() {
        let mut accounts = vec![acct("1", "alice", "   ")];
        upsert_account(&mut accounts, acct("1", "alice", "alice"));
        assert_eq!(accounts[0].label, "alice");
    }
}
