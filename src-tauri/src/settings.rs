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
    pub notify_80: bool,
    pub notify_95: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30,
            theme: "auto".to_string(),
            show_tray_percentage: true,
            notify_80: true,
            notify_95: true,
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
    fn json_round_trips() {
        let s = Settings {
            poll_interval_secs: 60,
            theme: "light".to_string(),
            show_tray_percentage: false,
            notify_80: false,
            notify_95: true,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), s);
        // snake_case field names are part of the IPC contract with the renderer.
        assert!(json.contains("poll_interval_secs"));
        assert!(json.contains("show_tray_percentage"));
    }
}
