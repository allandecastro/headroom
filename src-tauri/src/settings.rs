//! User preferences, persisted as JSON at `config_dir()/headroom/settings.json`.
//!
//! Writes are atomic (temp file + rename). A missing or unreadable file yields
//! [`Settings::default`]. The shape is mirrored by the `Settings` interface in
//! `src/lib/ipc.ts`; field names are snake_case on both sides.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Poll cadence. One of 15 | 30 | 60 | 300 seconds.
    pub poll_interval_secs: u64,
    /// "auto" | "light" | "dark".
    pub theme: String,
    pub show_tray_percentage: bool,
    /// Percentage at which the orange "heads up" alert fires (0 = off).
    pub notify_warn_pct: u8,
    /// Percentage at which the red "critical" alert fires (0 = off).
    pub notify_crit_pct: u8,
    /// Surface the optional "Claude Design" usage window in the popover.
    #[serde(default)]
    pub show_claude_design: bool,
    /// Periodically check GitHub for a newer release and notify once when found.
    #[serde(default = "default_true")]
    pub check_updates: bool,
    /// Query Codex's own `/codex/usage` endpoint for fresh rate limits, using the
    /// token already stored by the Codex CLI in `~/.codex`. When off, Headroom
    /// reads only Codex's local rollout logs (no network). Default on.
    #[serde(default = "default_true")]
    pub codex_live_query: bool,
    /// Latest version already notified about, so the toast fires once per new
    /// release rather than on every check. Empty = none yet. Carried through the
    /// renderer's settings round-trip but never shown in the UI.
    #[serde(default)]
    pub notified_update_version: String,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30,
            theme: "auto".to_string(),
            show_tray_percentage: true,
            notify_warn_pct: 80,
            notify_crit_pct: 95,
            show_claude_design: false,
            check_updates: true,
            codex_live_query: true,
            notified_update_version: String::new(),
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("headroom").join("settings.json"))
}

impl Settings {
    /// Read from disk, falling back to defaults on any error (missing file,
    /// malformed JSON, unknown config dir).
    pub fn load() -> Self {
        let Some(path) = settings_path() else {
            return Self::default();
        };
        match fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str::<Settings>(&raw)
                .map(Settings::sanitized)
                .unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist atomically: write a sibling `.tmp` then rename over the target.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = settings_path().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Clamp out-of-range values to safe defaults so a hand-edited file or a
    /// stale renderer can never poison the poll loop or theme handling.
    pub fn sanitized(mut self) -> Self {
        if !matches!(self.poll_interval_secs, 15 | 30 | 60 | 300) {
            self.poll_interval_secs = 30;
        }
        if !matches!(self.theme.as_str(), "auto" | "light" | "dark") {
            self.theme = "auto".to_string();
        }
        // Thresholds are percentages; 0 means the alert is off.
        if self.notify_warn_pct > 100 {
            self.notify_warn_pct = 80;
        }
        if self.notify_crit_pct > 100 {
            self.notify_crit_pct = 95;
        }
        // Critical must sit strictly above heads-up when both are enabled —
        // otherwise the red alert could fire at or below the orange one.
        if self.notify_warn_pct > 0
            && self.notify_crit_pct > 0
            && self.notify_crit_pct <= self.notify_warn_pct
        {
            self.notify_warn_pct = self.notify_crit_pct.saturating_sub(5);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let s = Settings::default();
        assert_eq!(s.poll_interval_secs, 30);
        assert_eq!(s.theme, "auto");
        assert!(s.show_tray_percentage);
    }

    #[test]
    fn sanitized_clamps_bad_interval_and_theme() {
        let s = Settings {
            poll_interval_secs: 7,
            theme: "neon".to_string(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.poll_interval_secs, 30);
        assert_eq!(s.theme, "auto");
    }

    #[test]
    fn sanitized_keeps_valid_values() {
        for secs in [15, 30, 60, 300] {
            let s = Settings {
                poll_interval_secs: secs,
                theme: "dark".to_string(),
                ..Settings::default()
            }
            .sanitized();
            assert_eq!(s.poll_interval_secs, secs);
            assert_eq!(s.theme, "dark");
        }
    }

    #[test]
    fn sanitized_keeps_critical_above_heads_up() {
        // crit <= warn (both enabled) → warn is pulled below crit.
        let s = Settings {
            notify_warn_pct: 90,
            notify_crit_pct: 85,
            ..Settings::default()
        }
        .sanitized();
        assert!(s.notify_crit_pct > s.notify_warn_pct, "{s:?}");

        // Equal values are also corrected.
        let s = Settings {
            notify_warn_pct: 80,
            notify_crit_pct: 80,
            ..Settings::default()
        }
        .sanitized();
        assert!(s.notify_crit_pct > s.notify_warn_pct, "{s:?}");

        // A disabled (0) threshold imposes no ordering constraint.
        let s = Settings {
            notify_warn_pct: 0,
            notify_crit_pct: 50,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.notify_warn_pct, 0);
        assert_eq!(s.notify_crit_pct, 50);
    }

    #[test]
    fn json_round_trips() {
        let s = Settings {
            poll_interval_secs: 60,
            theme: "light".to_string(),
            show_tray_percentage: false,
            notify_warn_pct: 75,
            notify_crit_pct: 90,
            show_claude_design: true,
            check_updates: false,
            codex_live_query: false,
            notified_update_version: "1.3.0".to_string(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), s);
        // snake_case field names are part of the IPC contract with the renderer.
        assert!(json.contains("poll_interval_secs"));
        assert!(json.contains("show_tray_percentage"));
    }
}
