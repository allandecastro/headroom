//! Tauri IPC command handlers exposed to the renderer.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
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
pub fn set_copilot_username(
    state: tauri::State<'_, Arc<AppState>>,
    username: String,
) -> Result<(), String> {
    state
        .credentials
        .set("copilot.username", &username)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_copilot_plan(
    state: tauri::State<'_, Arc<AppState>>,
    plan: String,
) -> Result<(), String> {
    // Reject unknown tiers so the monthly-cap lookup stays valid.
    if !credentials::is_valid_copilot_plan(&plan) {
        return Err(format!("invalid plan: {plan}"));
    }
    state
        .credentials
        .set("copilot.plan", &plan)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_credentials(
    state: tauri::State<'_, Arc<AppState>>,
    service: String,
) -> Result<(), String> {
    for key in credentials::service_keys(&service)? {
        state.credentials.delete(key).map_err(|e| e.to_string())?;
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

/// Show (and focus) a named window if it exists.
fn show_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
