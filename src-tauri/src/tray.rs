//! Tray icon installation and state machine.
//!
//! Four image variants live in `icons/`: `tray-ok.png`, `tray-warn.png`,
//! `tray-crit.png`, `tray-unreachable.png`. The worst quota across all
//! services drives which one is active.
//!
//! See SPEC.md § "Tray icon — visual states" and DESIGN_SYSTEM.md § "Tray icons".

use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tracing::error;

use crate::sources::{ServiceState, ServiceStatus};
use crate::Snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayState {
    Ok,
    Warn,
    Crit,
    Unreachable,
}

impl TrayState {
    fn icon_bytes(self) -> &'static [u8] {
        match self {
            TrayState::Ok => include_bytes!("../icons/tray-ok.png"),
            TrayState::Warn => include_bytes!("../icons/tray-warn.png"),
            TrayState::Crit => include_bytes!("../icons/tray-crit.png"),
            TrayState::Unreachable => include_bytes!("../icons/tray-unreachable.png"),
        }
    }
}

pub fn install(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Headroom", true, None::<&str>)?;
    let onboarding = MenuItem::with_id(app, "onboarding", "Set up accounts…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &onboarding, &settings, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(TrayState::Ok.icon_bytes())?)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                toggle_popover(app);
            }
            "onboarding" => {
                show_window(app, "onboarding");
            }
            "settings" => {
                show_window(app, "settings");
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                toggle_popover(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn toggle_popover(app: &AppHandle) {
    let Some(window) = app.get_webview_window("popover") else {
        return;
    };
    let visible = window.is_visible().unwrap_or(false);
    let action = if visible {
        window.hide()
    } else {
        let _ = window.show();
        window.set_focus()
    };
    if let Err(e) = action {
        error!(?e, "failed to toggle popover");
    }
}

fn show_window(app: &AppHandle, label: &str) {
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    let _ = window.show();
    if let Err(e) = window.set_focus() {
        error!(?e, label, "failed to focus window");
    }
}

pub fn update_state(app: &AppHandle, snapshot: &Snapshot) {
    let state = compute_state(snapshot);
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    if let Ok(icon) = Image::from_bytes(state.icon_bytes()) {
        if let Err(e) = tray.set_icon(Some(icon)) {
            error!(?e, "failed to update tray icon");
        }
    }
    if let Some(pct) = worst_percentage(snapshot) {
        let _ = tray.set_title(Some(&format!("{pct}%")));
    } else {
        let _ = tray.set_title(None::<&str>);
    }
}

fn compute_state(snapshot: &Snapshot) -> TrayState {
    let mut worst = TrayState::Ok;
    let mut any_unreachable = false;

    for service in &snapshot.services {
        match service.state {
            ServiceState::Unreachable => any_unreachable = true,
            ServiceState::Active => {
                for q in &service.quotas {
                    let pct = (q.used / q.total) * 100.0;
                    let local = if pct >= 95.0 {
                        TrayState::Crit
                    } else if pct >= 80.0 {
                        TrayState::Warn
                    } else {
                        TrayState::Ok
                    };
                    worst = worst.max(local);
                }
            }
            ServiceState::AuthRequired => any_unreachable = true,
        }
    }

    // If everything is unreachable AND nothing is critical, surface unreachable.
    // If anything was crit/warn, keep that — better to show a real state than mask it.
    if matches!(worst, TrayState::Ok) && any_unreachable {
        TrayState::Unreachable
    } else {
        worst
    }
}

fn worst_percentage(snapshot: &Snapshot) -> Option<u32> {
    snapshot
        .services
        .iter()
        .filter(|s| matches!(s.state, ServiceState::Active))
        .flat_map(|s: &ServiceStatus| s.quotas.iter())
        .map(|q| ((q.used / q.total) * 100.0) as u32)
        .max()
}

impl TrayState {
    fn max(self, other: TrayState) -> TrayState {
        use TrayState::*;
        let rank = |s| match s {
            Ok => 0,
            Unreachable => 1,
            Warn => 2,
            Crit => 3,
        };
        if rank(self) >= rank(other) {
            self
        } else {
            other
        }
    }
}
