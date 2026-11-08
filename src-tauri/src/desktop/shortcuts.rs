//! Global hotkeys. Registered OS-wide so they work while Blurt is minimised or in the tray.
//! Stored in canonical form (`control+alt+Digit1`) so equivalent spellings can't collide.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use crate::audio::engine::PlayMode;
use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use crate::state::AppState;

#[derive(Clone)]
enum Action {
    Play { sound_id: String, hold: bool },
    StopAll,
    ReplayLast,
    PlayRandom,
    SaveClip,
    ToggleOverlay,
    /// Number keys 1–9 and 0 while the overlay is open; the overlay maps them to its tiles.
    OverlayKey(u8),
    OverlayClose,
}

/// Bare keys the overlay listens to. Registered only while it's open, since they'd block typing.
const OVERLAY_KEYS: [(&str, Action); 26] = [
    ("Digit1", Action::OverlayKey(1)), ("Digit2", Action::OverlayKey(2)), ("Digit3", Action::OverlayKey(3)),
    ("Digit4", Action::OverlayKey(4)), ("Digit5", Action::OverlayKey(5)), ("Digit6", Action::OverlayKey(6)),
    ("Digit7", Action::OverlayKey(7)), ("Digit8", Action::OverlayKey(8)), ("Digit9", Action::OverlayKey(9)),
    ("Digit0", Action::OverlayKey(10)),
    ("Numpad1", Action::OverlayKey(1)), ("Numpad2", Action::OverlayKey(2)), ("Numpad3", Action::OverlayKey(3)),
    ("Numpad4", Action::OverlayKey(4)), ("Numpad5", Action::OverlayKey(5)), ("Numpad6", Action::OverlayKey(6)),
    ("Numpad7", Action::OverlayKey(7)), ("Numpad8", Action::OverlayKey(8)), ("Numpad9", Action::OverlayKey(9)),
    ("Numpad0", Action::OverlayKey(10)),
    // 11/12 = sound volume down/up, 13 = mute or unmute the mic.
    ("Minus", Action::OverlayKey(11)), ("Equal", Action::OverlayKey(12)),
    ("NumpadSubtract", Action::OverlayKey(11)), ("NumpadAdd", Action::OverlayKey(12)),
    ("KeyM", Action::OverlayKey(13)),
    ("Escape", Action::OverlayClose),
];

/// Turn the overlay's keys on or off. A key another app already owns is skipped.
pub fn set_overlay_keys(app: &AppHandle, on: bool) {
    let state = app.state::<AppState>();
    let gs = app.global_shortcut();
    let mut actions = state.shortcuts.actions.lock().unwrap_or_else(|e| e.into_inner());
    for (key, action) in OVERLAY_KEYS {
        let Ok(sc) = Shortcut::from_str(key) else { continue };
        if on && !actions.contains_key(&sc.id()) && gs.register(sc).is_ok() {
            actions.insert(sc.id(), action);
        } else if !on && actions.get(&sc.id()).is_some_and(|a| matches!(a, Action::OverlayKey(_) | Action::OverlayClose)) {
            let _ = gs.unregister(sc);
            actions.remove(&sc.id());
        }
    }
}

/// App-wide shortcuts kept in settings, with the name used in conflict messages.
fn globals(s: &Settings) -> [(Option<&String>, &'static str, Action); 5] {
    [
        (s.stop_all_shortcut.as_ref(), "Stop all sounds", Action::StopAll),
        (s.replay_last_shortcut.as_ref(), "Replay last sound", Action::ReplayLast),
        (s.overlay_shortcut.as_ref(), "Show overlay", Action::ToggleOverlay),
        (s.random_sound_shortcut.as_ref(), "Play a random sound", Action::PlayRandom),
        (s.clip_shortcut.as_ref(), "Save a clip", Action::SaveClip),
    ]
}

/// Canonicalise the app-wide shortcuts that changed and refuse clashes with each other or with a sound.
pub fn check_globals(state: &AppState, prev: &Settings, next: &mut Settings) -> AppResult<()> {
    let before = globals(prev);
    let mut seen: Vec<(&str, u32)> = Vec::new();
    for (i, (sc, label, _)) in globals(next).into_iter().enumerate() {
        let Some(sc) = sc else { continue };
        let (_, id) = canonical(sc)?;
        if let Some((first, _)) = seen.iter().find(|(_, o)| *o == id) {
            return Err(AppError::Shortcut(format!("“{label}” and “{first}” can't share a shortcut.")));
        }
        seen.push((label, id));
        if before[i].0 != Some(sc) {
            for (sid, other) in state.db.shortcuts()? {
                if canonical(&other).is_ok_and(|(_, oid)| oid == id) {
                    let name = state.db.get_sound(&sid).map(|s| s.name).unwrap_or_default();
                    return Err(AppError::Shortcut(format!("That shortcut is already assigned to “{name}”.")));
                }
            }
        }
    }
    for s in [&mut next.stop_all_shortcut, &mut next.replay_last_shortcut, &mut next.overlay_shortcut, &mut next.random_sound_shortcut, &mut next.clip_shortcut] {
        if let Some(sc) = s.as_mut() {
            *sc = canonical(sc)?.0;
        }
    }
    Ok(())
}

#[derive(Default)]
pub struct Registry {
    actions: Mutex<HashMap<u32, Action>>,
    held: Mutex<HashMap<u32, u64>>,
    problems: Mutex<Vec<ShortcutProblem>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutProblem {
    pub sound_id: Option<String>,
    pub shortcut: String,
    pub message: String,
}

/// Parse and canonicalise. Bare keys without a modifier are refused: as global hotkeys they'd
/// swallow normal typing in every other app.
pub fn canonical(accelerator: &str) -> AppResult<(String, u32)> {
    let hk = Shortcut::from_str(accelerator.trim())
        .map_err(|e| AppError::Shortcut(format!("\"{accelerator}\" isn't a valid shortcut: {e}")))?;
    let id = hk.id();
    let s = hk.into_string();
    let is_function_key = s.rsplit('+').next().is_some_and(|k| k.len() <= 3 && k.starts_with('F') && k[1..].chars().all(|c| c.is_ascii_digit()));
    if !s.contains('+') && !is_function_key {
        return Err(AppError::Shortcut("Global shortcuts need a modifier such as Ctrl or Alt.".into()));
    }
    Ok((s, id))
}

/// Check a proposed sound shortcut against everything else that's bound.
pub fn check_conflict(state: &AppState, sound_id: &str, accelerator: &str) -> AppResult<String> {
    let (canon, id) = canonical(accelerator)?;
    for (sc, label, _) in globals(&state.settings()) {
        if sc.is_some_and(|sc| canonical(sc).is_ok_and(|(_, gid)| gid == id)) {
            return Err(AppError::Shortcut(format!("That shortcut is already used for “{label}”.")));
        }
    }
    for (other, sc) in state.db.shortcuts()? {
        if other != sound_id && canonical(&sc).is_ok_and(|(_, oid)| oid == id) {
            let name = state.db.get_sound(&other).map(|s| s.name).unwrap_or_default();
            return Err(AppError::Shortcut(format!("That shortcut is already assigned to “{name}”.")));
        }
    }
    Ok(canon)
}

/// Re-register everything from the database and settings. Called at startup and after any change.
pub fn sync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let gs = app.global_shortcut();
    if let Err(e) = gs.unregister_all() {
        log::warn!("unregister_all failed: {e}");
    }
    let mut actions = HashMap::new();
    let mut problems = Vec::new();
    let mut register = |accel: &str, action: Action, sound_id: Option<String>| match Shortcut::from_str(accel) {
        Ok(sc) if actions.contains_key(&sc.id()) => problems.push(ShortcutProblem {
            sound_id,
            shortcut: accel.into(),
            message: "Used twice — only the first binding is active.".into(),
        }),
        Ok(sc) => match gs.register(sc) {
            Ok(()) => {
                actions.insert(sc.id(), action);
            }
            Err(e) => problems.push(ShortcutProblem {
                sound_id,
                shortcut: accel.into(),
                message: format!("Another app has already claimed this shortcut ({e})."),
            }),
        },
        Err(e) => problems.push(ShortcutProblem { sound_id, shortcut: accel.into(), message: e.to_string() }),
    };

    for (sc, _, action) in globals(&state.settings()) {
        if let Some(sc) = sc {
            register(sc, action, None);
        }
    }
    match state.db.list_sounds() {
        Ok(sounds) => {
            for s in sounds {
                if let Some(sc) = &s.shortcut {
                    register(sc, Action::Play { sound_id: s.id.clone(), hold: s.playback_mode == "hold" }, Some(s.id));
                }
            }
        }
        Err(e) => log::error!("couldn't load shortcuts: {e}"),
    }
    *state.shortcuts.actions.lock().unwrap_or_else(|e| e.into_inner()) = actions;
    if !problems.is_empty() {
        log::warn!("{} shortcut(s) could not be registered", problems.len());
        let _ = app.emit("shortcut-problems", &problems);
    }
    *state.shortcuts.problems.lock().unwrap_or_else(|e| e.into_inner()) = problems;
    // unregister_all above also dropped the open overlay's keys.
    if super::overlay::is_open(app) {
        set_overlay_keys(app, true);
    }
}

pub fn problems(state: &AppState) -> Vec<ShortcutProblem> {
    state.shortcuts.problems.lock().map(|p| p.clone()).unwrap_or_default()
}

/// The plugin holds its shortcut lock while it calls `handle`, and opening or closing the overlay
/// (un)registers the overlay's keys, which takes that same lock. Run those once the handler has
/// returned, or Blurt freezes with the overlay on screen.
fn after_handler(app: &AppHandle, f: fn(&AppHandle)) {
    let app = app.clone();
    std::thread::spawn(move || f(&app));
}

/// Global hotkey callback (runs on the OS event thread; playback work is pushed off it).
pub fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    let state = app.state::<AppState>();
    let id = shortcut.id();
    let action = state.shortcuts.actions.lock().ok().and_then(|a| a.get(&id).cloned());
    let Some(action) = action else { return };
    match (action, event.state) {
        (Action::StopAll, ShortcutState::Pressed) => state.engine.stop_all(false),
        (Action::ToggleOverlay, ShortcutState::Pressed) => after_handler(app, super::overlay::toggle),
        (Action::OverlayKey(n), ShortcutState::Pressed) => {
            super::overlay::touch(app);
            let _ = app.emit_to(super::overlay::LABEL, "overlay-key", n);
        }
        (Action::OverlayClose, ShortcutState::Pressed) => after_handler(app, super::overlay::hide),
        (Action::SaveClip, ShortcutState::Pressed) => {
            let app = app.clone();
            std::thread::spawn(move || {
                if let Err(e) = super::save_clip(&app) {
                    let _ = app.emit("playback-error", e.to_string());
                }
            });
        }
        (action @ (Action::ReplayLast | Action::PlayRandom), ShortcutState::Pressed) => {
            let app = app.clone();
            std::thread::spawn(move || {
                let state = app.state::<AppState>();
                let r = match action {
                    Action::ReplayLast => crate::playback::replay_last(&app, &state),
                    _ => crate::playback::play_random(&app, &state),
                };
                if let Err(e) = r {
                    let _ = app.emit("playback-error", e.to_string());
                }
            });
        }
        (Action::Play { sound_id, hold }, ShortcutState::Pressed) => {
            let app = app.clone();
            std::thread::spawn(move || {
                let state = app.state::<AppState>();
                let mode = hold.then_some(PlayMode::Restart);
                match crate::playback::play_sound(&app, &state, &sound_id, mode) {
                    Ok(outcome) => {
                        if let (true, Some(voice)) = (hold, outcome.voice_id) {
                            state.shortcuts.held.lock().unwrap_or_else(|e| e.into_inner()).insert(id, voice);
                        }
                    }
                    Err(e) => {
                        log::warn!("shortcut playback failed: {e}");
                        let _ = app.emit("playback-error", e.to_string());
                    }
                }
            });
        }
        (Action::Play { hold: true, .. }, ShortcutState::Released) => {
            if let Some(voice) = state.shortcuts.held.lock().unwrap_or_else(|e| e.into_inner()).remove(&id) {
                state.engine.stop(voice);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalises_equivalent_spellings() {
        let (a, ida) = canonical("Ctrl+Alt+1").unwrap();
        let (b, idb) = canonical("control+alt+Digit1").unwrap();
        assert_eq!((a.as_str(), ida), (b.as_str(), idb));
        assert_eq!(a, "control+alt+Digit1");
        assert!(canonical("A").is_err(), "bare letters would hijack typing");
        assert!(canonical("F9").is_ok(), "function keys are allowed alone");
        assert!(canonical("Ctrl+Nope").is_err());
    }

    #[test]
    fn default_app_shortcuts_are_valid_and_distinct() {
        let s = Settings::default();
        let mut ids: Vec<u32> = globals(&s).into_iter().map(|(sc, ..)| canonical(sc.unwrap()).unwrap().1).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 5);
    }
}
