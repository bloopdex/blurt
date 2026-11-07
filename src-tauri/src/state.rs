use std::sync::{Mutex, RwLock};

use crate::audio::cache::AudioCache;
use crate::audio::engine::{Engine, EngineConfig};
use crate::db::Db;
use crate::desktop::shortcuts::Registry;
use crate::downloader::Downloader;
use crate::paths::Paths;
use crate::settings::Settings;

pub struct AppState {
    pub db: Db,
    pub paths: Paths,
    pub engine: Engine,
    pub cache: AudioCache,
    pub downloader: Downloader,
    pub shortcuts: Registry,
    /// Last sound played to the microphone, for "Replay last".
    pub last_played: Mutex<Option<String>>,
    pub clips: crate::audio::clip::ClipRecorder,
    settings: RwLock<Settings>,
}

impl AppState {
    pub fn new(db: Db, paths: Paths, engine: Engine, settings: Settings) -> Self {
        Self {
            db,
            paths,
            engine,
            cache: AudioCache::default(),
            downloader: Downloader::default(),
            shortcuts: Registry::default(),
            last_played: Mutex::new(None),
            clips: Default::default(),
            settings: RwLock::new(settings),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn replace_settings(&self, next: Settings) {
        *self.settings.write().unwrap_or_else(|e| e.into_inner()) = next;
    }
}

/// The recording endpoint other apps should use: the user's explicit choice, else the playback
/// device's paired recording endpoint from OS metadata (or a known-loopback name match).
pub fn resolve_capture(s: &Settings) -> Option<String> {
    let output = s.virtual_mic_device.as_ref()?;
    s.virtual_mic_capture.clone().or_else(|| {
        crate::audio::devices::list().outputs.into_iter().find(|d| &d.name == output)?.paired_capture
    })
}

pub fn engine_config(s: &Settings) -> EngineConfig {
    EngineConfig {
        virtual_mic_capture: resolve_capture(s),
        virtual_mic: s.virtual_mic_device.clone(),
        monitor_device: s.monitor_device.clone(),
        mic_device: s.mic_device.clone(),
        mic_passthrough: s.mic_passthrough,
        sample_rate: s.sample_rate,
        max_voices: s.max_voices as usize,
    }
}

const ROUTE_CHECKS_KEY: &str = "route_checks";

impl AppState {
    /// Results of past routing tests, newest first, one per (playback, recording) pair.
    pub fn route_checks(&self) -> Vec<crate::audio::routing::VerifyResult> {
        self.db.get_setting(ROUTE_CHECKS_KEY).ok().flatten().and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
    }

    pub fn record_route_check(&self, result: &crate::audio::routing::VerifyResult) -> crate::error::AppResult<()> {
        let mut checks = self.route_checks();
        checks.retain(|c| !(c.output == result.output && c.capture == result.capture));
        checks.insert(0, result.clone());
        checks.truncate(20);
        let json = serde_json::to_string(&checks).map_err(|e| crate::error::AppError::Invalid(e.to_string()))?;
        self.db.put_setting(ROUTE_CHECKS_KEY, &json)
    }
}
