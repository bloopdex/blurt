//! Turning "play sound X" into an engine request: resolve behaviour, fetch decoded buffers for
//! each open device, and report honestly where the sound actually went.

use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::audio::engine::{PlayMode, PlayRequest};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Peak target for normalisation (≈ -3 dBFS), leaving headroom for overlapping sounds.
const NORMALIZE_PEAK: f32 = 0.7;
/// Never boost more than +12 dB, so near-silent files don't become noise blasts.
const MAX_BOOST: f32 = 4.0;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayOutcome {
    pub voice_id: Option<u64>,
    pub on_mic: bool,
    /// Set when the sound did not reach the virtual microphone.
    pub warning: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Played {
    id: String,
    play_count: i64,
    last_played_at: Option<i64>,
}

pub fn mode_from(sound_mode: &str, default_mode: &str) -> PlayMode {
    let m = if sound_mode == "default" { default_mode } else { sound_mode };
    match m {
        "toggle" => PlayMode::Toggle,
        "overlap" => PlayMode::Overlap,
        _ => PlayMode::Restart, // "restart", and "hold" when clicked rather than held
    }
}

pub fn normalization_gain(enabled: bool, peak: f32) -> f32 {
    if !enabled || peak <= 1e-4 { 1.0 } else { (NORMALIZE_PEAK / peak).min(MAX_BOOST) }
}

pub fn play_sound(app: &AppHandle, state: &AppState, id: &str, mode: Option<PlayMode>) -> AppResult<PlayOutcome> {
    let sound = state.db.get_sound(id)?;
    let s = state.settings();
    let path = state.paths.sound_file(&sound.file_name);
    if !path.exists() {
        return Err(AppError::NotFound(format!(
            "The audio file for “{}” is missing from the library folder. Re-import it or restore a backup.",
            sound.name
        )));
    }
    // Right after launch or a routing change the device may still be opening (no error yet):
    // wait briefly rather than fail. Callers are off the main thread (command pool / hotkey thread).
    let mut status = state.engine.status();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while s.virtual_mic_device.is_some()
        && !status.virtual_mic.open
        && status.virtual_mic.error.is_none()
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(50));
        status = state.engine.status();
    }
    let mic_rate = status.virtual_mic.open.then_some(status.virtual_mic.sample_rate).flatten();
    let monitor_rate = (s.monitor_enabled && status.monitor.open).then_some(status.monitor.sample_rate).flatten();
    if mic_rate.is_none() && monitor_rate.is_none() {
        return Err(AppError::Audio(no_output_message(&s.virtual_mic_device, s.monitor_enabled)));
    }

    let (mic_pcm, monitor_pcm, peak) = buffers(state, &path, mic_rate, monitor_rate)?;
    let voice_id = state.engine.play(PlayRequest {
        sound_id: Some(sound.id.clone()),
        name: sound.name.clone(),
        mic_pcm,
        monitor_pcm,
        start_ms: sound.trim_start_ms,
        end_ms: sound.trim_end_ms,
        gain: sound.volume as f32 * normalization_gain(s.normalize, peak),
        fade_in_ms: s.fade_in_ms,
        fade_out_ms: s.fade_out_ms,
        mode: mode.unwrap_or_else(|| mode_from(&sound.playback_mode, &s.playback_mode)),
        exclusive: s.stop_previous,
        preview: false,
    });

    if voice_id.is_some() {
        *state.last_played.lock().unwrap_or_else(|e| e.into_inner()) = Some(sound.id.clone());
        state.db.record_play(id)?;
        let _ = app.emit(
            "sound-played",
            Played { id: sound.id, play_count: sound.play_count + 1, last_played_at: Some(crate::error::now_ms()) },
        );
    }
    let on_mic = mic_rate.is_some();
    // An open device isn't proof: the latest routing test for this playback device may say otherwise.
    let failed_test = s.virtual_mic_device.as_ref().and_then(|d| state.route_checks().into_iter().find(|c| &c.output == d));
    let warning = match (&s.virtual_mic_device, on_mic, failed_test) {
        (None, _, _) => Some("No virtual microphone is set up, so only you can hear this. Open Audio setup to fix it.".to_owned()),
        (Some(d), false, _) => Some(format!("The virtual microphone “{d}” is unavailable, so only you can hear this.")),
        (Some(d), true, c) if c.as_ref().is_none_or(|c| !c.signal_received) && is_hardware_output(d) => Some(format!(
            "“{d}” is your real speakers or headphones, so only people in the room hear this, not anyone on a call. Choose a virtual cable in Audio setup (or press Set up automatically)."
        )),
        (Some(d), true, Some(c)) if !c.signal_received => Some(format!(
            "Your last routing test showed “{d}” doesn't reach “{}”, so people may not hear this. Pick a virtual cable in Audio setup.",
            c.capture
        )),
        _ => None,
    };
    Ok(PlayOutcome { voice_id, on_mic, warning })
}

/// Play the last sound again from the start.
pub fn replay_last(app: &AppHandle, state: &AppState) -> AppResult<PlayOutcome> {
    let last = state.last_played.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let id = last.ok_or_else(|| AppError::NotFound("Nothing to replay yet: play a sound first.".into()))?;
    play_sound(app, state, &id, Some(PlayMode::Restart))
}

/// Play a random sound from the library, avoiding an immediate repeat.
pub fn play_random(app: &AppHandle, state: &AppState) -> AppResult<PlayOutcome> {
    let last = state.last_played.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut ids: Vec<String> = state.db.list_sounds()?.into_iter().map(|s| s.id).collect();
    if ids.len() > 1 {
        ids.retain(|id| Some(id) != last.as_ref());
    }
    if ids.is_empty() {
        return Err(AppError::NotFound("Your library is empty.".into()));
    }
    let pick = (uuid::Uuid::new_v4().as_u128() % ids.len() as u128) as usize;
    play_sound(app, state, &ids[pick], None)
}

/// Physical playback device per OS metadata (cfgmgr only, no audio enumeration, so cheap per play).
fn is_hardware_output(name: &str) -> bool {
    crate::audio::endpoints::list().iter().any(|e| !e.capture && e.name == name && !e.software())
}

fn no_output_message(virtual_mic: &Option<String>, monitor_enabled: bool) -> String {
    match (virtual_mic, monitor_enabled) {
        (None, false) => "Nothing to play to: no virtual microphone is set up and monitoring is off.".into(),
        (None, true) => "No audio device is available. Check that your speakers or headphones are connected.".into(),
        (Some(d), _) => format!("“{d}” is unavailable and there's no monitor output. Reconnect it or pick another device in Audio setup."),
    }
}

type Buffers = (Option<std::sync::Arc<crate::audio::decode::Pcm>>, Option<std::sync::Arc<crate::audio::decode::Pcm>>, f32);

fn buffers(state: &AppState, path: &Path, mic_rate: Option<u32>, monitor_rate: Option<u32>) -> AppResult<Buffers> {
    let mut rates: Vec<u32> = [mic_rate, monitor_rate].into_iter().flatten().collect();
    rates.dedup();
    let bufs = state.cache.get(path, &rates)?;
    let pick = |r: Option<u32>| r.and_then(|r| bufs.iter().find(|b| b.rate == r).cloned());
    let peak = bufs.first().map(|b| b.peak).unwrap_or(1.0);
    Ok((pick(mic_rate), pick(monitor_rate), peak))
}

/// Preview to the monitor only — previews never go out over the virtual microphone.
pub fn preview(state: &AppState, path: &Path, name: &str, start_ms: i64, end_ms: Option<i64>, volume: f32) -> AppResult<Option<u64>> {
    let status = state.engine.status();
    let Some(rate) = status.monitor.open.then_some(status.monitor.sample_rate).flatten() else {
        return Err(AppError::Audio(
            "Previews play on your speakers, but no output device is available. Check Audio setup.".into(),
        ));
    };
    let s = state.settings();
    let (_, pcm, peak) = buffers(state, path, None, Some(rate))?;
    Ok(state.engine.play(PlayRequest {
        sound_id: None,
        name: name.to_owned(),
        mic_pcm: None,
        monitor_pcm: pcm,
        start_ms,
        end_ms,
        gain: volume * normalization_gain(s.normalize, peak),
        fade_in_ms: 0,
        fade_out_ms: 30,
        mode: PlayMode::Restart,
        exclusive: false,
        preview: true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_modes() {
        assert_eq!(mode_from("default", "toggle"), PlayMode::Toggle);
        assert_eq!(mode_from("overlap", "toggle"), PlayMode::Overlap);
        assert_eq!(mode_from("hold", "toggle"), PlayMode::Restart);
        assert_eq!(mode_from("default", "restart"), PlayMode::Restart);
    }

    #[test]
    fn normalization_is_bounded() {
        assert_eq!(normalization_gain(false, 0.1), 1.0);
        assert!((normalization_gain(true, 0.35) - 2.0).abs() < 1e-6);
        assert_eq!(normalization_gain(true, 0.01), MAX_BOOST);
        assert!(normalization_gain(true, 1.0) < 1.0, "hot files are turned down");
        assert_eq!(normalization_gain(true, 0.0), 1.0);
    }
}
