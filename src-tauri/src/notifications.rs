//! Threshold-crossing desktop notifications.

use std::collections::HashMap;
use std::sync::Arc;

use tauri::AppHandle;
use tracing::warn;

use crate::settings::Settings;
use crate::sources::ServiceState;
use crate::{AppState, Snapshot};

/// A single notification to display.
struct Alert {
    title: String,
    body: String,
}

/// The highest enabled threshold a percentage has crossed (returns that
/// threshold's value), or 0 for none. A threshold of 0 is "off"; critical takes
/// precedence over warning.
pub(crate) fn crossed_threshold(pct: f64, warn_pct: u8, crit_pct: u8) -> u8 {
    if crit_pct > 0 && pct >= crit_pct as f64 {
        crit_pct
    } else if warn_pct > 0 && pct >= warn_pct as f64 {
        warn_pct
    } else {
        0
    }
}

/// Decide which notifications to fire, mutating `notified` to record the highest
/// threshold alerted per `service:quota`. Pure (no I/O) so the once-per-crossing
/// and re-arm behaviour is unit-testable. A quota that drops back below every
/// enabled threshold re-arms, so a later re-crossing alerts again.
fn plan_alerts(
    snapshot: &Snapshot,
    settings: &Settings,
    notified: &mut HashMap<String, u8>,
) -> Vec<Alert> {
    let warn = settings.notify_warn_pct;
    let crit = settings.notify_crit_pct;
    if warn == 0 && crit == 0 {
        return vec![];
    }

    let mut alerts = vec![];
    for service in &snapshot.services {
        if !matches!(service.state, ServiceState::Active) {
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
                notified.insert(key, 0);
                continue;
            }
            if crossed > last {
                let is_crit = crit > 0 && crossed == crit;
                let (marker, level) = if is_crit {
                    ("🔴", "Critical")
                } else {
                    ("🟠", "Heads up")
                };
                // A per-account Copilot source is named just by its account
                // label, so prefix the service so a multi-account toast says
                // which account it's about (e.g. "GitHub Copilot · alice").
                let who = if service.id.starts_with("copilot:") {
                    format!("GitHub Copilot · {}", service.name)
                } else {
                    service.name.clone()
                };
                alerts.push(Alert {
                    title: "Headroom".to_string(),
                    body: format!("{marker} {level}: {who} · {} at {:.0}%", quota.label, pct),
                });
                notified.insert(key, crossed);
            }
        }
    }
    alerts
}

/// Fire any due threshold notifications for this snapshot.
pub(crate) async fn notify_thresholds(
    app: &AppHandle,
    state: &Arc<AppState>,
    snapshot: &Snapshot,
    settings: &Settings,
) {
    use tauri_plugin_notification::NotificationExt;

    // Compute under the lock, then release it before the (blocking) toast calls.
    let alerts = {
        let mut notified = state.notified.write().await;
        plan_alerts(snapshot, settings, &mut notified)
    };

    for alert in alerts {
        if let Err(e) = app
            .notification()
            .builder()
            .title(alert.title)
            .body(alert.body)
            .show()
        {
            warn!(?e, "failed to show notification");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::{Quota, QuotaUnit, QuotaWindow, ServiceStatus};

    fn quota(used: f64, total: f64) -> Quota {
        Quota::new(
            QuotaWindow::FiveHour,
            "Current session",
            used,
            total,
            QuotaUnit::Percent,
            chrono::Utc::now(),
        )
    }

    fn service(state: ServiceState, quotas: Vec<Quota>) -> ServiceStatus {
        ServiceStatus {
            id: "claude".to_string(),
            name: "Claude".to_string(),
            plan: String::new(),
            state,
            quotas,
            error_detail: None,
            copilot_usage: None,
            codex_meta: None,
        }
    }

    fn snapshot(services: Vec<ServiceStatus>) -> Snapshot {
        Snapshot {
            polled_at: 0,
            services,
        }
    }

    fn settings(warn: u8, crit: u8) -> Settings {
        Settings {
            notify_warn_pct: warn,
            notify_crit_pct: crit,
            ..Settings::default()
        }
    }

    #[test]
    fn crossed_threshold_respects_boundaries_and_precedence() {
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

    #[test]
    fn both_thresholds_off_never_alerts() {
        let snap = snapshot(vec![service(
            ServiceState::Active,
            vec![quota(99.0, 100.0)],
        )]);
        let mut notified = HashMap::new();
        assert!(plan_alerts(&snap, &settings(0, 0), &mut notified).is_empty());
    }

    #[test]
    fn fires_once_per_crossing_then_stays_quiet() {
        let snap = snapshot(vec![service(
            ServiceState::Active,
            vec![quota(85.0, 100.0)],
        )]);
        let cfg = settings(80, 95);
        let mut notified = HashMap::new();

        let first = plan_alerts(&snap, &cfg, &mut notified);
        assert_eq!(first.len(), 1);
        assert!(first[0].body.contains("Heads up"));

        // Same level on the next poll → no repeat.
        assert!(plan_alerts(&snap, &cfg, &mut notified).is_empty());
    }

    #[test]
    fn escalates_from_warning_to_critical() {
        let cfg = settings(80, 95);
        let mut notified = HashMap::new();

        let warn = plan_alerts(
            &snapshot(vec![service(
                ServiceState::Active,
                vec![quota(85.0, 100.0)],
            )]),
            &cfg,
            &mut notified,
        );
        assert!(warn[0].body.contains("Heads up"));

        let crit = plan_alerts(
            &snapshot(vec![service(
                ServiceState::Active,
                vec![quota(96.0, 100.0)],
            )]),
            &cfg,
            &mut notified,
        );
        assert_eq!(crit.len(), 1);
        assert!(crit[0].body.contains("Critical"));
    }

    #[test]
    fn re_arms_after_dropping_below_thresholds() {
        let cfg = settings(80, 95);
        let mut notified = HashMap::new();

        plan_alerts(
            &snapshot(vec![service(
                ServiceState::Active,
                vec![quota(85.0, 100.0)],
            )]),
            &cfg,
            &mut notified,
        );
        // Drops back below both — re-arms.
        assert!(plan_alerts(
            &snapshot(vec![service(
                ServiceState::Active,
                vec![quota(40.0, 100.0)]
            )]),
            &cfg,
            &mut notified,
        )
        .is_empty());
        // Crossing again alerts again.
        let again = plan_alerts(
            &snapshot(vec![service(
                ServiceState::Active,
                vec![quota(85.0, 100.0)],
            )]),
            &cfg,
            &mut notified,
        );
        assert_eq!(again.len(), 1);
    }

    #[test]
    fn copilot_account_alert_names_the_account() {
        let svc = ServiceStatus {
            id: "copilot:12345".to_string(),
            name: "alice".to_string(),
            plan: "Business".to_string(),
            state: ServiceState::Active,
            quotas: vec![quota(85.0, 100.0)],
            error_detail: None,
            copilot_usage: None,
            codex_meta: None,
        };
        let alerts = plan_alerts(&snapshot(vec![svc]), &settings(80, 95), &mut HashMap::new());
        assert_eq!(alerts.len(), 1);
        assert!(
            alerts[0].body.contains("GitHub Copilot · alice"),
            "toast should name the Copilot account: {}",
            alerts[0].body
        );
    }

    #[test]
    fn skips_inactive_services_and_empty_quotas() {
        let cfg = settings(80, 95);
        let mut notified = HashMap::new();

        // Non-active service is ignored even when "over" a threshold.
        assert!(plan_alerts(
            &snapshot(vec![service(
                ServiceState::Unreachable,
                vec![quota(99.0, 100.0)]
            )]),
            &cfg,
            &mut notified,
        )
        .is_empty());

        // A zero-total quota can't be a percentage — skip it (no divide-by-zero).
        assert!(plan_alerts(
            &snapshot(vec![service(ServiceState::Active, vec![quota(5.0, 0.0)])]),
            &cfg,
            &mut notified,
        )
        .is_empty());
    }
}
