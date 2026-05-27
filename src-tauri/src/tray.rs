//! Tray icon installation and state machine.
//!
//! Four image variants live in `icons/`: `tray-ok.png`, `tray-warn.png`,
//! `tray-crit.png`, `tray-unreachable.png`. The worst quota across all
//! services drives which one is active.
//!
//! See SPEC.md § "Tray icon — visual states" and DESIGN_SYSTEM.md § "Tray icons".

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
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
    let show = MenuItem::with_id(app, "show", "Open Headroom", true, None::<&str>)?;
    let onboarding = MenuItem::with_id(app, "onboarding", "Set up accounts…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Headroom", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &onboarding, &settings, &sep, &quit])?;

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
            // Only a left-button *release* toggles the popover. Matching every
            // Click fires on both press and release (double-toggle → flash),
            // and a bare match also stole right-clicks from the context menu.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
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

pub fn update_state(app: &AppHandle, snapshot: &Snapshot, show_percentage: bool) {
    let state = compute_state(snapshot);
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    if let Ok(icon) = Image::from_bytes(state.icon_bytes()) {
        if let Err(e) = tray.set_icon(Some(icon)) {
            error!(?e, "failed to update tray icon");
        }
    }
    // Title shows next to the macOS menu-bar icon; Windows ignores it.
    match worst_percentage(snapshot).filter(|_| show_percentage) {
        Some(pct) => {
            let _ = tray.set_title(Some(&format!("{pct}%")));
        }
        None => {
            let _ = tray.set_title(None::<&str>);
        }
    }

    // Tooltip is where the percentage is actually visible on Windows (on hover).
    let tooltip = match worst_percentage(snapshot) {
        Some(pct) if show_percentage => format!("Headroom — {pct}% used"),
        _ => "Headroom".to_string(),
    };
    let _ = tray.set_tooltip(Some(&tooltip));
}

fn compute_state(snapshot: &Snapshot) -> TrayState {
    let mut worst = TrayState::Ok;
    let mut any_unreachable = false;

    for service in &snapshot.services {
        match service.state {
            ServiceState::Unreachable => any_unreachable = true,
            // Not connected yet is not an alert condition for the tray icon.
            ServiceState::NeedsSetup => {}
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
