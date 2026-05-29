//! Linear burndown projection. Given how much of a quota window is used and
//! when it resets, extrapolate the average rate so far to estimate end-of-window
//! utilization — answering "am I on track to hit the cap before it resets?".

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

use crate::sources::QuotaWindow;

#[derive(Debug, Clone, Serialize)]
pub struct Projection {
    /// Extrapolated utilization (%) at the moment the window resets.
    pub projected_pct: f64,
    /// Whether usage is on track to reach 100% before the window resets.
    pub will_exceed: bool,
    /// When utilization is projected to hit 100%, if before reset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<DateTime<Utc>>,
}

/// Recent-burn-rate pace, computed from the last ~24h of history samples.
/// Complements `Projection` (which uses the window-average): `Pace` is more
/// reactive to today's behaviour, so a quiet morning won't hide a hot afternoon.
#[derive(Debug, Clone, Serialize)]
pub struct Pace {
    /// Percentage points consumed per day over the recent lookback window.
    pub daily_rate: f64,
    /// The most you could burn per day and still arrive at 100% exactly at reset.
    pub safe_pace: f64,
    /// `true` when `daily_rate > safe_pace` — you'll hit the cap early if this continues.
    pub over_pace: bool,
}

impl QuotaWindow {
    /// The length of the quota window, used to locate its start from `resets_at`.
    /// `Monthly` is approximated at 30 days.
    pub fn duration(self) -> Duration {
        match self {
            QuotaWindow::FiveHour => Duration::hours(5),
            QuotaWindow::WeeklyAll
            | QuotaWindow::WeeklySonnet
            | QuotaWindow::WeeklyOpus
            | QuotaWindow::ClaudeDesign => Duration::days(7),
            QuotaWindow::Monthly => Duration::days(30),
        }
    }
}

/// Don't project until the window is at least this far along — early on, a tiny
/// elapsed fraction makes the extrapolated rate wildly noisy.
const MIN_ELAPSED_FRACTION: f64 = 0.1;

/// Project end-of-window utilization assuming usage continues at the average
/// rate seen so far. Returns `None` when it's too early to be meaningful or the
/// inputs are degenerate (window not started, or already past reset).
pub fn project(
    used_pct: f64,
    window: QuotaWindow,
    resets_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<Projection> {
    let duration = window.duration();
    let start = resets_at - duration;
    let total_secs = duration.num_seconds() as f64;
    let elapsed_secs = (now - start).num_seconds() as f64;
    if total_secs <= 0.0 || elapsed_secs <= 0.0 {
        return None;
    }

    let elapsed_frac = elapsed_secs / total_secs;
    if !(MIN_ELAPSED_FRACTION..1.0).contains(&elapsed_frac) {
        return None;
    }
    if used_pct <= 0.0 {
        return Some(Projection {
            projected_pct: 0.0,
            will_exceed: false,
            eta: None,
        });
    }

    let projected_pct = used_pct / elapsed_frac;
    let will_exceed = projected_pct >= 100.0;

    // At the average rate, time from window start to 100% is (100 / rate); when
    // will_exceed this lands at or before reset.
    let eta = will_exceed.then(|| {
        let secs_to_full = (100.0 / projected_pct) * total_secs;
        start + Duration::seconds(secs_to_full as i64)
    });

    Some(Projection {
        projected_pct,
        will_exceed,
        eta,
    })
}

/// Pace from a recent-history delta: how much you're burning per day vs the
/// max %/day that lands at exactly 100% at reset. Meaningless once you've hit
/// the cap, or past reset, or when the delta is degenerate.
pub fn pace(
    used_pct: f64,
    resets_at: DateTime<Utc>,
    now: DateTime<Utc>,
    recent_delta_pct: f64,
    recent_delta_secs: i64,
) -> Option<Pace> {
    if used_pct >= 100.0 || recent_delta_secs <= 0 || recent_delta_pct < 0.0 {
        return None;
    }
    let secs_to_reset = (resets_at - now).num_seconds();
    if secs_to_reset <= 0 {
        return None;
    }
    let daily_rate = (recent_delta_pct / recent_delta_secs as f64) * 86_400.0;
    let days_to_reset = secs_to_reset as f64 / 86_400.0;
    let safe_pace = (100.0 - used_pct) / days_to_reset;
    Some(Pace {
        daily_rate,
        safe_pace,
        over_pace: daily_rate > safe_pace,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_track_when_pace_stays_under_cap() {
        // 7d window, 2 days elapsed (≈29%), 10% used → ~35% projected.
        let now = Utc::now();
        let resets_at = now + Duration::days(5); // 2 days into a 7-day window
        let p = project(10.0, QuotaWindow::WeeklyAll, resets_at, now).unwrap();
        assert!(!p.will_exceed);
        assert!(p.projected_pct > 30.0 && p.projected_pct < 40.0);
        assert!(p.eta.is_none());
    }

    #[test]
    fn flags_overage_and_gives_eta_before_reset() {
        // 2 days into a 7-day window, already 50% used → ~175% projected.
        let now = Utc::now();
        let resets_at = now + Duration::days(5);
        let p = project(50.0, QuotaWindow::WeeklyAll, resets_at, now).unwrap();
        assert!(p.will_exceed);
        assert!(p.projected_pct > 100.0);
        let eta = p.eta.expect("eta when exceeding");
        assert!(eta > now && eta < resets_at);
    }

    #[test]
    fn too_early_in_window_yields_none() {
        // Only a few minutes into a 7-day window — below MIN_ELAPSED_FRACTION.
        let now = Utc::now();
        let resets_at = now + Duration::days(7) - Duration::minutes(5);
        assert!(project(2.0, QuotaWindow::WeeklyAll, resets_at, now).is_none());
    }

    #[test]
    fn nothing_used_is_on_track() {
        let now = Utc::now();
        let resets_at = now + Duration::days(3);
        let p = project(0.0, QuotaWindow::WeeklyAll, resets_at, now).unwrap();
        assert!(!p.will_exceed);
        assert_eq!(p.projected_pct, 0.0);
    }

    #[test]
    fn past_reset_yields_none() {
        let now = Utc::now();
        let resets_at = now - Duration::hours(1); // window already over
        assert!(project(50.0, QuotaWindow::WeeklyAll, resets_at, now).is_none());
    }

    #[test]
    fn pace_flags_over_when_daily_rate_exceeds_safe_pace() {
        // 50% used, 5 days to reset → safe_pace = 10%/day.
        // Δ of 6%/24h ≈ 6%/day → over the 10%/day safe pace? No, 6 < 10 — within.
        let now = Utc::now();
        let resets_at = now + Duration::days(5);
        let within = pace(50.0, resets_at, now, 6.0, 86_400).unwrap();
        assert!((within.safe_pace - 10.0).abs() < 0.01);
        assert!((within.daily_rate - 6.0).abs() < 0.01);
        assert!(!within.over_pace);

        // Δ of 15%/24h → 15%/day → over the 10%/day safe pace.
        let over = pace(50.0, resets_at, now, 15.0, 86_400).unwrap();
        assert!(over.over_pace);
    }

    #[test]
    fn pace_returns_none_when_already_exhausted_or_past_reset() {
        let now = Utc::now();
        // Already at 100%.
        assert!(pace(100.0, now + Duration::days(3), now, 5.0, 3600).is_none());
        // Past reset.
        assert!(pace(50.0, now - Duration::hours(1), now, 5.0, 3600).is_none());
        // Degenerate delta.
        assert!(pace(50.0, now + Duration::days(3), now, -1.0, 3600).is_none());
        assert!(pace(50.0, now + Duration::days(3), now, 5.0, 0).is_none());
    }
}
