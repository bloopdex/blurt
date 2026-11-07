//! Native desktop behaviour: tray, global shortcuts, window lifecycle, login items, notifications.

pub mod overlay;
pub mod ptt;
pub mod remote;
pub mod shortcuts;
pub mod tray;

use tauri::{AppHandle, Manager, Window, WindowEvent};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;

use crate::settings::Settings;
use crate::state::AppState;

pub fn show_main(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn main_window_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

/// Closing the window keeps Blurt (and its audio engine) running in the tray when enabled.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() == overlay::LABEL {
        return overlay::on_window_event(window, event);
    }
    if let WindowEvent::CloseRequested { api, .. } = event
        && window.label() == "main"
    {
        let app = window.app_handle();
        let close_to_tray = app.try_state::<AppState>().is_some_and(|s| s.settings().close_to_tray);
        if close_to_tray {
            api.prevent_close();
            let _ = window.hide();
            #[cfg(target_os = "macos")]
            if !app.state::<AppState>().settings().show_dock_icon {
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
        } else {
            app.exit(0);
        }
    }
}

/// Save the instant clip into the library and tell every window (used by the hotkey, UI and remote control).
pub fn save_clip(app: &AppHandle) -> crate::error::AppResult<crate::db::sounds::Sound> {
    use tauri::Emitter;
    let state = app.state::<AppState>();
    let sound = crate::library::save_clip(&state.db, &state.paths, &state.clips, state.settings().default_sound_volume as f64)?;
    let _ = app.emit("sound-added", &sound);
    notify(app, "Clip saved", &format!("“{}” is in your library.", sound.name));
    Ok(sound)
}

/// OS notification, only when the window isn't visible (in-app toasts cover the visible case).
pub fn notify(app: &AppHandle, title: &str, body: &str) {
    let enabled = app.try_state::<AppState>().is_some_and(|s| s.settings().show_notifications);
    if !enabled || main_window_visible(app) {
        return;
    }
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("notification failed: {e}");
    }
}

/// Apply settings that map to OS state. Returns a user-facing error for the login item.
pub fn apply_os_settings(app: &AppHandle, prev: Option<&Settings>, next: &Settings) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if prev.is_none_or(|p| p.show_dock_icon != next.show_dock_icon) {
        let policy = if next.show_dock_icon || main_window_visible(app) {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        let _ = app.set_activation_policy(policy);
    }
    if prev.is_none_or(|p| p.start_at_login != next.start_at_login) {
        let launcher = app.autolaunch();
        let enabled = launcher.is_enabled().unwrap_or(false);
        let result = match (next.start_at_login, enabled) {
            (true, false) => launcher.enable(),
            (false, true) => launcher.disable(),
            _ => Ok(()),
        };
        result.map_err(|e| format!("Couldn't update the login item: {e}"))?;
    }
    Ok(())
}
