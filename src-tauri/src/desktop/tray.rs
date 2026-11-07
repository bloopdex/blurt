//! System tray (Windows notification area / macOS menu bar).

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::audio::engine::EngineStatus;
use crate::state::AppState;

pub struct TrayState {
    status: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Blurt", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pause / resume playback", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "Stop all sounds", true, None::<&str>)?;
    let status = MenuItem::with_id(app, "status", "Virtual mic: starting…", false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Blurt", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &pause,
            &stop,
            &PredefinedMenuItem::separator(app)?,
            &status,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Blurt")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let state = app.state::<AppState>();
            match event.id().as_ref() {
                "open" => super::show_main(app),
                "pause" => state.engine.set_paused(None, None),
                "stop" => state.engine.stop_all(true),
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                super::show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    app.manage(TrayState { status });
    Ok(())
}

pub fn status_text(s: &EngineStatus) -> String {
    match (&s.virtual_mic.device, s.virtual_mic.open) {
        (None, _) => "Virtual mic: not set up".into(),
        (Some(_), true) => "Virtual mic: ready".into(),
        (Some(_), false) => "Virtual mic: unavailable".into(),
    }
}

pub fn update(app: &AppHandle, s: &EngineStatus) {
    if let Some(tray) = app.try_state::<TrayState>()
        && let Err(e) = tray.status.set_text(status_text(s))
    {
        log::warn!("tray update failed: {e}");
    }
}
