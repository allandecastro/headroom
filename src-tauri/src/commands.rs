//! Tauri IPC command handlers exposed to the renderer.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;
use tracing::{error, warn};

use crate::orchestrator::poll_once;
use crate::settings::Settings;
use crate::{credentials, AppState, Snapshot};

#[tauri::command]
pub async fn refresh_all(state: tauri::State<'_, Arc<AppState>>) -> Result<Snapshot, String> {
    Ok(poll_once(state.inner().clone()).await)
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn set_claude_session(
    state: tauri::State<'_, Arc<AppState>>,
    session_key: String,
) -> Result<(), String> {
    state
        .credentials
        .set("claude.session", &session_key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_copilot_token(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
) -> Result<(), String> {
    state
        .credentials
        .set("copilot.token", &token)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_credentials(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    service: String,
) -> Result<(), String> {
    for key in credentials::service_keys(&service)? {
        state.credentials.delete(key).map_err(|e| e.to_string())?;
    }
    // Claude's session is a webview cookie, not just a keychain entry. Clear the
    // webview's data so the next sign-in starts from a clean session instead of
    // silently reusing the old account — see #23.
    if service == "claude" {
        if let Some(window) = app.get_webview_window("popover") {
            if let Err(e) = window.clear_all_browsing_data() {
                warn!(?e, "failed to clear webview data on sign-out");
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<Settings, String> {
    Ok(state.settings.read().await.clone())
}

#[tauri::command]
pub async fn set_settings(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    settings: Settings,
) -> Result<(), String> {
    let settings = settings.sanitized();
    settings.save().map_err(|e| e.to_string())?;
    *state.settings.write().await = settings.clone();
    // Let open windows react live (theme switch, Claude Design toggle, …).
    let _ = app.emit("settings-updated", &settings);
    Ok(())
}

#[tauri::command]
pub fn open_onboarding(app: AppHandle) {
    show_window(&app, "onboarding");
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    show_window(&app, "settings");
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    // Dev builds would register target\debug\headroom.exe — see #20.
    if enabled && cfg!(debug_assertions) {
        return Err(
            "Launch at startup can only be enabled from an installed Headroom build, \
             not a dev build."
                .to_string(),
        );
    }
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| e.to_string())
}

/// Open an embedded Claude login window. The webview is a real browser engine,
/// so it clears the Cloudflare challenge our HTTP client cannot. After sign-in,
/// poll the cookie store for `sessionKey`, store it, and emit a fresh snapshot.
#[tauri::command]
pub async fn start_claude_signin(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    if let Some(existing) = app.get_webview_window("claude-login") {
        let _ = existing.set_focus();
        return Ok(());
    }

    // Present a mainstream desktop-Chrome UA so identity providers (notably
    // Google SSO) don't reject the embedded webview as an "insecure browser".
    const LOGIN_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
         (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

    let url = "https://claude.ai/login"
        .parse()
        .map_err(|e| format!("bad login URL: {e}"))?;
    WebviewWindowBuilder::new(&app, "claude-login", WebviewUrl::External(url))
        .title("Sign in to Claude")
        .inner_size(520.0, 720.0)
        .center()
        .user_agent(LOGIN_UA)
        .build()
        .map_err(|e| e.to_string())?;

    // cookies() deadlocks on Windows if called from a sync command or the main
    // thread, so poll it from a background task.
    let state = state.inner().clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        for _ in 0..200 {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let Some(window) = app_handle.get_webview_window("claude-login") else {
                break; // user closed the login window
            };
            let cookies = match window.cookies() {
                Ok(c) => c,
                Err(e) => {
                    warn!(?e, "failed to read claude cookies");
                    continue;
                }
            };
            if let Some(cookie) = cookies
                .iter()
                .find(|c| c.name() == "sessionKey" && !c.value().is_empty())
            {
                if let Err(e) = state.credentials.set("claude.session", cookie.value()) {
                    error!(?e, "failed to store claude session key");
                }
                let _ = app_handle.emit("claude-signed-in", ());
                let _ = window.close();
                let snapshot = poll_once(state.clone()).await;
                let _ = app_handle.emit("tokens-updated", &snapshot);
                break;
            }
        }
    });

    Ok(())
}

/// Device-code details returned to the renderer so it can show the user code
/// and open the verification page.
#[derive(serde::Serialize)]
pub struct CopilotSigninStart {
    user_code: String,
    verification_uri: String,
    expires_in: u64,
}

/// Begin the GitHub device-flow sign-in for Copilot. Returns the user code +
/// verification URL immediately, then polls in the background; on success the
/// token is stored under `copilot.token` and `copilot-signed-in` is emitted
/// (or `copilot-signin-error` with a message on failure).
#[tauri::command]
pub async fn start_copilot_signin(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<CopilotSigninStart, String> {
    use crate::github_signin::{GithubSignin, GITHUB_CLIENT_ID};

    let signin = GithubSignin::default();
    let device = signin
        .request_device_code(GITHUB_CLIENT_ID)
        .await
        .map_err(|e| e.to_string())?;

    let info = CopilotSigninStart {
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
        expires_in: device.expires_in,
    };

    let state = state.inner().clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        match signin.poll_for_token(GITHUB_CLIENT_ID, &device).await {
            Ok(token) => {
                if let Err(e) = state.credentials.set("copilot.token", &token) {
                    error!(?e, "failed to store copilot token");
                    let _ = app_handle.emit("copilot-signin-error", "failed to store token");
                    return;
                }
                let _ = app_handle.emit("copilot-signed-in", ());
                let snapshot = poll_once(state.clone()).await;
                let _ = app_handle.emit("tokens-updated", &snapshot);
            }
            Err(e) => {
                warn!(?e, "copilot device-flow sign-in failed");
                let _ = app_handle.emit("copilot-signin-error", e.to_string());
            }
        }
    });

    Ok(info)
}

/// The cached update result (None = up to date or not yet checked), for the UI
/// to render its banner / Settings line on load.
#[tauri::command]
pub async fn get_update(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<crate::updates::UpdateInfo>, String> {
    Ok(state.update.read().await.clone())
}

/// Force an update check now (the Settings "Check for updates" button). Returns
/// the result directly so the UI can show "up to date" vs "update available"
/// immediately. Does not fire a notification — the user is already looking.
#[tauri::command]
pub async fn check_for_update_now(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<crate::updates::UpdateInfo>, String> {
    let result = state
        .update_checker
        .check(crate::updates::CURRENT_VERSION)
        .await
        .map_err(|e| e.to_string())?;
    *state.update.write().await = result.clone();
    *state.last_update_check.write().await = chrono::Utc::now().timestamp();
    if let Some(info) = &result {
        let _ = app.emit("update-available", info);
    }
    Ok(result)
}

/// What the renderer should do after clicking "Update now". A successful in-app
/// install never yields this value — the app restarts into the new version
/// instead. The only variant the renderer sees is the browser fallback (macOS,
/// `.deb`, or when no signed manifest entry is available for this platform).
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallOutcome {
    OpenUrl { url: String },
}

/// Download progress, emitted as the `update-progress` event during install.
#[derive(Clone, serde::Serialize)]
struct UpdateProgress {
    downloaded: u64,
    content_length: Option<u64>,
}

/// Whether the running build can replace itself in place. Windows (MSI) and
/// Linux AppImage can; macOS (needs notarized signing, deferred) and Linux
/// `.deb` (owned by the system package manager) cannot — those fall back to the
/// browser download. A `.deb` install is just a non-AppImage Linux process.
fn self_install_supported() -> bool {
    if cfg!(target_os = "windows") {
        true
    } else if cfg!(target_os = "linux") {
        std::env::var_os("APPIMAGE").is_some()
    } else {
        false
    }
}

/// Download and install the latest release, then relaunch — the "Update now"
/// button. On platforms that can't self-install (and on any updater failure) it
/// returns `OpenUrl` so the renderer opens the release page instead, preserving
/// the old manual flow. Progress is streamed via the `update-progress` event.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<InstallOutcome, String> {
    let fallback_url = state
        .update
        .read()
        .await
        .as_ref()
        .map(|u| u.url.clone())
        .unwrap_or_else(|| crate::updates::RELEASES_PAGE.to_string());

    if !self_install_supported() {
        return Ok(InstallOutcome::OpenUrl { url: fallback_url });
    }

    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = match updater.check().await {
        Ok(Some(update)) => update,
        // No signed manifest entry for this platform (or not actually newer) —
        // fall back rather than dead-end the user.
        Ok(None) => return Ok(InstallOutcome::OpenUrl { url: fallback_url }),
        Err(e) => {
            warn!(?e, "updater check failed; falling back to browser download");
            return Ok(InstallOutcome::OpenUrl { url: fallback_url });
        }
    };

    // `on_chunk` must be `Fn`, so accumulate through an atomic rather than a
    // captured `mut`.
    let downloaded = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let progress_app = app.clone();
    let counter = downloaded.clone();
    update
        .download_and_install(
            move |chunk, content_length| {
                let total = counter.fetch_add(chunk as u64, std::sync::atomic::Ordering::Relaxed)
                    + chunk as u64;
                let _ = progress_app.emit(
                    "update-progress",
                    UpdateProgress {
                        downloaded: total,
                        content_length,
                    },
                );
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    // Installed — relaunch into the new version. `restart` diverges.
    app.restart();
}

/// Diagnostics: fetch the raw `copilot_internal/user` JSON (token redacted) so a
/// user can paste their real payload when the parser can't classify it. The
/// endpoint is undocumented and changed with the AI-Credits migration, so a live
/// payload is the only ground truth — see `sources::copilot`.
#[tauri::command]
pub async fn copilot_diagnostics(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let token = state
        .credentials
        .copilot_token()
        .ok_or("No Copilot token stored — sign in to GitHub first.")?;
    crate::sources::copilot::CopilotSource::default()
        .fetch_raw(&token)
        .await
        .map_err(|e| e.to_string())
}

/// Diagnostics: fetch the raw Claude `/usage` JSON (sessionKey redacted) — the
/// Claude counterpart to `copilot_diagnostics`, since that endpoint is
/// undocumented too. See `sources::claude`.
#[tauri::command]
pub async fn claude_diagnostics(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let session = state
        .credentials
        .claude_session()
        .ok_or("No Claude session stored — sign in to Claude first.")?;
    let org_id = state.credentials.claude_org_id();
    crate::sources::claude::ClaudeSource::default()
        .fetch_raw(&session, org_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Show (and focus) a named window if it exists.
fn show_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
