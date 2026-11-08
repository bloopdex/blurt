//! In-game overlay: a top-most sound picker toggled by a global shortcut. It never takes focus, so the
//! game stays in front and fullscreen. It shows over borderless games and over most "fullscreen"
//! DirectX games (Windows' fullscreen optimisations compose top-most windows). Blurt never injects
//! into games as Discord/Steam do: anti-cheat such as Riot Vanguard bans that.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow, Window, WindowEvent};

pub const LABEL: &str = "overlay";

/// Games keep Esc for themselves, so the overlay closes after this long without use. Timed here, not
/// in the webview: WebView2 throttles timers in a window that never gets focus.
const IDLE_CLOSE: Duration = Duration::from_secs(4);
static ACTIVITY: AtomicU64 = AtomicU64::new(0);

pub fn init(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        round_corners(&w);
    }
}

/// Windows 11 rounds the window itself, matching the panel (no-op on Windows 10).
#[cfg(windows)]
fn round_corners(w: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute};
    let Ok(h) = w.hwnd() else { return };
    let pref = DWMWCP_ROUND;
    let _ = unsafe { DwmSetWindowAttribute(HWND(h.0), DWMWA_WINDOW_CORNER_PREFERENCE, (&raw const pref).cast(), size_of_val(&pref) as u32) };
}

#[cfg(not(windows))]
fn round_corners(_w: &WebviewWindow) {}

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).is_some_and(|w| w.is_visible().unwrap_or(false))
}

/// Restart the idle timer (a key, click or slider use in the overlay).
pub fn touch(app: &AppHandle) {
    let n = ACTIVITY.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(IDLE_CLOSE);
        if ACTIVITY.load(Ordering::SeqCst) == n && is_open(&app) {
            hide(&app);
        }
    });
}

pub fn toggle(app: &AppHandle) {
    let Some(w) = app.get_webview_window(LABEL) else { return };
    if is_open(app) {
        hide(app);
        return;
    }
    place_on_cursor_monitor(&w);
    // Taken while the overlay is still hidden, so it shows only what's behind it.
    let backdrop = w.outer_position().ok().zip(w.outer_size().ok()).and_then(|(p, s)| capture(p.x, p.y, s.width as i32, s.height as i32));
    // Never take focus: the game stays in front (no taskbar popping up), and keys arrive as global keys.
    let _ = w.set_focusable(false);
    show_without_focus(&w);
    super::shortcuts::set_overlay_keys(app, true);
    let _ = w.emit_to(LABEL, "overlay-shown", backdrop);
    touch(app);
}

/// A small, scaled-down copy of the screen behind the overlay, which the overlay blurs into frosted
/// glass. (Windows' own blur effects turn solid in a window that never activates.)
#[derive(Clone, Serialize)]
pub struct Backdrop {
    width: i32,
    height: i32,
    /// Top-down BGRA rows.
    bgra: Vec<u8>,
}

/// It's blurred to nothing anyway, so 1/8 scale keeps the event small (about 7k pixels).
const BACKDROP_SCALE: i32 = 8;

#[cfg(windows)]
fn capture(x: i32, y: i32, w: i32, h: i32) -> Option<Backdrop> {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC,
        GetDIBits, HALFTONE, ReleaseDC, SRCCOPY, SelectObject, SetStretchBltMode, StretchBlt,
    };
    let (sw, sh) = ((w / BACKDROP_SCALE).max(1), (h / BACKDROP_SCALE).max(1));
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, sw, sh);
        let old = SelectObject(mem, bmp.into());
        SetStretchBltMode(mem, HALFTONE);
        let copied = StretchBlt(mem, 0, 0, sw, sh, Some(screen), x, y, w, h, SRCCOPY).as_bool();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: sw,
                biHeight: -sh, // negative = top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (sw * sh * 4) as usize];
        let lines = GetDIBits(mem, bmp, 0, sh as u32, Some(bgra.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        (copied && lines == sh).then_some(Backdrop { width: sw, height: sh, bgra })
    }
}

#[cfg(not(windows))]
fn capture(_x: i32, _y: i32, _w: i32, _h: i32) -> Option<Backdrop> {
    None
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

/// Shown with a raw ShowWindow, so Tauri still thinks it's hidden and its hide() does nothing: hide it the same way.
#[cfg(windows)]
fn hide_window(w: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
    if let Ok(h) = w.hwnd() {
        let _ = unsafe { ShowWindow(HWND(h.0), SW_HIDE) };
    }
}

#[cfg(not(windows))]
fn hide_window(w: &WebviewWindow) {
    let _ = w.hide();
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
        hide_window(&w);
    }
    super::shortcuts::set_overlay_keys(app, false);
}

/// Closing (Alt+F4) only hides, so the next toggle is instant.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        hide(window.app_handle());
    }
}
