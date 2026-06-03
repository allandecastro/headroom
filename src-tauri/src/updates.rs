//! Update **detection** — ask the GitHub Releases API for the newest published
//! release and compare it to the running version.
//!
//! This module only detects a newer build and surfaces `UpdateInfo { version,
//! url }`; the actual download-install-relaunch lives in `commands::install_update`
//! (on `tauri-plugin-updater`). The current version is baked in from `Cargo.toml`
//! at compile time.

use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

const RELEASES_API: &str = "https://api.github.com/repos/allandecastro/headroom/releases/latest";
pub const RELEASES_PAGE: &str = "https://github.com/allandecastro/headroom/releases/latest";

/// The running version, from `Cargo.toml` at build time (e.g. "1.2.1").
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A newer release the user can download. Serialized to the renderer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateInfo {
    /// Latest version, without the leading `v` (e.g. "1.3.0").
    pub version: String,
    /// Release page to open for the download.
    pub url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

pub struct UpdateChecker {
    client: Client,
    api_url: String,
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::build(RELEASES_API.to_string())
    }
}

impl UpdateChecker {
    fn build(api_url: String) -> Self {
        let client = Client::builder()
            // GitHub's API rejects requests without a User-Agent.
            .user_agent(concat!("Headroom/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("reqwest client builds");
        Self { client, api_url }
    }

    /// Override the API URL — used only in tests to point at a mock server.
    #[cfg(test)]
    pub(crate) fn with_api_url(api_url: impl Into<String>) -> Self {
        Self::build(api_url.into())
    }

    /// `Some(UpdateInfo)` when the latest published release is newer than
    /// `current`, `None` when up to date. Errors are returned, never panicked —
    /// a check failure must never take down the poll loop.
    pub async fn check(&self, current: &str) -> anyhow::Result<Option<UpdateInfo>> {
        let release: GithubRelease = self
            .client
            .get(&self.api_url)
            .header(header::ACCEPT, "application/vnd.github+json")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // `/releases/latest` already excludes drafts and prereleases, but guard
        // in case the endpoint or repo settings ever change.
        if release.draft || release.prerelease {
            return Ok(None);
        }

        let latest = release.tag_name.trim_start_matches('v').to_string();
        if !is_newer(current, &latest) {
            return Ok(None);
        }
        let url = if release.html_url.is_empty() {
            RELEASES_PAGE.to_string()
        } else {
            release.html_url
        };
        Ok(Some(UpdateInfo {
            version: latest,
            url,
        }))
    }
}

/// `true` when `latest` is a strictly higher `major.minor.patch` than `current`.
/// Missing or non-numeric components count as 0, and any pre-release/build
/// suffix on a component is ignored ("1.3.0-rc1" → (1, 3, 0)).
fn is_newer(current: &str, latest: &str) -> bool {
    parse(latest) > parse(current)
}

fn parse(v: &str) -> (u64, u64, u64) {
    let mut parts = v.trim_start_matches('v').split('.');
    let next = |p: &mut std::str::Split<char>| -> u64 {
        p.next()
            .unwrap_or("0")
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0)
    };
    (next(&mut parts), next(&mut parts), next(&mut parts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn is_newer_compares_semver_components() {
        assert!(is_newer("1.2.1", "1.2.2"));
        assert!(is_newer("1.2.1", "1.3.0"));
        assert!(is_newer("1.9.9", "2.0.0"));
        // Multi-digit components compare numerically, not lexically.
        assert!(is_newer("1.2.9", "1.2.10"));
        // Equal or older → not newer.
        assert!(!is_newer("1.2.1", "1.2.1"));
        assert!(!is_newer("1.2.1", "1.2.0"));
        assert!(!is_newer("2.0.0", "1.9.9"));
        // Leading `v` and pre-release suffixes are tolerated.
        assert!(is_newer("v1.2.1", "v1.2.2"));
        assert!(!is_newer("1.3.0", "1.3.0-rc1"));
    }

    #[tokio::test]
    async fn check_returns_update_when_remote_is_newer() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v1.3.0",
                "html_url": "https://example.test/releases/v1.3.0",
                "draft": false,
                "prerelease": false
            })))
            .mount(&server)
            .await;

        let checker = UpdateChecker::with_api_url(format!("{}/releases/latest", server.uri()));
        let info = checker.check("1.2.1").await.unwrap().expect("an update");
        assert_eq!(info.version, "1.3.0");
        assert_eq!(info.url, "https://example.test/releases/v1.3.0");
    }

    #[tokio::test]
    async fn check_returns_none_when_up_to_date() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v1.2.1",
                "html_url": "https://example.test/releases/v1.2.1"
            })))
            .mount(&server)
            .await;

        let checker = UpdateChecker::with_api_url(format!("{}/releases/latest", server.uri()));
        assert!(checker.check("1.2.1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn check_ignores_prereleases() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v2.0.0",
                "html_url": "https://example.test/releases/v2.0.0",
                "prerelease": true
            })))
            .mount(&server)
            .await;

        let checker = UpdateChecker::with_api_url(format!("{}/releases/latest", server.uri()));
        assert!(checker.check("1.2.1").await.unwrap().is_none());
    }
}
