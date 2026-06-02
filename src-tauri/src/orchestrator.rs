//! Background polling: fetch every source, broadcast a snapshot, and drive the
//! tray + notifications off the result.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

use crate::notifications::notify_thresholds;
use crate::sources::{QuotaWindow, ServiceState, ServiceStatus, SourceError};
use crate::tray;
use crate::{AppState, Snapshot};

/// Long windows where a 24h sample delta is a meaningful pace signal.
/// FiveHour is intentionally excluded — the lookback would dwarf the window.
fn is_long_window(w: QuotaWindow) -> bool {
    matches!(
        w,
        QuotaWindow::WeeklyAll
            | QuotaWindow::WeeklySonnet
            | QuotaWindow::WeeklyOpus
            | QuotaWindow::ClaudeDesign
            | QuotaWindow::Monthly
    )
}

/// Per-source fetch budget. A source that exceeds it is shown as unreachable.
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the background loop checks GitHub for a newer release. Kept well
/// above the quota poll cadence — releases are rare and GitHub's unauthenticated
/// API allows only ~60 calls/hr.
const UPDATE_CHECK_INTERVAL_SECS: i64 = 6 * 3600;

/// Fetch every source once and persist the resulting snapshot.
pub(crate) async fn poll_once(state: Arc<AppState>) -> Snapshot {
    let mut services = Vec::with_capacity(state.sources.len());

    for source in &state.sources {
        let id = source.id();
        let name = source.name();
        let status =
            match tokio::time::timeout(FETCH_TIMEOUT, source.fetch(&state.credentials)).await {
                Ok(result) => {
                    if let Err(e) = &result {
                        if !matches!(e, SourceError::MissingCredentials(_)) {
                            warn!(source = id, error = ?e, "source fetch failed");
                        }
                    }
                    classify(id, name, result)
                }
                Err(_) => {
                    warn!(source = id, "source fetch timed out");
                    ServiceStatus::unreachable(id, name, "timeout after 10s".to_string())
                }
            };
        services.push(status);
    }

    // Attach the burndown projection + sparkline to each quota (sources leave
    // them empty). Record active quotas into history under the lock, then
    // persist outside it to keep file I/O off the lock.
    let now = chrono::Utc::now();
    let now_secs = now.timestamp();
    let mut to_persist = Vec::new();
    {
        let mut history = state.history.write().await;
        for service in &mut services {
            let id = service.id.clone();
            let active = matches!(service.state, ServiceState::Active);
            for quota in &mut service.quotas {
                let used_pct = if quota.total > 0.0 {
                    (quota.used / quota.total) * 100.0
                } else {
                    0.0
                };
                quota.projection =
                    crate::projection::project(used_pct, quota.window, quota.resets_at, now);
                if active {
                    if let Some(sample) = history.record(&id, quota.window, used_pct, now_secs) {
                        to_persist.push(sample);
                    }
                    quota.sparkline = history.sparkline(&id, quota.window, now_secs);
                    // Recent-burn-rate pace, only for the long (weekly/monthly)
                    // windows where a 24h delta is meaningful.
                    if is_long_window(quota.window) {
                        if let Some(rd) =
                            history.recent_delta(&id, quota.window, now_secs, 24 * 3600)
                        {
                            quota.pace = crate::projection::pace(
                                used_pct,
                                quota.resets_at,
                                now,
                                rd.delta_pct,
                                rd.span_secs,
                                rd.low_confidence,
                            );
                        }
                    }
                }
            }
        }
    }
    for sample in &to_persist {
        crate::history::append_to_file(sample);
    }

    let snapshot = Snapshot {
        polled_at: now.timestamp(),
        services,
    };

    *state.last_snapshot.write().await = Some(snapshot.clone());
    snapshot
}

/// Map a source result to the service card the renderer should show. Missing
/// credentials are "needs setup" (not an error); a rejected token is
/// "auth required" (re-sign-in); everything else is "unreachable".
fn classify(id: &str, name: &str, result: Result<ServiceStatus, SourceError>) -> ServiceStatus {
    match result {
        Ok(status) => status,
        Err(SourceError::MissingCredentials(_)) => ServiceStatus::not_configured(id, name),
        Err(SourceError::AuthRejected(_)) => ServiceStatus::auth_required(id, name),
        Err(e) => ServiceStatus::unreachable(id, name, e.to_string()),
    }
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
            tray::update_state(
                &handle,
                &snapshot,
                settings.show_tray_percentage,
                settings.notify_warn_pct,
                settings.notify_crit_pct,
            );
            notify_thresholds(&handle, &state, &snapshot, &settings).await;
            maybe_check_update(&handle, &state, &settings).await;
            tokio::time::sleep(Duration::from_secs(settings.poll_interval_secs)).await;
        }
    });
}

/// Check GitHub for a newer release, throttled to [`UPDATE_CHECK_INTERVAL_SECS`]
/// and gated on the `check_updates` setting. On a newer release: cache it, emit
/// `update-available`, and fire a desktop notification once per version. A failed
/// check is logged and retried next interval — it never interrupts polling.
async fn maybe_check_update(
    handle: &AppHandle,
    state: &Arc<AppState>,
    settings: &crate::settings::Settings,
) {
    if !settings.check_updates {
        return;
    }
    let now = chrono::Utc::now().timestamp();
    {
        let last = *state.last_update_check.read().await;
        if last != 0 && now - last < UPDATE_CHECK_INTERVAL_SECS {
            return;
        }
    }
    *state.last_update_check.write().await = now;

    match state
        .update_checker
        .check(crate::updates::CURRENT_VERSION)
        .await
    {
        Ok(Some(info)) => {
            *state.update.write().await = Some(info.clone());
            let _ = handle.emit("update-available", &info);
            // Notify once per version, then persist so we don't nag every check.
            if settings.notified_update_version != info.version {
                notify_update(handle, &info);
                let mut s = state.settings.write().await;
                s.notified_update_version = info.version.clone();
                if let Err(e) = s.save() {
                    warn!(?e, "failed to persist notified update version");
                }
            }
        }
        Ok(None) => {
            *state.update.write().await = None;
        }
        Err(e) => warn!(?e, "update check failed"),
    }
}

fn notify_update(app: &AppHandle, info: &crate::updates::UpdateInfo) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(e) = app
        .notification()
        .builder()
        .title("Headroom update available")
        .body(format!(
            "Version {} is available — open Headroom to download.",
            info.version
        ))
        .show()
    {
        warn!(?e, "failed to show update notification");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::Credentials;
    use crate::settings::Settings;
    use crate::sources::{QuotaSource, ServiceState};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    fn active(id: &str) -> ServiceStatus {
        ServiceStatus {
            id: id.to_string(),
            name: id.to_string(),
            plan: String::new(),
            state: ServiceState::Active,
            quotas: vec![],
            error_detail: None,
            copilot_usage: None,
        }
    }

    #[test]
    fn classify_maps_each_source_outcome_to_a_state() {
        assert!(matches!(
            classify("c", "C", Ok(active("c"))).state,
            ServiceState::Active
        ));
        assert!(matches!(
            classify("c", "C", Err(SourceError::MissingCredentials("c"))).state,
            ServiceState::NeedsSetup
        ));
        assert!(matches!(
            classify("c", "C", Err(SourceError::AuthRejected("bad token".into()))).state,
            ServiceState::AuthRequired
        ));
        assert!(matches!(
            classify("c", "C", Err(SourceError::CloudflareChallenge)).state,
            ServiceState::Unreachable
        ));
        assert!(matches!(
            classify(
                "c",
                "C",
                Err(SourceError::HttpStatus {
                    status: 500,
                    body: "boom".into()
                })
            )
            .state,
            ServiceState::Unreachable
        ));
        assert!(matches!(
            classify("c", "C", Err(SourceError::Parse("nope".into()))).state,
            ServiceState::Unreachable
        ));
    }

    /// A source whose `fetch` returns a preset outcome, for testing the loop
    /// without real network or credentials.
    struct MockSource {
        id: &'static str,
        outcome: fn() -> Result<ServiceStatus, SourceError>,
    }

    #[async_trait]
    impl QuotaSource for MockSource {
        fn id(&self) -> &'static str {
            self.id
        }
        fn name(&self) -> &'static str {
            "Mock"
        }
        async fn fetch(&self, _creds: &Credentials) -> Result<ServiceStatus, SourceError> {
            (self.outcome)()
        }
    }

    fn state_with(sources: Vec<Arc<dyn QuotaSource>>) -> Arc<AppState> {
        Arc::new(AppState {
            credentials: Credentials::new(),
            sources,
            last_snapshot: RwLock::new(None),
            settings: RwLock::new(Settings::default()),
            notified: RwLock::new(HashMap::new()),
            history: RwLock::new(crate::history::History::default()),
            update_checker: crate::updates::UpdateChecker::default(),
            update: RwLock::new(None),
            last_update_check: RwLock::new(0),
        })
    }

    #[tokio::test]
    async fn poll_once_aggregates_and_stores_each_source_state() {
        let state = state_with(vec![
            Arc::new(MockSource {
                id: "ok",
                outcome: || Ok(active("ok")),
            }),
            Arc::new(MockSource {
                id: "missing",
                outcome: || Err(SourceError::MissingCredentials("missing")),
            }),
            Arc::new(MockSource {
                id: "boom",
                outcome: || Err(SourceError::Parse("boom".into())),
            }),
        ]);

        let snapshot = poll_once(state.clone()).await;

        assert_eq!(snapshot.services.len(), 3);
        assert!(matches!(snapshot.services[0].state, ServiceState::Active));
        assert!(matches!(
            snapshot.services[1].state,
            ServiceState::NeedsSetup
        ));
        assert!(matches!(
            snapshot.services[2].state,
            ServiceState::Unreachable
        ));

        // The snapshot is cached for the IPC `refresh_all` / startup paths.
        assert!(state.last_snapshot.read().await.is_some());
    }
}
