//! Threshold-crossing desktop notifications.

use std::sync::Arc;

use tauri::AppHandle;
use tracing::warn;

use crate::settings::Settings;
use crate::sources::ServiceState;
use crate::{AppState, Snapshot};

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

/// Notify once per threshold crossing — orange "heads up" at the warning level,
/// red "critical" at the critical level. State re-arms when a quota drops back
/// below both thresholds, so a later re-crossing alerts again.
pub(crate) async fn notify_thresholds(
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
