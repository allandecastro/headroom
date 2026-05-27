//! Background polling: fetch every source, broadcast a snapshot, and drive the
//! tray + notifications off the result.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

use crate::notifications::notify_thresholds;
use crate::sources::{self, ServiceStatus};
use crate::tray;
use crate::{AppState, Snapshot};

/// Fetch every source once and persist the resulting snapshot.
pub(crate) async fn poll_once(state: Arc<AppState>) -> Snapshot {
    let mut services = Vec::with_capacity(state.sources.len());

    for source in &state.sources {
        let id = source.id();
        match tokio::time::timeout(Duration::from_secs(10), source.fetch(&state.credentials)).await
        {
            Ok(Ok(status)) => services.push(status),
            Ok(Err(sources::SourceError::MissingCredentials(_))) => {
                // The user simply hasn't connected this service yet — a
                // "needs setup" card, not an "unreachable" error.
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

/// Spawn the polling loop: each tick re-reads settings, so changes to the poll
/// interval take effect on the next cycle without a restart.
pub(crate) fn spawn_poll_loop(handle: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        info!("starting polling loop");
        loop {
            let snapshot = poll_once(state.clone()).await;
            if let Err(e) = handle.emit("tokens-updated", &snapshot) {
                error!(?e, "failed to emit tokens-updated");
            }
            let settings = state.settings.read().await.clone();
            tray::update_state(&handle, &snapshot, settings.show_tray_percentage);
            notify_thresholds(&handle, &state, &snapshot, &settings).await;
            tokio::time::sleep(Duration::from_secs(settings.poll_interval_secs)).await;
        }
    });
}
