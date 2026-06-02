//! Usage history: a bounded, persisted time series per quota window. Drives the
//! popover sparkline. Stored as append-only JSONL in the data dir and capped at
//! [`RETENTION_DAYS`]; sampling is throttled so a fast poll interval can't flood
//! it.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::sources::QuotaWindow;

/// One recorded usage point (utilization percentage at a moment in time).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub ts: i64, // unix seconds
    pub service: String,
    pub window: QuotaWindow,
    pub used_pct: f64,
}

/// A recent-burn-rate measurement from [`History::recent_delta`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecentDelta {
    /// Utilization consumed over the span (percentage points).
    pub delta_pct: f64,
    /// Wall-clock span the delta covers, in seconds.
    pub span_secs: i64,
    /// The lookback straddled an app-closed gap, so the rate is averaged across
    /// unobserved time — surface it as tentative.
    pub low_confidence: bool,
}

/// History older than this is dropped (in memory and from the file on load).
const RETENTION_DAYS: i64 = 30;
/// Minimum spacing between recorded samples per series, so a 30s poll interval
/// doesn't write a point every tick.
const MIN_INTERVAL_SECS: i64 = 300; // 5 minutes
/// How many points a sparkline is downsampled to.
const SPARK_POINTS: usize = 24;
/// Inter-sample spacing above which the series is treated as having a gap — the
/// app was closed. Far above the 5-minute sampling cadence, so a normal run
/// never trips it. A delta whose lookback straddles such a gap is averaged
/// across time we didn't observe, so it's reported but flagged low-confidence.
const GAP_THRESHOLD_SECS: i64 = 3600; // 1 hour

fn series_key(service: &str, window: QuotaWindow) -> String {
    format!("{service}:{window:?}")
}

fn history_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("headroom").join("history.jsonl"))
}

/// In-memory series store, keyed by `service:window`.
#[derive(Default)]
pub struct History {
    series: HashMap<String, Vec<Sample>>,
}

impl History {
    /// Load persisted history, dropping anything past the retention horizon.
    pub fn load() -> Self {
        let mut history = History::default();
        let Some(path) = history_path() else {
            return history;
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return history; // missing file on first run
        };
        let cutoff = now_secs() - RETENTION_DAYS * 86_400;
        for line in raw.lines() {
            if let Ok(sample) = serde_json::from_str::<Sample>(line) {
                if sample.ts >= cutoff {
                    history
                        .series
                        .entry(series_key(&sample.service, sample.window))
                        .or_default()
                        .push(sample);
                }
            }
        }
        history
    }

    /// Record a sample unless the previous one for this series is too recent.
    /// Returns the stored sample (to persist) or `None` when throttled.
    pub fn record(
        &mut self,
        service: &str,
        window: QuotaWindow,
        used_pct: f64,
        now: i64,
    ) -> Option<Sample> {
        let series = self.series.entry(series_key(service, window)).or_default();
        if let Some(last) = series.last() {
            if now - last.ts < MIN_INTERVAL_SECS {
                return None;
            }
        }
        let sample = Sample {
            ts: now,
            service: service.to_string(),
            window,
            used_pct,
        };
        series.push(sample.clone());
        let cutoff = now - RETENTION_DAYS * 86_400;
        series.retain(|s| s.ts >= cutoff);
        Some(sample)
    }

    /// Δutilization over the last `lookback_secs` for a series. Returns `None`
    /// when there aren't enough samples in the window or the window straddled a
    /// reset (delta would go negative).
    ///
    /// The delta is measured over wall-clock — `Δusage / elapsed`, idle time
    /// included — which is the honest pace for a calendar-reset quota (the reset
    /// fires regardless of activity). If the in-window samples contain a stretch
    /// longer than [`GAP_THRESHOLD_SECS`] (the app was closed), the rate is being
    /// averaged across time we didn't observe, so [`RecentDelta::low_confidence`]
    /// is set and the value is surfaced as tentative rather than re-anchored —
    /// re-anchoring to the post-reopen burst would over-state the daily rate
    /// whenever the gap was just idle sleep.
    pub fn recent_delta(
        &self,
        service: &str,
        window: QuotaWindow,
        now: i64,
        lookback_secs: i64,
    ) -> Option<RecentDelta> {
        let series = self.series.get(&series_key(service, window))?;
        let since = now - lookback_secs;
        let in_window: Vec<&Sample> = series.iter().filter(|s| s.ts >= since).collect();
        let first = *in_window.first()?;
        let last = *in_window.last()?;
        let dt = last.ts - first.ts;
        if dt <= 0 {
            return None;
        }
        let delta = last.used_pct - first.used_pct;
        if delta < 0.0 {
            return None; // window reset mid-lookback — delta is meaningless
        }
        let low_confidence = in_window
            .windows(2)
            .any(|pair| pair[1].ts - pair[0].ts > GAP_THRESHOLD_SECS);
        Some(RecentDelta {
            delta_pct: delta,
            span_secs: dt,
            low_confidence,
        })
    }

    /// Downsampled utilization series within the current window, for a sparkline.
    pub fn sparkline(&self, service: &str, window: QuotaWindow, now: i64) -> Vec<f64> {
        let Some(series) = self.series.get(&series_key(service, window)) else {
            return vec![];
        };
        let window_start = now - window.duration().num_seconds();
        let points: Vec<f64> = series
            .iter()
            .filter(|s| s.ts >= window_start)
            .map(|s| s.used_pct)
            .collect();
        downsample(&points, SPARK_POINTS)
    }
}

/// Append a sample to the on-disk history (best effort).
pub fn append_to_file(sample: &Sample) {
    let Some(path) = history_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(mut file) => {
            if let Ok(line) = serde_json::to_string(sample) {
                let _ = writeln!(file, "{line}");
            }
        }
        Err(e) => warn!(?e, "failed to append usage history"),
    }
}

/// Pick `target` evenly spaced points from `points`, preserving the first and
/// last. Returns the input unchanged when it already fits.
fn downsample(points: &[f64], target: usize) -> Vec<f64> {
    if target == 0 {
        return vec![];
    }
    if points.len() <= target {
        return points.to_vec();
    }
    (0..target)
        .map(|i| points[i * (points.len() - 1) / (target - 1)])
        .collect()
}

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_throttles_within_min_interval() {
        let mut h = History::default();
        let t0 = 1_000_000;
        assert!(h
            .record("claude", QuotaWindow::WeeklyAll, 10.0, t0)
            .is_some());
        // Too soon — dropped.
        assert!(h
            .record("claude", QuotaWindow::WeeklyAll, 11.0, t0 + 60)
            .is_none());
        // After the interval — recorded.
        assert!(h
            .record(
                "claude",
                QuotaWindow::WeeklyAll,
                12.0,
                t0 + MIN_INTERVAL_SECS
            )
            .is_some());
    }

    #[test]
    fn record_prunes_beyond_retention() {
        let mut h = History::default();
        let old = 1_000_000;
        h.record("claude", QuotaWindow::WeeklyAll, 5.0, old);
        // A sample far in the future drops the stale one.
        let now = old + (RETENTION_DAYS + 1) * 86_400;
        h.record("claude", QuotaWindow::WeeklyAll, 50.0, now);
        let spark = h.sparkline("claude", QuotaWindow::WeeklyAll, now);
        assert_eq!(spark, vec![50.0], "stale sample should be pruned");
    }

    #[test]
    fn sparkline_filters_to_current_window() {
        let mut h = History::default();
        let now = 10_000_000;
        // One sample inside the 7-day window, one well before it.
        h.record("claude", QuotaWindow::WeeklyAll, 20.0, now - 2 * 86_400);
        // Manually push an older point past throttling to land outside window.
        h.series
            .get_mut(&series_key("claude", QuotaWindow::WeeklyAll))
            .unwrap()
            .insert(
                0,
                Sample {
                    ts: now - 30 * 86_400,
                    service: "claude".into(),
                    window: QuotaWindow::WeeklyAll,
                    used_pct: 99.0,
                },
            );
        let spark = h.sparkline("claude", QuotaWindow::WeeklyAll, now);
        assert_eq!(spark, vec![20.0], "only in-window points are returned");
    }

    #[test]
    fn downsample_keeps_endpoints_and_count() {
        let pts: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let out = downsample(&pts, 24);
        assert_eq!(out.len(), 24);
        assert_eq!(out[0], 0.0);
        assert_eq!(*out.last().unwrap(), 99.0);
    }

    #[test]
    fn downsample_passes_through_when_small() {
        let pts = vec![1.0, 2.0, 3.0];
        assert_eq!(downsample(&pts, 24), pts);
    }

    #[test]
    fn recent_delta_returns_delta_and_span_within_lookback() {
        let mut h = History::default();
        let t0 = 1_000_000;
        // A continuous run (samples ≤ the gap threshold apart): 10% → 16% → 22%
        // over 1h. Lookback 24h covers all three; delta is 12% across the span.
        h.record("c", QuotaWindow::WeeklyAll, 10.0, t0);
        h.record("c", QuotaWindow::WeeklyAll, 16.0, t0 + 30 * 60);
        h.record("c", QuotaWindow::WeeklyAll, 22.0, t0 + 60 * 60);
        let rd = h
            .recent_delta("c", QuotaWindow::WeeklyAll, t0 + 60 * 60, 24 * 3600)
            .unwrap();
        assert!((rd.delta_pct - 12.0).abs() < 0.01);
        assert_eq!(rd.span_secs, 60 * 60);
    }

    #[test]
    fn recent_delta_returns_none_after_window_reset() {
        let mut h = History::default();
        let t0 = 1_000_000;
        h.record("c", QuotaWindow::WeeklyAll, 80.0, t0);
        // Big drop = window reset; delta would be negative.
        h.record("c", QuotaWindow::WeeklyAll, 5.0, t0 + 12 * 3600);
        assert!(h
            .recent_delta("c", QuotaWindow::WeeklyAll, t0 + 12 * 3600, 24 * 3600)
            .is_none());
    }

    #[test]
    fn recent_delta_needs_at_least_two_in_window() {
        let mut h = History::default();
        let t0 = 1_000_000;
        h.record("c", QuotaWindow::WeeklyAll, 10.0, t0);
        // Only one sample in the 1h lookback.
        assert!(h
            .recent_delta("c", QuotaWindow::WeeklyAll, t0 + 30 * 60, 3600)
            .is_none());
    }

    #[test]
    fn recent_delta_keeps_wall_clock_slope_but_flags_a_gap() {
        let mut h = History::default();
        let t0 = 1_000_000;
        // A little burn, then the app is closed 8h (idle sleep), then it reopens.
        // The honest slope is the wall-clock delta across the whole span; we do
        // NOT re-anchor to the post-reopen burst (that would over-state the daily
        // rate for an idle gap). The gap just flags the result low-confidence.
        h.record("c", QuotaWindow::WeeklyAll, 10.0, t0);
        h.record("c", QuotaWindow::WeeklyAll, 12.0, t0 + 5 * 60);
        h.record("c", QuotaWindow::WeeklyAll, 12.0, t0 + 8 * 3600);
        h.record("c", QuotaWindow::WeeklyAll, 14.0, t0 + 8 * 3600 + 30 * 60);
        let now = t0 + 8 * 3600 + 30 * 60;
        let rd = h
            .recent_delta("c", QuotaWindow::WeeklyAll, now, 24 * 3600)
            .unwrap();
        assert!(
            (rd.delta_pct - 4.0).abs() < 0.01,
            "delta was {}",
            rd.delta_pct
        );
        assert_eq!(rd.span_secs, now - t0, "span is the full wall-clock window");
        assert!(
            rd.low_confidence,
            "an in-window gap should flag low confidence"
        );
    }

    #[test]
    fn recent_delta_continuous_run_is_high_confidence() {
        let mut h = History::default();
        let t0 = 1_000_000;
        // Samples ≤ the gap threshold apart — no gap, so high confidence.
        h.record("c", QuotaWindow::WeeklyAll, 10.0, t0);
        h.record("c", QuotaWindow::WeeklyAll, 16.0, t0 + 30 * 60);
        h.record("c", QuotaWindow::WeeklyAll, 22.0, t0 + 60 * 60);
        let rd = h
            .recent_delta("c", QuotaWindow::WeeklyAll, t0 + 60 * 60, 24 * 3600)
            .unwrap();
        assert!(!rd.low_confidence);
    }
}
