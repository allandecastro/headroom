//! Headroom — menu bar quota meter.
//!
//! See SPEC.md at the project root for the full architecture.

mod credentials;
mod settings;
mod sources;
mod tray;

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::credentials::Credentials;
use crate::settings::Settings;
use crate::sources::{QuotaSource, ServiceStatus};

/// Aggregate snapshot emitted to the renderer on each poll.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub polled_at: i64,
    pub services: Vec<ServiceStatus>,
}

/// Shared app state. Wrapped in `Arc<RwLock<_>>` so the polling task
/// and IPC handlers can both touch it without contention.
pub struct AppState {
    pub credentials: Credentials,
    pub sources: Vec<Arc<dyn QuotaSource>>,
    pub last_snapshot: RwLock<Option<Snapshot>>,
    pub settings: RwLock<Settings>,
    /// Highest notification threshold (0/80/95) already fired per quota, keyed
    /// by "service_id:quota_label", so we alert once per threshold crossing.
    pub notified: RwLock<std::collections::HashMap<String, u8>>,
}

#[tauri::command]
async fn refresh_all(state: tauri::State<'_, Arc<AppState>>) -> Result<Snapshot, String> {
    let snapshot = poll_once(state.inner().clone()).await;
    Ok(snapshot)
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Store the Claude `sessionKey` cookie value in the keychain.
#[tauri::command]
fn set_claude_session(
    state: tauri::State<'_, Arc<AppState>>,
    session_key: String,
) -> Result<(), String> {
    state
        .credentials
        .set("claude.session", &session_key)
        .map_err(|e| e.to_string())
}

/// Store the Copilot Bearer token (OAuth or PAT) in the keychain.
#[tauri::command]
fn set_copilot_token(state: tauri::State<'_, Arc<AppState>>, token: String) -> Result<(), String> {
    state
        .credentials
        .set("copilot.token", &token)
        .map_err(|e| e.to_string())
}

/// Store the GitHub username used in the Copilot billing API path.
#[tauri::command]
fn set_copilot_username(
    state: tauri::State<'_, Arc<AppState>>,
    username: String,
) -> Result<(), String> {
    state
        .credentials
        .set("copilot.username", &username)
        .map_err(|e| e.to_string())
}

/// Store the Copilot plan tier. Rejects anything other than the recognized
/// tiers (`free` | `pro` | `pro_plus`) so the monthly-cap lookup stays valid.
#[tauri::command]
fn set_copilot_plan(state: tauri::State<'_, Arc<AppState>>, plan: String) -> Result<(), String> {
    if !credentials::is_valid_copilot_plan(&plan) {
        return Err(format!("invalid plan: {plan}"));
    }
    state
        .credentials
        .set("copilot.plan", &plan)
        .map_err(|e| e.to_string())
}

/// Clear every credential stored under a service's prefix (`claude` | `copilot`).
#[tauri::command]
fn clear_credentials(
    state: tauri::State<'_, Arc<AppState>>,
    service: String,
) -> Result<(), String> {
    let keys = credentials::service_keys(&service)?;
    for key in keys {
        state.credentials.delete(key).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Return the current user settings.
#[tauri::command]
async fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<Settings, String> {
    Ok(state.settings.read().await.clone())
}

/// Persist user settings (sanitized) and apply them to the running app. The
/// poll loop reads `poll_interval_secs` from this state on its next tick.
#[tauri::command]
async fn set_settings(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    settings: Settings,
) -> Result<(), String> {
    let settings = settings.sanitized();
    settings.save().map_err(|e| e.to_string())?;
    *state.settings.write().await = settings.clone();
    // Let every window react (e.g. live theme switch).
    let _ = app.emit("settings-updated", &settings);
    Ok(())
}

/// Show the onboarding window (used by the Settings "Re-authenticate" action).
#[tauri::command]
fn open_onboarding(app: AppHandle) {
    if let Some(window) = app.get_webview_window("onboarding") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Show the settings window (used by the popover's settings button).
#[tauri::command]
fn open_settings(app: AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Whether Headroom is registered to launch at login.
#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Enable or disable launching Headroom at login.
#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| e.to_string())
}

/// Open an embedded Claude login window. The webview is a real browser engine,
/// so it clears the Cloudflare challenge our HTTP client cannot. After the user
/// signs in, poll the webview cookie store for the `sessionKey`, store it, close
/// the window, and emit `claude-signed-in` + a fresh snapshot.
#[tauri::command]
async fn start_claude_signin(
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

async fn poll_once(state: Arc<AppState>) -> Snapshot {
    let mut services = Vec::with_capacity(state.sources.len());

    for source in &state.sources {
        let id = source.id();
        match tokio::time::timeout(Duration::from_secs(10), source.fetch(&state.credentials)).await
        {
            Ok(Ok(status)) => services.push(status),
            Ok(Err(sources::SourceError::MissingCredentials(_))) => {
                // Not an error state — the user simply hasn't connected this
                // service yet. Surface a "needs setup" card, not "unreachable".
                services.push(ServiceStatus::not_configured(id, source.name()));
            }
            Ok(Err(e)) => {
                warn!(source = id, error = ?e, "source fetch failed");
                services.push(ServiceStatus::unreachable(id, source.name(), e.to_string()));
            }
            Err(_) => {
                warn!(source = id, "source fetch timed out");
                services.push(ServiceStatus::unreachable(
                    id,
                    source.name(),
                    "timeout after 10s".to_string(),
                ));
            }
        }
    }

    let snapshot = Snapshot {
        polled_at: chrono::Utc::now().timestamp(),
        services,
    };

    *state.last_snapshot.write().await = Some(snapshot.clone());
    snapshot
}

/// The highest enabled threshold a usage percentage has crossed (returns the
/// threshold's own percentage value), or 0 if none. A threshold of 0 is "off",
/// and the critical threshold takes precedence over the warning one.
fn crossed_threshold(pct: f64, warn_pct: u8, crit_pct: u8) -> u8 {
    if crit_pct > 0 && pct >= crit_pct as f64 {
        crit_pct
    } else if warn_pct > 0 && pct >= warn_pct as f64 {
        warn_pct
    } else {
        0
    }
}

/// Fire a desktop notification the first time an active quota crosses an
/// enabled threshold, once per crossing: an orange "heads up" at the warning
/// threshold, a red "critical" at the critical threshold. State resets when the
/// quota drops back below both, so a later re-crossing alerts again.
async fn notify_thresholds(
    app: &AppHandle,
    state: &Arc<AppState>,
    snapshot: &Snapshot,
    settings: &Settings,
) {
    use tauri_plugin_notification::NotificationExt;

    let warn = settings.notify_warn_pct;
    let crit = settings.notify_crit_pct;
    if warn == 0 && crit == 0 {
        return;
    }

    let mut notified = state.notified.write().await;
    for service in &snapshot.services {
        if !matches!(service.state, crate::sources::ServiceState::Active) {
            continue;
        }
        for quota in &service.quotas {
            if quota.total <= 0.0 {
                continue;
            }
            let pct = (quota.used / quota.total) * 100.0;
            let key = format!("{}:{}", service.id, quota.label);

            let crossed = crossed_threshold(pct, warn, crit);
            let last = notified.get(&key).copied().unwrap_or(0);

            if crossed == 0 {
                // Back below every enabled threshold — re-arm.
                notified.insert(key, 0);
                continue;
            }
            if crossed > last {
                // Critical (red) when the crit threshold is what we crossed.
                let is_crit = crit > 0 && crossed == crit;
                let (marker, level) = if is_crit {
                    ("🔴", "Critical")
                } else {
                    ("🟠", "Heads up")
                };
                let body = format!(
                    "{marker} {level}: {} · {} at {:.0}%",
                    service.name, quota.label, pct
                );
                if let Err(e) = app
                    .notification()
                    .builder()
                    .title("Headroom")
                    .body(body)
                    .show()
                {
                    warn!(?e, "failed to show notification");
                }
                notified.insert(key, crossed);
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "headroom=info,warn".into()),
        )
        .init();

    let state = Arc::new(AppState {
        credentials: Credentials::new(),
        sources: vec![
            Arc::new(sources::claude::ClaudeSource::default()),
            Arc::new(sources::copilot::CopilotSource::default()),
        ],
        last_snapshot: RwLock::new(None),
        settings: RwLock::new(Settings::load()),
        notified: RwLock::new(std::collections::HashMap::new()),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            refresh_all,
            quit_app,
            set_claude_session,
            set_copilot_token,
            set_copilot_username,
            set_copilot_plan,
            clear_credentials,
            get_settings,
            set_settings,
            open_onboarding,
            open_settings,
            get_autostart,
            set_autostart,
            start_claude_signin
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            tray::install(app)?;

            // First-run routing: with no stored credentials, surface the
            // onboarding window rather than starting silently in the tray.
            // The poll loop still runs (harmlessly reporting MissingCredentials)
            // so it picks up credentials as soon as onboarding writes them.
            let has_credentials = state.credentials.claude_session().is_some()
                || state.credentials.copilot_token().is_some();
            if !has_credentials {
                if let Some(window) = app.get_webview_window("onboarding") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            // Tray-app lifecycle: closing a window hides it instead of
            // destroying it, so it can be reopened from the tray and closing a
            // window never quits the app (only the tray "Quit" does).
            for label in ["popover", "onboarding", "settings"] {
                if let Some(window) = app.get_webview_window(label) {
                    let w = window.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                            let _ = w.hide();
                        }
                    });
                }
            }

            // Background polling loop
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                info!("starting polling loop");
                loop {
                    let snapshot = poll_once(state_clone.clone()).await;
                    if let Err(e) = handle.emit("tokens-updated", &snapshot) {
                        error!(?e, "failed to emit tokens-updated");
                    }
                    let settings = state_clone.settings.read().await.clone();
                    tray::update_state(&handle, &snapshot, settings.show_tray_percentage);
                    notify_thresholds(&handle, &state_clone, &snapshot, &settings).await;
                    tokio::time::sleep(Duration::from_secs(settings.poll_interval_secs)).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::crossed_threshold;

    #[test]
    fn thresholds_respect_configured_values_and_precedence() {
        // Defaults: warn 80, crit 95. Crit wins when both are crossed.
        assert_eq!(crossed_threshold(96.0, 80, 95), 95);
        assert_eq!(crossed_threshold(85.0, 80, 95), 80);
        assert_eq!(crossed_threshold(50.0, 80, 95), 0);
        // Boundaries are inclusive.
        assert_eq!(crossed_threshold(95.0, 80, 95), 95);
        assert_eq!(crossed_threshold(80.0, 80, 95), 80);
        // A threshold of 0 is "off".
        assert_eq!(crossed_threshold(99.0, 80, 0), 80);
        assert_eq!(crossed_threshold(99.0, 0, 95), 95);
        assert_eq!(crossed_threshold(99.0, 0, 0), 0);
        assert_eq!(crossed_threshold(85.0, 0, 95), 0);
        // Custom thresholds are honored.
        assert_eq!(crossed_threshold(72.0, 70, 90), 70);
        assert_eq!(crossed_threshold(91.0, 70, 90), 90);
    }
}
