//! Tests against real audio hardware. Ignored by default (CI has no sound devices); run with
//! `cargo test -- --ignored --nocapture` on a machine with a virtual cable installed.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::StreamTrait;

use super::decode::Pcm;
use super::devices::{find_input, list, open_input, pick_config};
use super::engine::{Engine, EngineConfig, EngineEvent, PlayMode, PlayRequest, Volumes};
use super::mixer::AtomicF32;
use super::routing::verify;

fn first_cable() -> Option<(String, String)> {
    let devices = list();
    println!("outputs: {:#?}", devices.outputs.iter().map(|d| (&d.name, &d.known_loopback, &d.paired_capture)).collect::<Vec<_>>());
    // A known loopback family with a paired recording endpoint (VB-CABLE, Steam, BlackHole…).
    devices.outputs.into_iter().find_map(|d| d.paired_capture.filter(|_| d.known_loopback.is_some()).map(|c| (d.name, c)))
}

/// Direction and metadata against the real machine: every software output's pair is a
/// recording endpoint, hardware outputs never pair, and no name appears as the wrong direction.
#[test]
#[ignore = "needs real audio devices"]
fn real_devices_are_classified_by_direction() {
    let l = list();
    for d in &l.outputs {
        println!("OUT {:<48} software={:<5} known={:?} pair={:?} adapter={:?}", d.name, d.software, d.known_loopback, d.paired_capture, d.adapter);
        if let Some(p) = &d.paired_capture {
            assert!(d.software, "{} is hardware but paired", d.name);
            assert!(l.inputs.iter().any(|i| &i.name == p), "{p} must be a recording endpoint");
        }
    }
    for d in &l.inputs {
        println!("IN  {:<48} software={:<5} known={:?} adapter={:?}", d.name, d.software, d.known_loopback, d.adapter);
        assert!(d.paired_capture.is_none());
    }
    #[cfg(windows)]
    assert!(l.outputs.iter().chain(&l.inputs).all(|d| d.adapter.is_some()), "every endpoint has OS metadata");
}

#[test]
#[ignore = "needs a virtual audio cable"]
fn virtual_cable_tone_round_trip() {
    let (out, cap) = first_cable().expect("no virtual cable with a capture side found");
    let r = verify(&out, &cap).expect("verify ran");
    println!("{out} -> {cap}: {r:?}");
    assert!(r.received_db > -60.0, "no test tone reached the virtual microphone: {}", r.message);
}

/// Diagnostic for any pair: `BLURT_OUT="Speakers (X)" BLURT_IN="Microphone (X)" cargo test probe_pair -- --ignored --nocapture`
#[test]
#[ignore = "diagnostic; needs BLURT_OUT and BLURT_IN"]
fn probe_pair() {
    let (Ok(out), Ok(cap)) = (std::env::var("BLURT_OUT"), std::env::var("BLURT_IN")) else { return };
    println!("{out} -> {cap}: {:?}", verify(&out, &cap).expect("verify ran"));
}

/// Every software playback device × every recording device: which pairs carry sound.
#[test]
#[ignore = "diagnostic: plays test tones into every software output"]
fn probe_all_pairs() {
    let list = crate::audio::devices::list();
    for out in list.outputs.iter().filter(|d| d.software) {
        for cap in &list.inputs {
            match verify(&out.name, &cap.name) {
                Ok(r) => println!("{} -> {}: received={} quality={} {:.1} dB (noise {:.1})", out.name, cap.name, r.signal_received, r.quality_ok, r.received_db, r.noise_db),
                Err(e) => println!("{} -> {}: error {e}", out.name, cap.name),
            }
        }
    }
}

/// Passthrough with every real microphone: speakers beep → room → mic → Blurt passthrough →
/// cable → cable's microphone. Acoustic, so it needs speakers on and audible to the mics.
#[test]
#[ignore = "diagnostic: beeps on the default speakers"]
fn passthrough_from_every_microphone() {
    let (out, cap) = first_cable().expect("no virtual cable with a capture side found");
    let l = list();
    let room = l.outputs.iter().find(|d| d.is_default).expect("default output").name.clone();
    for mic in l.inputs.iter().filter(|d| d.name != cap) {
        let engine = Engine::start(
            EngineConfig {
                virtual_mic: Some(out.clone()),
                virtual_mic_capture: Some(cap.clone()),
                monitor_device: None,
                mic_device: Some(mic.name.clone()),
                mic_passthrough: true,
                sample_rate: None,
                max_voices: 8,
            },
            Volumes {
                master: Arc::new(AtomicF32::new(1.0)),
                monitor: Arc::new(AtomicF32::new(0.0)),
                mic: Arc::new(AtomicF32::new(1.0)),
                output: Arc::new(AtomicF32::new(1.0)),
            },
            |_| {},
        );
        std::thread::sleep(Duration::from_millis(1500));
        let st = engine.status();
        let r = verify(&room, &cap).expect("verify ran");
        println!(
            "mic {:<40} passthrough open={} err={:?} -> {cap}: received={} {:.1} dBFS (noise {:.1})",
            mic.name, st.passthrough.open, st.passthrough.error, r.signal_received, r.received_db, r.noise_db
        );
        drop(engine);
    }
}

/// The real product path: engine → mixer → virtual cable output → cable "microphone".
#[test]
#[ignore = "needs a virtual audio cable"]
fn engine_plays_into_virtual_microphone() {
    let (out, cap) = first_cable().expect("no virtual cable with a capture side found");
    let status_rate = Arc::new(AtomicUsize::new(0));
    let rate_seen = status_rate.clone();
    let engine = Engine::start(
        EngineConfig {
            virtual_mic: Some(out.clone()),
            virtual_mic_capture: Some(cap.clone()),
            monitor_device: None,
            mic_device: None,
            mic_passthrough: false,
            sample_rate: None,
            max_voices: 8,
        },
        Volumes {
            master: Arc::new(AtomicF32::new(1.0)),
            monitor: Arc::new(AtomicF32::new(0.0)),
            mic: Arc::new(AtomicF32::new(1.0)),
            output: Arc::new(AtomicF32::new(1.0)),
        },
        move |ev| {
            if let EngineEvent::Status(s) = ev
                && let Some(r) = s.virtual_mic.sample_rate
            {
                rate_seen.store(r as usize, Ordering::Relaxed);
            }
        },
    );
    for _ in 0..50 {
        if status_rate.load(Ordering::Relaxed) != 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let rate = status_rate.load(Ordering::Relaxed) as u32;
    assert!(rate > 0, "virtual mic sink never opened: {:?}", engine.status());

    // Capture on the cable's microphone side, like Discord would.
    let in_dev = find_input(Some(&cap)).unwrap();
    let in_cfg = pick_config(&in_dev, false, None).unwrap();
    let in_rate = in_cfg.sample_rate().0;
    let captured = Arc::new(Mutex::new(Vec::<f32>::new()));
    let sink = captured.clone();
    let recording = Arc::new(AtomicBool::new(true));
    let rec = recording.clone();
    let stream = open_input(
        &in_dev,
        &in_cfg,
        move |data, ch| {
            if rec.load(Ordering::Relaxed)
                && let Ok(mut v) = sink.try_lock()
            {
                v.extend(data.chunks_exact(ch).map(|f| f.iter().sum::<f32>() / ch as f32));
            }
        },
        |e| eprintln!("capture error: {e}"),
    )
    .unwrap();
    stream.play().unwrap();
    std::thread::sleep(Duration::from_millis(300));

    // A 660 Hz "sound" as if decoded from a file.
    let frames = rate as usize; // 1 s
    let samples: Vec<f32> = (0..frames)
        .flat_map(|i| {
            let v = (i as f32 * 660.0 * std::f32::consts::TAU / rate as f32).sin() * 0.3;
            [v, v]
        })
        .collect();
    let pcm = Arc::new(Pcm { samples, rate, peak: 0.3 });
    let voice = engine.play(PlayRequest {
        sound_id: Some("test".into()),
        name: "tone".into(),
        mic_pcm: Some(pcm),
        monitor_pcm: None,
        start_ms: 0,
        end_ms: None,
        gain: 1.0,
        fade_in_ms: 0,
        fade_out_ms: 0,
        mode: PlayMode::Restart,
        exclusive: false,
        preview: false,
    });
    assert!(voice.is_some(), "engine refused to play");
    std::thread::sleep(Duration::from_millis(1300));
    recording.store(false, Ordering::Relaxed);
    drop(stream);

    let s = captured.lock().unwrap().clone();
    let window = (in_rate / 50) as usize;
    let levels: Vec<f32> = s.chunks_exact(window).map(|w| tone_level(w, 660.0, in_rate)).collect();
    // First ~250 ms were recorded before playback started: that's the noise floor.
    let floor = levels.iter().take(12).cloned().fold(0f32, f32::max);
    let best = levels.iter().cloned().fold(0f32, f32::max);
    println!(
        "captured {} samples at {in_rate} Hz; 660 Hz: floor {:.1} dBFS, received {:.1} dBFS (sent -10.5 dBFS)",
        s.len(),
        20.0 * floor.max(1e-7).log10(),
        20.0 * best.max(1e-7).log10()
    );
    assert!(best > 0.001 && best > floor * 10.0, "the sound did not arrive on the virtual microphone");
}

fn tone_level(w: &[f32], freq: f32, rate: u32) -> f32 {
    let wv = std::f32::consts::TAU * freq / rate as f32;
    let c = 2.0 * wv.cos();
    let (mut s1, mut s2) = (0f32, 0f32);
    for &x in w {
        let s0 = x + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    2.0 * (s1 * s1 + s2 * s2 - c * s1 * s2).max(0.0).sqrt() / w.len() as f32
}

/// Decodes every file in `BLURT_DECODE_DIR` (e.g. one per format, made with ffmpeg) and reports each.
#[test]
#[ignore = "diagnostic; needs BLURT_DECODE_DIR"]
fn decode_every_format() {
    let Ok(dir) = std::env::var("BLURT_DECODE_DIR") else { return };
    for f in std::fs::read_dir(dir).unwrap().flatten() {
        match super::decode::decode_file(&f.path()) {
            Ok(d) => println!("OK   {:<16} {} Hz, {} ms, peak {:.2}, {} ch", f.file_name().to_string_lossy(), d.pcm.rate, d.pcm.duration_ms(), d.pcm.peak, d.source_channels),
            Err(e) => println!("FAIL {:<16} {e}", f.file_name().to_string_lossy()),
        }
    }
}

/// Startup cost of each device step (diagnostic).
#[test]
#[ignore = "diagnostic: needs real devices"]
fn time_device_steps() {
    use cpal::traits::DeviceTrait;
    let t = std::time::Instant::now();
    let lap = |what: &str, t: &mut std::time::Instant| { println!("{what:<34} {:>6} ms", t.elapsed().as_millis()); *t = std::time::Instant::now(); };
    let mut t = t;
    let out = super::devices::find_output(Some("CABLE Input (VB-Audio Virtual Cable)")).unwrap();
    lap("find_output(CABLE Input)", &mut t);
    let cfg = pick_config(&out, true, None).unwrap();
    lap("pick_config(output)", &mut t);
    let s = out.build_output_stream(&cfg.config(), |d: &mut [f32], _| d.fill(0.0), |_| {}, None).unwrap();
    s.play().unwrap();
    lap("open+play output", &mut t);
    let mic = find_input(Some("Microphone (Realtek(R) Audio)")).unwrap();
    lap("find_input(Realtek mic)", &mut t);
    let _ = pick_config(&mic, false, Some(48000)).unwrap();
    lap("pick_config(input)", &mut t);
    let _ = cpal::traits::HostTrait::default_output_device(&cpal::default_host()).unwrap();
    lap("default_output_device", &mut t);
    let _ = list();
    lap("devices::list()", &mut t);
    let _ = crate::audio::endpoints::list();
    lap("endpoints::list()", &mut t);
}

/// The fast OS endpoint list must name devices exactly as cpal (and every app) does.
#[test]
#[ignore = "needs real audio devices"]
fn fast_device_names_match_cpal() {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    for capture in [false, true] {
        let it = if capture { host.input_devices() } else { host.output_devices() };
        let mut slow: Vec<String> = it.unwrap().filter_map(|d| d.name().ok()).collect();
        let mut fast = crate::audio::endpoints::active_names(capture).expect("endpoint list");
        slow.sort();
        fast.sort();
        assert_eq!(fast, slow, "capture={capture}");
    }
}
