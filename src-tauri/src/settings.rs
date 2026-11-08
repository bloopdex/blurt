//! User settings: one JSON document in the `settings` table. Unknown/missing fields fall back
//! to defaults so older databases keep loading after upgrades.

use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::error::{AppError, AppResult};

const KEY: &str = "app";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    // General
    pub theme: String,
    pub start_at_login: bool,
    pub launch_minimized: bool,
    pub close_to_tray: bool,
    pub confirm_delete: bool,
    pub show_notifications: bool,
    pub show_dock_icon: bool,
    pub onboarding_done: bool,
    /// Check GitHub for a newer signed release at startup (installing always asks first).
    pub auto_update: bool,
    // Audio routing
    /// Output device that feeds the virtual microphone (e.g. "CABLE Input (VB-Audio Virtual Cable)").
    pub virtual_mic_device: Option<String>,
    /// Recording endpoint other apps use as their microphone (the cable's other side). `None` =
    /// the driver's paired recording endpoint. This is where the routing test listens.
    pub virtual_mic_capture: Option<String>,
    /// Where you hear sounds yourself. `None` = system default output.
    pub monitor_device: Option<String>,
    pub monitor_enabled: bool,
    /// Physical microphone mixed into the virtual mic so your voice still reaches the call.
    pub mic_device: Option<String>,
    pub mic_passthrough: bool,
    /// Lower the passthrough mic while a sound plays, so the sound comes through clearly.
    pub duck_mic: bool,
    pub mic_volume: f32,
    pub master_volume: f32,
    /// Applied to everything sent to the virtual mic (soundboard + passthrough mic) together.
    pub output_volume: f32,
    pub monitor_volume: f32,
    pub default_sound_volume: f32,
    /// Preferred device sample rate; `None` uses each device's own default.
    pub sample_rate: Option<u32>,
    pub max_voices: u32,
    // Playback
    pub playback_mode: String,
    pub stop_previous: bool,
    pub fade_in_ms: u32,
    pub fade_out_ms: u32,
    pub normalize: bool,
    // Shortcuts
    pub stop_all_shortcut: Option<String>,
    pub replay_last_shortcut: Option<String>,
    /// Shows or hides the in-game overlay.
    pub overlay_shortcut: Option<String>,
    pub random_sound_shortcut: Option<String>,
    // Gaming & streaming
    /// Key Blurt holds down while a sound plays, for push-to-talk voice chats ("KeyV", "Mouse4"…).
    pub ptt_key: Option<String>,
    /// Keep the last `clip_seconds` of what you hear, ready to save as a sound.
    pub clip_recording: bool,
    pub clip_seconds: u32,
    pub clip_shortcut: Option<String>,
    /// Local HTTP control for Stream Deck, Touch Portal, Streamer.bot…
    pub remote_control: bool,
    pub remote_token: String,
    /// Accept "Send to Blurt" from the browser extension (extension origins only).
    pub browser_extension: bool,
    /// Silences the passthrough mic without losing its level.
    pub mic_muted: bool,
    // Interface
    pub grid_density: String,
    pub sort: String,
    pub sidebar_collapsed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            start_at_login: false,
            launch_minimized: false,
            close_to_tray: true,
            confirm_delete: true,
            show_notifications: true,
            show_dock_icon: true,
            onboarding_done: false,
            auto_update: true,
            virtual_mic_device: None,
            virtual_mic_capture: None,
            monitor_device: None,
            monitor_enabled: true,
            mic_device: None,
            mic_passthrough: false,
            duck_mic: false,
            mic_volume: 1.0,
            master_volume: 0.8,
            output_volume: 1.0,
            monitor_volume: 0.6,
            default_sound_volume: 1.0,
            sample_rate: None,
            max_voices: 8,
            playback_mode: "restart".into(),
            stop_previous: false,
            fade_in_ms: 0,
            fade_out_ms: 60,
            normalize: true,
            stop_all_shortcut: Some("CommandOrControl+Alt+Period".into()),
            replay_last_shortcut: Some("CommandOrControl+Alt+Comma".into()),
            overlay_shortcut: Some("CommandOrControl+Alt+KeyB".into()),
            random_sound_shortcut: Some("CommandOrControl+Alt+KeyR".into()),
            ptt_key: None,
            clip_recording: false,
            clip_seconds: 30,
            clip_shortcut: Some("CommandOrControl+Alt+KeyC".into()),
            remote_control: false,
            remote_token: String::new(),
            browser_extension: true,
            mic_muted: false,
            grid_density: "comfortable".into(),
            sort: "manual".into(),
            sidebar_collapsed: false,
        }
    }
}

/// Mic level while ducked: −12 dB.
const DUCK_LEVEL: f32 = 0.25;

impl Settings {
    pub fn mic_level(&self) -> f32 {
        if self.mic_muted { 0.0 } else { self.mic_volume }
    }

    /// The local server runs for remote control or for the browser extension.
    pub fn local_server(&self) -> bool {
        self.remote_control || self.browser_extension
    }

    pub fn mic_duck_level(&self) -> f32 {
        if self.duck_mic { DUCK_LEVEL } else { 1.0 }
    }

    pub fn load(db: &Db) -> AppResult<Self> {
        match db.get_setting(KEY)? {
            Some(json) => serde_json::from_str(&json).or_else(|e| {
                log::error!("settings are unreadable ({e}); using defaults");
                Ok(Self::default())
            }),
            None => Ok(Self::default()),
        }
    }

    pub fn save(&self, db: &Db) -> AppResult<()> {
        let json = serde_json::to_string(self).map_err(|e| AppError::Invalid(e.to_string()))?;
        db.put_setting(KEY, &json)
    }

    /// Merge a partial JSON object from the UI and validate the result.
    pub fn merged(&self, patch: serde_json::Value) -> AppResult<Self> {
        let mut base = serde_json::to_value(self).map_err(|e| AppError::Invalid(e.to_string()))?;
        let (Some(obj), serde_json::Value::Object(p)) = (base.as_object_mut(), patch) else {
            return Err(AppError::Invalid("Settings update must be an object.".into()));
        };
        for (k, v) in p {
            if !obj.contains_key(&k) {
                return Err(AppError::Invalid(format!("Unknown setting '{k}'.")));
            }
            obj.insert(k, v);
        }
        let next: Settings =
            serde_json::from_value(base).map_err(|e| AppError::Invalid(format!("Invalid setting value: {e}")))?;
        next.validate()?;
        Ok(next)
    }

    fn validate(&self) -> AppResult<()> {
        let bad = |m: &str| Err(AppError::Invalid(m.into()));
        if !["dark", "light", "system"].contains(&self.theme.as_str()) {
            return bad("Theme must be dark, light or system.");
        }
        if !["restart", "toggle", "overlap"].contains(&self.playback_mode.as_str()) {
            return bad("Unknown default playback mode.");
        }
        if !["compact", "comfortable", "large"].contains(&self.grid_density.as_str()) {
            return bad("Unknown grid density.");
        }
        if !["manual", "name", "recent", "plays", "duration"].contains(&self.sort.as_str()) {
            return bad("Unknown sort order.");
        }
        for v in [self.master_volume, self.output_volume, self.monitor_volume, self.mic_volume, self.default_sound_volume] {
            if !(0.0..=2.0).contains(&v) {
                return bad("Volumes must be between 0% and 200%.");
            }
        }
        if !(1..=32).contains(&self.max_voices) {
            return bad("Simultaneous sounds must be between 1 and 32.");
        }
        if self.fade_in_ms > 5000 || self.fade_out_ms > 5000 {
            return bad("Fades are limited to 5 seconds.");
        }
        if !(5..=120).contains(&self.clip_seconds) {
            return bad("Clips must be between 5 and 120 seconds.");
        }
        if let Some(k) = &self.ptt_key
            && !crate::desktop::ptt::is_supported(k)
        {
            return bad("That push-to-talk key isn't supported. Use a letter, number, F-key, Caps Lock or a mouse side button.");
        }
        if let Some(r) = self.sample_rate
            && !(8000..=192_000).contains(&r)
        {
            return bad("Unsupported sample rate.");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_validates_and_rejects_unknown_keys() {
        let s = Settings::default();
        let n = s.merged(serde_json::json!({"masterVolume": 0.5, "theme": "light"})).unwrap();
        assert_eq!((n.master_volume, n.theme.as_str()), (0.5, "light"));
        assert!(s.merged(serde_json::json!({"masterVolume": 5})).is_err());
        assert!(s.merged(serde_json::json!({"nope": 1})).is_err());
        assert!(s.merged(serde_json::json!({"theme": "neon"})).is_err());
        assert!(s.merged(serde_json::json!({"virtualMicDevice": null})).is_ok());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let db = Db::in_memory().unwrap();
        db.put_setting(KEY, r#"{"theme":"light"}"#).unwrap();
        let s = Settings::load(&db).unwrap();
        assert_eq!(s.theme, "light");
        assert_eq!(s.max_voices, Settings::default().max_voices);
    }
}
