//! In-game overlay: a top-most sound picker toggled by a global shortcut. It never takes focus, so the
//! game stays in front and fullscreen. It shows over borderless games and over most "fullscreen"
//! DirectX games (Windows' fullscreen optimisations compose top-most windows). Blurt never injects
//! into games as Discord/Steam do: anti-cheat such as Riot Vanguard bans that.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow, Window, WindowEvent};

pub const LABEL: &str = "overlay";

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).is_some_and(|w| w.is_visible().unwrap_or(false))
}

pub fn toggle(app: &AppHandle) {
    let Some(w) = app.get_webview_window(LABEL) else { return };
    if is_open(app) {
        hide(app);
        return;
    }
    place_on_cursor_monitor(&w);
    // Never take focus: the game stays in front (no taskbar popping up), and 1–9 arrive as global keys.
    let _ = w.set_focusable(false);
    show_without_focus(&w);
    super::shortcuts::set_overlay_keys(app, true);
    let _ = w.emit_to(LABEL, "overlay-shown", ());
}

#[cfg(windows)]
fn show_without_focus(w: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, ShowWindow,
    };
    let Ok(h) = w.hwnd() else { return };
    let h = HWND(h.0);
    unsafe {
        let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

#[cfg(not(windows))]
fn show_without_focus(w: &WebviewWindow) {
    let _ = w.show();
}

/// Games keep the cursor on their own screen, so centre there rather than where the overlay last was.
fn place_on_cursor_monitor(w: &WebviewWindow) {
    let monitor = w.cursor_position().ok().and_then(|p| w.monitor_from_point(p.x, p.y).ok().flatten());
    let (Some(m), Ok(size)) = (monitor, w.outer_size()) else {
        let _ = w.center();
        return;
    };
    let (pos, area) = (m.position(), m.size());
    let x = pos.x + (area.width as i32 - size.width as i32) / 2;
    let y = pos.y + (area.height as i32 - size.height as i32) / 2;
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
    super::shortcuts::set_overlay_keys(app, false);
}

/// Closing (Alt+F4) only hides, so the next toggle is instant.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide(window.app_handle());
        }
        _ => {}
    }
}
