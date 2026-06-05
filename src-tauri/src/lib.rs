//! Headroom — menu bar quota meter.
//!
//! See SPEC.md at the project root for the full architecture.

mod commands;
mod credentials;
mod github_signin;
mod history;
mod notifications;
mod orchestrator;
mod projection;
mod settings;
mod sources;
mod tray;
mod updates;

use std::sync::Arc;

use serde::Serialize;
use tauri::Manager;
use tokio::sync::RwLock;

use crate::credentials::Credentials;
use crate::settings::Settings;
use crate::sources::{QuotaSource, ServiceStatus};

/// Aggregate snapshot emitted to the renderer on each poll.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub polled_at: i64,
    pub services: Vec<ServiceStatus>,
}

/// Shared app state, touched by both the polling task and the IPC handlers.
pub struct AppState {
    pub credentials: Credentials,
    /// The live source list: Claude plus one Copilot source per connected
    /// account. Rebuilt by the orchestrator when accounts change, so it's behind
    /// a lock rather than fixed at startup.
    pub sources: RwLock<Vec<Arc<dyn QuotaSource>>>,
    pub last_snapshot: RwLock<Option<Snapshot>>,
    pub settings: RwLock<Settings>,
    /// Highest threshold (0/warn/crit) already fired per "service:quota", so we
    /// alert once per crossing.
    pub notified: RwLock<std::collections::HashMap<String, u8>>,
    /// Persisted per-quota usage history, for the popover sparkline.
    pub history: RwLock<history::History>,
    /// Polls GitHub Releases for a newer build.
    pub update_checker: updates::UpdateChecker,
    /// Latest update found (None = up to date or not yet checked).
    pub update: RwLock<Option<updates::UpdateInfo>>,
    /// Unix seconds of the last update check, to throttle GitHub API hits.
    pub last_update_check: RwLock<i64>,
    /// Pulsed by a manual "Refresh now" so the polling loop resets its interval
    /// from that moment instead of continuing on its old schedule.
    pub refresh_notify: tokio::sync::Notify,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "headroom=info,warn".into()),
        )
        .init();

    let credentials = Credentials::new();
    // Initial source list from whatever's already connected; the poll loop
    // migrates any legacy token and rebuilds before the first fetch.
    let sources = orchestrator::build_sources(&credentials);
    let state = Arc::new(AppState {
        credentials,
        sources: RwLock::new(sources),
        last_snapshot: RwLock::new(None),
        settings: RwLock::new(Settings::load()),
        notified: RwLock::new(std::collections::HashMap::new()),
        history: RwLock::new(history::History::load()),
        update_checker: updates::UpdateChecker::default(),
        update: RwLock::new(None),
        last_update_check: RwLock::new(0),
        refresh_notify: tokio::sync::Notify::new(),
    });

    tauri::Builder::default()
        // Must be the first plugin: a second launch is redirected here to
        // surface the running instance instead of starting a new one.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A relaunch means the user wants to interact: surface onboarding
            // when signed out (so they can reconnect) and the popover otherwise.
            // Showing the empty popover while signed out looks like nothing
            // happened — see #23.
            let state = app.state::<Arc<AppState>>();
            let has_credentials = state.credentials.claude_session().is_some()
                || state.credentials.copilot_token().is_some();
            let label = if has_credentials {
                "popover"
            } else {
                "onboarding"
            };
            if let Some(window) = app.get_webview_window(label) {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::refresh_all,
            commands::quit_app,
            commands::set_claude_session,
            commands::set_copilot_token,
            commands::clear_credentials,
            commands::get_settings,
            commands::set_settings,
            commands::open_onboarding,
            commands::open_settings,
            commands::get_autostart,
            commands::set_autostart,
            commands::start_claude_signin,
            commands::start_copilot_signin,
            commands::list_copilot_accounts,
            commands::remove_copilot_account,
            commands::set_copilot_account_label,
            commands::get_update,
            commands::check_for_update_now,
            commands::install_update,
            commands::copilot_diagnostics,
            commands::claude_diagnostics
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            tray::install(app)?;

            // First-run routing: with no stored credentials, show onboarding
            // rather than starting silently in the tray.
            let has_credentials = state.credentials.claude_session().is_some()
                || state.credentials.copilot_token().is_some();
            if !has_credentials {
                if let Some(window) = app.get_webview_window("onboarding") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            // Closing a window hides it (so the tray can reopen it); only the
            // tray "Quit" exits the app.
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

            // Refresh stale autostart path on each release-build launch — see #20.
            #[cfg(not(debug_assertions))]
            {
                use tauri_plugin_autostart::ManagerExt;
                let manager = app.autolaunch();
                if manager.is_enabled().unwrap_or(false) {
                    if let Err(e) = manager.enable() {
                        tracing::warn!("autostart self-heal failed: {e}");
                    }
                }
            }

            orchestrator::spawn_poll_loop(handle, state.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
