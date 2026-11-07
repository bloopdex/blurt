//! Instant clips: keep the last N seconds of what you hear (the default playback device, captured
//! by WASAPI loopback) so a funny moment can be saved as a sound. Nothing is written to disk until
//! you clip. Only runs while the setting is on.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::error::{AppError, AppResult};

#[derive(Default)]
struct Ring {
    /// Interleaved stereo.
    samples: VecDeque<f32>,
    rate: u32,
    cap: usize,
}

#[derive(Default)]
pub struct ClipRecorder {
    stop: Mutex<Option<Arc<AtomicBool>>>,
    ring: Arc<Mutex<Ring>>,
}

impl ClipRecorder {
    /// Start, restart (new length) or stop recording.
    // shortcut: follows the default playback device at start only; restart via the setting if it changes.
    pub fn configure(&self, on: bool, seconds: u32) {
        let mut stop = self.stop.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = stop.take() {
            s.store(true, Ordering::Relaxed);
        }
        if !on {
            self.ring.lock().unwrap_or_else(|e| e.into_inner()).samples = VecDeque::new();
            return;
        }
        let flag = Arc::new(AtomicBool::new(false));
        *stop = Some(flag.clone());
        let ring = self.ring.clone();
        std::thread::spawn(move || {
            if let Err(e) = record(&ring, seconds, &flag) {
                log::warn!("clip recording unavailable: {e}");
            }
        });
    }

    /// Write what's buffered as a 16-bit WAV.
    pub fn save(&self, path: &Path) -> AppResult<()> {
        let (samples, rate) = {
            let r = self.ring.lock().unwrap_or_else(|e| e.into_inner());
            (r.samples.iter().copied().collect::<Vec<f32>>(), r.rate)
        };
        if rate == 0 || samples.len() < rate as usize {
            return Err(AppError::Invalid("Nothing recorded yet. Turn on instant clips in Settings → Stream & game.".into()));
        }
        write_wav(path, rate, &samples)?;
        Ok(())
    }
}

fn record(ring: &Arc<Mutex<Ring>>, seconds: u32, stop: &AtomicBool) -> Result<(), String> {
    let device = cpal::default_host().default_output_device().ok_or("no playback device")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    if config.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!("unsupported sample format {:?}", config.sample_format()));
    }
    let channels = config.channels() as usize;
    let rate = config.sample_rate().0;
    {
        let mut r = ring.lock().unwrap_or_else(|e| e.into_inner());
        *r = Ring { samples: VecDeque::with_capacity(seconds as usize * rate as usize * 2), rate, cap: seconds as usize * rate as usize * 2 };
    }
    let sink = ring.clone();
    // On WASAPI, an input stream on a playback device is a loopback capture.
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let Ok(mut r) = sink.try_lock() else { return }; // only contended while saving; dropping a block is fine
                for f in data.chunks_exact(channels) {
                    r.samples.push_back(f[0]);
                    r.samples.push_back(*f.get(1).unwrap_or(&f[0]));
                }
                let excess = r.samples.len().saturating_sub(r.cap);
                r.samples.drain(..excess);
            },
            |e| log::warn!("clip stream error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    log::info!("clip recording {seconds}s at {rate} Hz");
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Ok(())
}

pub fn write_wav(path: &Path, rate: u32, stereo: &[f32]) -> std::io::Result<()> {
    let data_len = (stereo.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    for v in [16u32.to_le_bytes(), [1, 0, 2, 0], rate.to_le_bytes(), (rate * 4).to_le_bytes(), [4, 0, 16, 0]] {
        out.extend_from_slice(&v);
    }
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in stereo {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_clip_decodes_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.wav");
        let samples: Vec<f32> = (0..48_000).flat_map(|i| [(i as f32 * 0.01).sin() * 0.5; 2]).collect();
        write_wav(&path, 48_000, &samples).unwrap();
        let d = crate::audio::decode::decode_file(&path).unwrap();
        assert_eq!((d.pcm.rate, d.source_channels, d.pcm.duration_ms()), (48_000, 2, 1000));
    }

    #[test]
    fn save_refuses_an_empty_buffer() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ClipRecorder::default().save(&dir.path().join("x.wav")).is_err());
    }
}
