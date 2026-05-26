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

async fn poll_once(state: Arc<AppState>) -> Snapshot {
    let mut services = Vec::with_capacity(state.sources.len());

    for source in &state.sources {
        let id = source.id();
        match tokio::time::timeout(Duration::from_secs(10), source.fetch(&state.credentials)).await
        {
            Ok(Ok(status)) => services.push(status),
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

/// The highest enabled notification threshold a usage percentage has crossed,
/// or 0 if none. 95 takes precedence over 80.
fn crossed_threshold(pct: f64, notify_80: bool, notify_95: bool) -> u8 {
    if notify_95 && pct >= 95.0 {
        95
    } else if notify_80 && pct >= 80.0 {
        80
    } else {
        0
    }
}

/// Fire a desktop notification the first time an active quota crosses an
/// enabled threshold (80% / 95%), once per crossing. Resets a quota's state
/// when it drops back below 80% so a later re-crossing alerts again.
async fn notify_thresholds(
    app: &AppHandle,
    state: &Arc<AppState>,
    snapshot: &Snapshot,
    settings: &Settings,
) {
    use tauri_plugin_notification::NotificationExt;

    if !settings.notify_80 && !settings.notify_95 {
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

            if pct < 80.0 {
                notified.insert(key, 0);
                continue;
            }

            let crossed = crossed_threshold(pct, settings.notify_80, settings.notify_95);
            let last = notified.get(&key).copied().unwrap_or(0);
            if crossed > last {
                let body = format!("{} · {} at {:.0}%", service.name, quota.label, pct);
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
            open_onboarding
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
    fn thresholds_respect_enabled_flags_and_precedence() {
        // 95 wins over 80 when both enabled.
        assert_eq!(crossed_threshold(96.0, true, true), 95);
        assert_eq!(crossed_threshold(85.0, true, true), 80);
        assert_eq!(crossed_threshold(50.0, true, true), 0);
        // Boundaries are inclusive.
        assert_eq!(crossed_threshold(95.0, true, true), 95);
        assert_eq!(crossed_threshold(80.0, true, true), 80);
        // Disabled flags are never reported.
        assert_eq!(crossed_threshold(99.0, true, false), 80);
        assert_eq!(crossed_threshold(99.0, false, true), 95);
        assert_eq!(crossed_threshold(99.0, false, false), 0);
        assert_eq!(crossed_threshold(85.0, false, true), 0);
    }
}
