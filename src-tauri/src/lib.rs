//! Headroom — menu bar quota meter.
//!
//! See SPEC.md at the project root for the full architecture.

mod credentials;
mod sources;
mod tray;

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::credentials::Credentials;
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
            clear_credentials
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            tray::install(app)?;

            // Background polling loop
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                info!("starting polling loop");
                loop {
                    let snapshot = poll_once(state_clone.clone()).await;
                    if let Err(e) = handle.emit("tokens-updated", &snapshot) {
                        error!(?e, "failed to emit tokens-updated");
                    }
                    tray::update_state(&handle, &snapshot);
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
