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

/// History older than this is dropped (in memory and from the file on load).
const RETENTION_DAYS: i64 = 30;
/// Minimum spacing between recorded samples per series, so a 30s poll interval
/// doesn't write a point every tick.
const MIN_INTERVAL_SECS: i64 = 300; // 5 minutes
/// How many points a sparkline is downsampled to.
const SPARK_POINTS: usize = 24;

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
}
