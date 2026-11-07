//! Microphone passthrough: your real mic is captured and mixed into the virtual-mic output, so
//! picking the virtual mic in Discord carries both your voice and your sounds.
//!
//! Input and output run on different device clocks, so the reader resamples by the nominal rate
//! ratio and corrects residual drift by keeping the ring buffer near a small target fill.

use std::sync::Arc;

use rtrb::{Consumer, Producer, RingBuffer};

use super::mixer::AtomicF32;

/// ~30 ms of buffered mic audio: low enough for conversation, high enough to absorb jitter.
const TARGET_SECS: f64 = 0.03;

pub fn ring(in_rate: u32) -> (Producer<f32>, Consumer<f32>) {
    RingBuffer::new((in_rate as usize / 2).max(1024))
}

/// Push one interleaved input block (any channel count) as mono.
pub fn push_input<T: Copy>(producer: &mut Producer<f32>, data: &[T], channels: usize, to_f32: impl Fn(T) -> f32) {
    let channels = channels.max(1);
    for frame in data.chunks_exact(channels) {
        let mono = frame.iter().map(|&s| to_f32(s)).sum::<f32>() / channels as f32;
        if producer.push(mono).is_err() {
            break; // output side stalled; dropping is better than blocking the input thread
        }
    }
}

pub struct MicReader {
    consumer: Consumer<f32>,
    step: f64,
    frac: f64,
    prev: f32,
    cur: f32,
    primed: bool,
    target: usize,
    gain: Arc<AtomicF32>,
    block_gain: f32,
}

impl MicReader {
    pub fn new(consumer: Consumer<f32>, in_rate: u32, out_rate: u32, gain: Arc<AtomicF32>) -> Self {
        Self {
            consumer,
            step: in_rate as f64 / out_rate.max(1) as f64,
            frac: 0.0,
            prev: 0.0,
            cur: 0.0,
            primed: false,
            target: ((in_rate as f64 * TARGET_SECS) as usize).max(16),
            gain,
            block_gain: 1.0,
        }
    }

    /// Called once per output callback: drift correction and gain snapshot.
    pub fn begin_block(&mut self) {
        self.block_gain = self.gain.get();
        let fill = self.consumer.slots();
        if fill > self.target * 4 {
            // Input clock runs fast (or output stalled): drop the backlog to keep latency low.
            let skip = fill - self.target;
            if let Ok(chunk) = self.consumer.read_chunk(skip) {
                chunk.commit_all();
            }
        }
    }

    pub fn next_sample(&mut self) -> f32 {
        if !self.primed {
            if self.consumer.slots() < self.target {
                return 0.0;
            }
            self.primed = true;
        }
        self.frac += self.step;
        while self.frac >= 1.0 {
            self.prev = self.cur;
            match self.consumer.pop() {
                Ok(s) => self.cur = s,
                Err(_) => {
                    // Underrun: go quiet and re-prime rather than crackle.
                    self.primed = false;
                    self.frac = 0.0;
                    return 0.0;
                }
            }
            self.frac -= 1.0;
        }
        (self.prev + (self.cur - self.prev) * self.frac as f32) * self.block_gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primes_then_resamples_and_recovers_from_underrun() {
        let (mut p, c) = ring(1000);
        let gain = Arc::new(AtomicF32::new(1.0));
        let mut r = MicReader::new(c, 1000, 2000, gain);
        assert_eq!(r.next_sample(), 0.0, "silent until primed");
        push_input(&mut p, &vec![0.5f32; 40], 1, |s| s);
        r.begin_block();
        // 2x upsample: 40 input samples -> ~80 output samples of 0.5 (after the first ramp-in).
        let out: Vec<f32> = (0..70).map(|_| r.next_sample()).collect();
        assert!(out[10..].iter().all(|s| (s - 0.5).abs() < 1e-6), "{out:?}");
        let tail: Vec<f32> = (0..40).map(|_| r.next_sample()).collect();
        assert_eq!(*tail.last().unwrap(), 0.0, "underrun goes silent");
    }

    #[test]
    fn stereo_input_is_mixed_to_mono_and_backlog_trimmed() {
        let (mut p, c) = ring(1000);
        push_input(&mut p, &[1.0f32, 0.0, 1.0, 0.0], 2, |s| s);
        assert_eq!(c.slots(), 2);
        let mut r = MicReader::new(c, 1000, 1000, Arc::new(AtomicF32::new(1.0)));
        push_input(&mut p, &vec![0.1f32; 400], 1, |s| s);
        r.begin_block();
        assert_eq!(r.consumer.slots(), r.target, "backlog dropped to target");
    }
}
