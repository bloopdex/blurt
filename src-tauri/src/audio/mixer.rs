//! Real-time mixing. A `SinkRenderer` lives inside one device callback: it receives new voices
//! over a channel, mixes them, adds mic passthrough, and soft-limits. No locks, no allocation in
//! the steady state; control happens through atomics on `VoiceShared`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;

use super::decode::Pcm;
use super::passthrough::MicReader;

#[derive(Default)]
pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub const fn new(v: f32) -> Self {
        Self(AtomicU32::new(v.to_bits()))
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
    pub fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed)
    }
}

#[derive(Default)]
pub struct VoiceShared {
    pub frame: AtomicUsize,
    pub paused: AtomicBool,
    pub stopping: AtomicBool,
    pub done: AtomicBool,
}

pub struct Voice {
    pub pcm: Arc<Pcm>,
    pub start: usize,
    pub end: usize,
    pub gain: f32,
    pub fade_in: usize,
    pub fade_out: usize,
    pub shared: Arc<VoiceShared>,
    pos: usize,
    release_left: Option<usize>,
}

impl Voice {
    pub fn new(pcm: Arc<Pcm>, start: usize, end: usize, gain: f32, fade_in: usize, fade_out: usize) -> Self {
        let end = end.min(pcm.frames());
        let start = start.min(end);
        let shared = Arc::new(VoiceShared { frame: AtomicUsize::new(start), ..Default::default() });
        Self { pcm, start, end, gain, fade_in, fade_out, shared, pos: start, release_left: None }
    }

    /// Mix into `mix` (interleaved stereo). Returns false once finished.
    fn render(&mut self, mix: &mut [f32]) -> bool {
        if self.shared.paused.load(Ordering::Relaxed) && !self.shared.stopping.load(Ordering::Relaxed) {
            return true;
        }
        // Even an "instant" stop gets a 5 ms ramp so it doesn't click.
        let min_ramp = (self.pcm.rate as usize / 200).max(1);
        if self.release_left.is_none() && self.shared.stopping.load(Ordering::Relaxed) {
            self.release_left = Some(self.fade_out.max(min_ramp));
        }
        let release_total = self.fade_out.max(min_ramp) as f32;
        let tail = self.fade_out.max(min_ramp);
        let s = &self.pcm.samples;
        for frame in mix.chunks_exact_mut(2) {
            if self.pos >= self.end {
                return false;
            }
            let mut env = 1.0f32;
            let played = self.pos - self.start;
            if self.fade_in > 0 && played < self.fade_in {
                env *= played as f32 / self.fade_in as f32;
            }
            let remaining = self.end - self.pos;
            if remaining < tail {
                env *= remaining as f32 / tail as f32;
            }
            if let Some(left) = self.release_left.as_mut() {
                if *left == 0 {
                    return false;
                }
                env *= *left as f32 / release_total;
                *left -= 1;
            }
            let g = self.gain * env;
            frame[0] += s[self.pos * 2] * g;
            frame[1] += s[self.pos * 2 + 1] * g;
            self.pos += 1;
        }
        self.shared.frame.store(self.pos, Ordering::Relaxed);
        true
    }
}

/// Passthrough-mic level while a sound plays (1.0 = no ducking). Set from settings.
pub static MIC_DUCK: AtomicF32 = AtomicF32::new(1.0);
/// Per-sample smoothing toward the duck target (~20 ms at 48 kHz), so the mic never clicks.
const DUCK_SMOOTHING: f32 = 0.001;

pub struct SinkRenderer {
    voices: Vec<Voice>,
    rx: Receiver<Voice>,
    volume: Arc<AtomicF32>,
    mic: Option<MicReader>,
    post: Option<Arc<AtomicF32>>,
    mix: Vec<f32>,
    duck: f32,
}

impl SinkRenderer {
    /// `post` scales the final mix (sounds + mic) — the virtual mic's combined output level.
    pub fn new(rx: Receiver<Voice>, volume: Arc<AtomicF32>, mic: Option<MicReader>, post: Option<Arc<AtomicF32>>) -> Self {
        Self { voices: Vec::with_capacity(64), rx, volume, mic, post, mix: vec![0.0; 8192], duck: 1.0 }
    }

    /// Fill a device buffer with `channels` interleaved channels.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        while let Ok(v) = self.rx.try_recv() {
            self.voices.push(v);
        }
        let frames = out.len() / channels.max(1);
        if self.mix.len() < frames * 2 {
            self.mix.resize(frames * 2, 0.0); // only if the device suddenly uses a larger buffer
        }
        let mix = &mut self.mix[..frames * 2];
        mix.fill(0.0);

        self.voices.retain_mut(|v| {
            let alive = v.render(mix);
            if !alive {
                v.shared.done.store(true, Ordering::Relaxed);
            }
            alive
        });

        let vol = self.volume.get();
        if vol != 1.0 {
            mix.iter_mut().for_each(|s| *s *= vol);
        }
        if let Some(mic) = self.mic.as_mut() {
            mic.begin_block();
            let target = if self.voices.is_empty() { 1.0 } else { MIC_DUCK.get() };
            for frame in mix.chunks_exact_mut(2) {
                self.duck += (target - self.duck) * DUCK_SMOOTHING;
                let m = mic.next_sample() * self.duck;
                frame[0] += m;
                frame[1] += m;
            }
        }
        if let Some(g) = self.post.as_ref().map(|p| p.get()).filter(|g| *g != 1.0) {
            mix.iter_mut().for_each(|s| *s *= g);
        }
        for (frame, dst) in mix.chunks_exact(2).zip(out.chunks_exact_mut(channels)) {
            let (l, r) = (soft_clip(frame[0]), soft_clip(frame[1]));
            match dst.len() {
                1 => dst[0] = (l + r) * 0.5,
                _ => {
                    dst[0] = l;
                    dst[1] = r;
                    dst[2..].fill(0.0);
                }
            }
        }
    }
}

/// Transparent below -1 dBFS, then a smooth knee that never exceeds full scale.
pub fn soft_clip(x: f32) -> f32 {
    const T: f32 = 0.89;
    let a = x.abs();
    if a <= T {
        x
    } else {
        x.signum() * (T + (1.0 - T) * ((a - T) / (1.0 - T)).tanh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn pcm(frames: usize, value: f32) -> Arc<Pcm> {
        Arc::new(Pcm { samples: vec![value; frames * 2], rate: 1000, peak: value })
    }

    #[test]
    fn mic_is_ducked_while_a_sound_plays() {
        use crate::audio::passthrough::{MicReader, push_input, ring};
        MIC_DUCK.set(0.25);
        // Realistic rate: the 30 ms jitter target must exceed one 200-frame block.
        let (mut p, c) = ring(10_000);
        let mic = MicReader::new(c, 10_000, 10_000, Arc::new(AtomicF32::new(1.0)));
        let (tx, rx) = channel();
        let mut r = SinkRenderer::new(rx, Arc::new(AtomicF32::new(1.0)), Some(mic), None);
        tx.send(Voice::new(pcm(20_000, 0.0), 0, 20_000, 1.0, 0, 0)).unwrap(); // silent sound: output is the mic alone
        let mut out = vec![0.0f32; 400];
        for _ in 0..40 {
            push_input(&mut p, &[0.4f32; 300], 1, |s| s); // a little faster than consumed, like a real mic with drift
            r.render(&mut out, 2);
        }
        MIC_DUCK.set(1.0);
        assert!((out[398] - 0.1).abs() < 0.01, "mic at a quarter while playing, got {}", out[398]);
    }

    #[test]
    fn mixes_overlapping_voices_and_finishes() {
        let (tx, rx) = channel();
        let mut r = SinkRenderer::new(rx, Arc::new(AtomicF32::new(1.0)), None, None);
        let a = Voice::new(pcm(100, 0.25), 0, 100, 1.0, 0, 0);
        let b = Voice::new(pcm(100, 0.25), 0, 100, 1.0, 0, 0);
        let (sa, sb) = (a.shared.clone(), b.shared.clone());
        tx.send(a).unwrap();
        tx.send(b).unwrap();
        let mut out = vec![0f32; 20 * 2];
        r.render(&mut out, 2);
        assert!((out[0] - 0.5).abs() < 1e-6, "two voices sum: {}", out[0]);
        let mut big = vec![0f32; 200 * 2];
        r.render(&mut big, 2);
        assert!(sa.done.load(Ordering::Relaxed) && sb.done.load(Ordering::Relaxed));
        assert_eq!(big[399], 0.0, "silence after the end");
    }

    #[test]
    fn stop_ramps_down_without_click_then_ends() {
        let (tx, rx) = channel();
        let mut r = SinkRenderer::new(rx, Arc::new(AtomicF32::new(1.0)), None, None);
        let v = Voice::new(pcm(10_000, 0.5), 0, 10_000, 1.0, 0, 0);
        let shared = v.shared.clone();
        tx.send(v).unwrap();
        let mut out = vec![0f32; 10];
        r.render(&mut out, 1);
        shared.stopping.store(true, Ordering::Relaxed);
        let mut out = vec![0f32; 20];
        r.render(&mut out, 1);
        assert!(out[0] > 0.4 && out[0] <= 0.5, "ramp starts near full level");
        assert!(out.windows(2).all(|w| w[1] <= w[0] + 1e-6), "monotonic ramp: {out:?}");
        assert!(shared.done.load(Ordering::Relaxed), "5 ms ramp at 1 kHz = 5 frames");
    }

    #[test]
    fn trim_window_and_gain_are_respected() {
        let (tx, rx) = channel();
        let mut r = SinkRenderer::new(rx, Arc::new(AtomicF32::new(0.5)), None, None);
        let samples: Vec<f32> = (0..100).flat_map(|i| [i as f32 / 1000.0; 2]).collect();
        let p = Arc::new(Pcm { samples, rate: 1000, peak: 0.1 });
        let v = Voice::new(p, 50, 60, 2.0, 0, 0);
        let shared = v.shared.clone();
        tx.send(v).unwrap();
        let mut out = vec![0f32; 4];
        r.render(&mut out, 2);
        assert!((out[0] - 0.05).abs() < 1e-6, "starts at frame 50 with gain 2 × master 0.5");
        assert_eq!(shared.frame.load(Ordering::Relaxed), 52);
    }

    #[test]
    fn paused_voice_holds_position() {
        let (tx, rx) = channel();
        let mut r = SinkRenderer::new(rx, Arc::new(AtomicF32::new(1.0)), None, None);
        let v = Voice::new(pcm(100, 0.5), 0, 100, 1.0, 0, 0);
        let shared = v.shared.clone();
        shared.paused.store(true, Ordering::Relaxed);
        tx.send(v).unwrap();
        let mut out = vec![0f32; 20];
        r.render(&mut out, 2);
        assert_eq!(out, vec![0.0; 20]);
        assert_eq!(shared.frame.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn combined_output_gain_scales_the_whole_mix() {
        let (tx, rx) = channel();
        let mut r = SinkRenderer::new(rx, Arc::new(AtomicF32::new(1.0)), None, Some(Arc::new(AtomicF32::new(0.5))));
        tx.send(Voice::new(pcm(100, 0.4), 0, 100, 1.0, 0, 0)).unwrap();
        let mut out = vec![0f32; 4];
        r.render(&mut out, 2);
        assert!((out[0] - 0.2).abs() < 1e-6, "{}", out[0]);
    }

    #[test]
    fn soft_clip_bounds_output() {
        assert_eq!(soft_clip(0.5), 0.5);
        for x in [0.95f32, 1.5, 4.0, -9.0] {
            assert!(soft_clip(x).abs() <= 1.0);
        }
        assert!(soft_clip(1.5) > soft_clip(0.95), "monotonic");
    }

    #[test]
    fn mono_device_gets_downmix() {
        let (tx, rx) = channel();
        let mut r = SinkRenderer::new(rx, Arc::new(AtomicF32::new(1.0)), None, None);
        let samples: Vec<f32> = (0..100).flat_map(|_| [0.4, 0.0]).collect();
        let p = Arc::new(Pcm { samples, rate: 1000, peak: 0.4 });
        tx.send(Voice::new(p, 0, 100, 1.0, 0, 0)).unwrap();
        let mut out = vec![0f32; 1];
        r.render(&mut out, 1);
        assert!((out[0] - 0.2).abs() < 1e-6);
    }
}
