//! Virtual microphone routing.
//!
//! Neither Windows nor macOS lets a user-mode app create a microphone. Blurt plays into the
//! *playback* side of a loopback driver (VB-CABLE, BlackHole, …); the driver exposes a matching
//! *recording* endpoint that Discord/Zoom/games select as their microphone.
//!
//! Classification, strongest evidence first:
//! 1. **Signal test** (`verify`) — the only proof that audio played into A comes out of B.
//! 2. **OS metadata** (`endpoints.rs`) — real direction, and whether both endpoints belong to the
//!    same software driver. That makes them a *candidate* pair, nothing more: NVIDIA Broadcast has
//!    both directions on one driver and does not loop.
//! 3. **Name hints** — recognise well-known loopback drivers. A hint never changes a direction.
//!
//! See docs/ARCHITECTURE.md (ADR-001, ADR-006).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::StreamTrait;
use serde::{Deserialize, Serialize};

use super::devices::{find_input, find_output, open_input, open_output, pick_config};
use super::endpoints::Endpoint;

/// (substring of a device name, driver family) for drivers known to loop playback into recording.
const KNOWN_LOOPBACKS: &[(&str, &str)] = &[
    ("vb-audio virtual cable", "VB-CABLE"),
    ("vb-audio cable", "VB-CABLE A+B"),
    ("vb-audio hi-fi cable", "VB-Audio Hi-Fi Cable"),
    ("voicemeeter", "Voicemeeter"),
    ("blackhole", "BlackHole"),
    ("loopback audio", "Loopback"),
    ("virtual audio cable", "Virtual Audio Cable"),
    ("steam streaming microphone", "Steam Streaming Microphone"),
];

/// Name hint: which known loopback family a device belongs to, if any.
pub fn known_loopback(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    KNOWN_LOOPBACKS.iter().find(|(needle, _)| lower.contains(needle)).map(|(_, p)| (*p).to_owned())
}

/// The recording endpoint that playback into `output` would come out of, if it loops.
/// With OS metadata: the capture endpoint on the same software driver. Without metadata (macOS for
/// now): name pairing, only for known loopback drivers.
pub fn paired_capture(output: &str, endpoints: &[Endpoint], input_names: &[String]) -> Option<String> {
    if let Some(ep) = endpoints.iter().find(|e| !e.capture && e.name == output) {
        if !ep.software() {
            return None; // hardware outputs never loop into a microphone
        }
        let siblings: Vec<&Endpoint> = endpoints.iter().filter(|e| e.capture && e.adapter == ep.adapter).collect();
        let by_name = name_candidates(output);
        return siblings
            .iter()
            .find(|s| by_name.iter().any(|c| s.name.eq_ignore_ascii_case(c)))
            .or(siblings.first())
            .map(|s| s.name.clone());
    }
    known_loopback(output)?;
    let by_name = name_candidates(output);
    by_name.iter().find_map(|c| input_names.iter().find(|i| i.eq_ignore_ascii_case(c))).cloned()
}

/// "CABLE Input" ↔ "CABLE Output", "Speakers (X)" ↔ "Microphone (X)", BlackHole uses one name.
fn name_candidates(output: &str) -> [String; 3] {
    [output.replace("Input", "Output"), output.replace("Speakers", "Microphone"), output.to_owned()]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResult {
    pub output: String,
    pub capture: String,
    /// The test tone was found on the recording side, clearly above its noise floor.
    pub signal_received: bool,
    /// Received level and signal-to-noise ratio are good enough for a call.
    pub quality_ok: bool,
    /// `signal_received && quality_ok`.
    pub ok: bool,
    pub expected_db: f32,
    pub received_db: f32,
    pub noise_db: f32,
    pub message: String,
    pub checked_at: i64,
}

const TONE_HZ: f32 = 1000.0;
const TONE_AMP: f32 = 0.25; // -12 dBFS
/// How far below the sent level "Fix level" aims, so loud sounds keep headroom.
pub const FIX_HEADROOM_DB: f32 = 6.0;

/// Anything quieter than this below what was sent counts as poor quality.
const MAX_LOSS_DB: f32 = 18.0;
const MIN_SNR_DB: f32 = 20.0;

/// End-to-end proof of a route: play a 1 kHz tone into `output` and listen for it on `capture`,
/// exactly the path Discord would read from. Existence of a device proves nothing.
pub fn verify(output_name: &str, capture_name: &str) -> Result<VerifyResult, String> {
    let out_dev = find_output(Some(output_name))?;
    let in_dev = find_input(Some(capture_name))?;
    let out_cfg = pick_config(&out_dev, true, None)?;
    let in_cfg = pick_config(&in_dev, false, None)?;
    let out_rate = out_cfg.sample_rate().0 as usize;
    let in_rate = in_cfg.sample_rate().0;

    let captured: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(in_rate as usize * 2)));
    let sink = captured.clone();
    let input = open_input(
        &in_dev,
        &in_cfg,
        move |data, ch| {
            if let Ok(mut v) = sink.try_lock() {
                v.extend(data.chunks_exact(ch).map(|f| f.iter().sum::<f32>() / ch as f32));
            }
        },
        |e| log::warn!("verify input error: {e}"),
    )?;

    // Tone from 300 ms to 1100 ms; the silence before it is the noise-floor reference.
    let mut n = 0usize;
    let (on, off) = (out_rate * 3 / 10, out_rate * 11 / 10);
    let output = open_output(
        &out_dev,
        &out_cfg,
        move |buf, ch| {
            for frame in buf.chunks_exact_mut(ch) {
                let v = if (on..off).contains(&n) {
                    (n as f32 * TONE_HZ * std::f32::consts::TAU / out_rate as f32).sin() * TONE_AMP
                } else {
                    0.0
                };
                frame.fill(v);
                n += 1;
            }
        },
        |e| log::warn!("verify output error: {e}"),
    )?;
    input.play().map_err(|e| e.to_string())?;
    output.play().map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(1500));
    drop(output);
    drop(input);

    let samples = captured.lock().map_err(|_| "capture buffer poisoned")?.clone();
    Ok(analyse(&samples, in_rate, output_name, capture_name))
}

fn db(v: f32) -> f32 {
    20.0 * v.max(1e-6).log10()
}

fn analyse(samples: &[f32], rate: u32, output: &str, capture: &str) -> VerifyResult {
    let expected_db = db(TONE_AMP);
    let window = (rate / 50) as usize; // 20 ms
    let result = |signal_received, quality_ok, received_db, noise_db, message: String| VerifyResult {
        output: output.into(),
        capture: capture.into(),
        signal_received,
        quality_ok,
        ok: signal_received && quality_ok,
        expected_db,
        received_db,
        noise_db,
        message,
        checked_at: crate::error::now_ms(),
    };
    if samples.len() < window * 10 {
        return result(
            false,
            false,
            -120.0,
            -120.0,
            format!("“{capture}” delivered no audio at all. Make sure it isn't disabled, muted, or in exclusive use by another app."),
        );
    }
    let levels: Vec<f32> = samples.chunks_exact(window).map(|w| goertzel(w, TONE_HZ, rate)).collect();
    let noise = levels[..levels.len().min(10)].iter().cloned().fold(0f32, f32::max);
    let tone = levels.iter().cloned().fold(0f32, f32::max);
    let (received_db, noise_db) = (db(tone), db(noise));
    let snr = received_db - noise_db;
    let received = tone > 0.002 && snr >= 12.0;
    let loss = expected_db - received_db;
    let quality = received && loss <= MAX_LOSS_DB && snr >= MIN_SNR_DB;

    let message = if quality {
        format!("Signal received at {received_db:.0} dBFS (sent {expected_db:.0}). Apps using “{capture}” as their microphone will hear your sounds.")
    } else if received && loss > MAX_LOSS_DB {
        format!("The signal arrives, but {loss:.0} dB quieter than sent ({received_db:.0} dBFS). People will hear your sounds faintly — press “Fix level” to raise this microphone's input level, or use a cable that passes audio at full level (e.g. VB-CABLE).")
    } else if received {
        format!("The signal arrives, but only {snr:.0} dB above background noise. Something else is feeding “{capture}” — check for other apps or effects on that device.")
    } else {
        format!("The test tone never reached “{capture}”. Audio played into “{output}” doesn't come out of that microphone, so they aren't a loopback pair. Some software devices (for example noise-removal apps) process audio and send it to your speakers instead.")
    };
    result(received, quality, received_db, noise_db, message)
}

/// Amplitude of one frequency in a block (Goertzel algorithm).
fn goertzel(block: &[f32], freq: f32, rate: u32) -> f32 {
    let w = std::f32::consts::TAU * freq / rate as f32;
    let coeff = 2.0 * w.cos();
    let (mut s1, mut s2) = (0f32, 0f32);
    for &x in block {
        let s0 = x + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let power = (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0);
    2.0 * power.sqrt() / block.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn ep(name: &str, capture: bool, adapter: &str) -> Endpoint {
        Endpoint { name: name.into(), capture, adapter: adapter.into() }
    }

    /// The real device tree from the Windows test machine, plus VB-CABLE.
    fn machine() -> Vec<Endpoint> {
        vec![
            ep("Speakers (NVIDIA Broadcast)", false, "ROOT\\UNNAMED_DEVICE\\0000"),
            ep("Microphone (NVIDIA Broadcast)", true, "ROOT\\UNNAMED_DEVICE\\0000"),
            ep("Speakers (Steam Streaming Microphone)", false, "ROOT\\SteamStreamingMicrophone\\0000"),
            ep("Microphone (Steam Streaming Microphone)", true, "ROOT\\SteamStreamingMicrophone\\0000"),
            ep("Speakers (Steam Streaming Speakers)", false, "ROOT\\SteamStreamingSpeakers\\0000"),
            ep("Speakers (Realtek(R) Audio)", false, "HDAUDIO\\FUNC_01&VEN_10EC\\4&1&0001"),
            ep("Microphone (Realtek(R) Audio)", true, "HDAUDIO\\FUNC_01&VEN_10EC\\4&1&0001"),
            ep("CABLE Input (VB-Audio Virtual Cable)", false, "ROOT\\VBAudioVACWDM\\0000"),
            ep("CABLE Output (VB-Audio Virtual Cable)", true, "ROOT\\VBAudioVACWDM\\0000"),
        ]
    }

    #[test]
    fn pairs_by_driver_metadata_not_names() {
        let eps = machine();
        let pair = |o| paired_capture(o, &eps, &[]);
        assert_eq!(pair("CABLE Input (VB-Audio Virtual Cable)").as_deref(), Some("CABLE Output (VB-Audio Virtual Cable)"));
        // Same software driver ⇒ candidate pair (only the signal test can confirm or reject it).
        assert_eq!(pair("Speakers (NVIDIA Broadcast)").as_deref(), Some("Microphone (NVIDIA Broadcast)"));
        // A hardware codec has a mic too, but speakers never loop into it.
        assert_eq!(pair("Speakers (Realtek(R) Audio)"), None);
        // Software output with no recording side.
        assert_eq!(pair("Speakers (Steam Streaming Speakers)"), None);
    }

    #[test]
    fn name_hints_are_hints_only() {
        assert_eq!(known_loopback("CABLE Input (VB-Audio Virtual Cable)").as_deref(), Some("VB-CABLE"));
        assert_eq!(known_loopback("Speakers (NVIDIA Broadcast)"), None, "NVIDIA Broadcast is not a loopback driver");
        // Without metadata (macOS), only known loopbacks pair, by name.
        let inputs = names(&["BlackHole 2ch", "MacBook Pro Microphone"]);
        assert_eq!(paired_capture("BlackHole 2ch", &[], &inputs).as_deref(), Some("BlackHole 2ch"));
        assert_eq!(paired_capture("MacBook Pro Speakers", &[], &inputs), None);
        // Metadata wins over a name hint: a hardware endpoint is never paired.
        let eps = vec![ep("Virtual Audio Cable Speakers", false, "USB\\VID_1\\1"), ep("Virtual Audio Cable Mic", true, "USB\\VID_1\\1")];
        assert_eq!(paired_capture("Virtual Audio Cable Speakers", &eps, &[]), None);
    }

    #[test]
    fn goertzel_measures_tone_amplitude() {
        let rate = 48000;
        let tone: Vec<f32> =
            (0..960).map(|i| (i as f32 * TONE_HZ * std::f32::consts::TAU / rate as f32).sin() * 0.25).collect();
        assert!((goertzel(&tone, TONE_HZ, rate) - 0.25).abs() < 0.01);
        let other: Vec<f32> = (0..960).map(|i| (i as f32 * 3000.0 * std::f32::consts::TAU / 48000.0).sin()).collect();
        assert!(goertzel(&other, TONE_HZ, rate) < 0.01);
    }

    fn capture(noise: f32, tone: f32) -> Vec<f32> {
        let rate = 48000.0;
        // Deterministic broadband "noise": a 3 kHz component plus a 1 kHz floor of `noise`.
        let n = |i: usize| (i as f32 * 3000.0 * std::f32::consts::TAU / rate).sin() * 0.01 + (i as f32 * TONE_HZ * std::f32::consts::TAU / rate).sin() * noise;
        let mut s: Vec<f32> = (0..14400).map(n).collect();
        s.extend((14400..52800).map(|i| n(i) + (i as f32 * TONE_HZ * std::f32::consts::TAU / rate).sin() * tone));
        s
    }

    #[test]
    fn analyse_separates_received_from_quality() {
        let a = |s: &[f32]| analyse(s, 48000, "out", "in");
        let good = a(&capture(0.0, 0.2));
        assert!(good.signal_received && good.quality_ok && good.ok, "{good:?}");

        let faint = a(&capture(0.0, 0.006)); // ≈ -44 dBFS: received, but too quiet
        assert!(faint.signal_received && !faint.quality_ok, "{faint:?}");

        let noisy = a(&capture(0.05, 0.3)); // loud, but only ~16 dB above a 1 kHz floor
        assert!(noisy.signal_received && !noisy.quality_ok, "{noisy:?}");

        let none = a(&capture(0.001, 0.0));
        assert!(!none.signal_received && none.message.contains("never reached"), "{none:?}");

        assert!(!a(&[]).signal_received);
    }
}
