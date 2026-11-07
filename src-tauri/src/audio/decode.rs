//! File decoding (Symphonia), waveform peaks and sample-rate conversion.
//! Everything is normalised to interleaved stereo f32 so the mixer has one format to handle.

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::error::{AppError, AppResult};

/// shortcut: whole clips are decoded into memory; fine for soundboard clips, switch to
/// streaming decode if long-form audio (podcasts, albums) becomes a use case.
pub const MAX_DURATION_SECS: u64 = 15 * 60;

pub const SUPPORTED_EXTENSIONS: &[&str] =
    &["mp3", "wav", "wave", "ogg", "oga", "flac", "m4a", "mp4", "aac", "alac", "caf", "webm", "mka", "mkv", "aiff", "aif"];

#[derive(Debug)]
pub struct Pcm {
    /// Interleaved stereo.
    pub samples: Vec<f32>,
    pub rate: u32,
    /// Absolute peak, used for normalisation.
    pub peak: f32,
}

impl Pcm {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }
    pub fn duration_ms(&self) -> i64 {
        (self.frames() as u64 * 1000 / self.rate.max(1) as u64) as i64
    }
}

#[derive(Debug)]
pub struct Decoded {
    pub pcm: Pcm,
    pub source_channels: u16,
}

pub fn decode_file(path: &Path) -> AppResult<Decoded> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(&ext);

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| unsupported(&ext, &e))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| AppError::Unsupported("This file has no audio track.".into()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| unsupported(&ext, &e))?;

    let mut samples: Vec<f32> = Vec::new();
    let mut rate = track.codec_params.sample_rate.unwrap_or(0);
    let mut channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(0);
    let mut buf: Option<SampleBuffer<f32>> = None;
    let mut bad_packets = 0usize;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(AppError::Audio(format!("Couldn't read this file: {e}"))),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // A few corrupt frames shouldn't reject a whole file, but a file of nothing but errors should.
            Err(SymError::DecodeError(_)) => {
                bad_packets += 1;
                if bad_packets > 100 && samples.is_empty() {
                    return Err(AppError::Audio("This file looks corrupted and couldn't be decoded.".into()));
                }
                continue;
            }
            Err(e) => return Err(AppError::Audio(format!("Couldn't decode this file: {e}"))),
        };
        let spec = *decoded.spec();
        rate = spec.rate;
        channels = spec.channels.count() as u16;
        let sb = buf.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
        if sb.capacity() < decoded.capacity() * channels as usize {
            *sb = SampleBuffer::new(decoded.capacity() as u64, spec);
        }
        sb.copy_interleaved_ref(decoded);
        push_stereo(&mut samples, sb.samples(), channels as usize);
        if rate > 0 && (samples.len() / 2) as u64 > MAX_DURATION_SECS * rate as u64 {
            return Err(AppError::Unsupported("Clips longer than 15 minutes aren't supported.".into()));
        }
    }

    if samples.is_empty() || rate == 0 {
        return Err(AppError::Audio("This file contains no playable audio.".into()));
    }
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    Ok(Decoded { pcm: Pcm { samples, rate, peak }, source_channels: channels })
}

fn unsupported(ext: &str, e: &SymError) -> AppError {
    log::warn!("unsupported media ({ext}): {e}");
    let hint = match ext {
        "webm" | "opus" => " WebM/Opus audio isn't supported yet — convert it to MP3, OGG or FLAC.",
        "wma" => " WMA isn't supported — convert it to MP3 or FLAC.",
        "m4a" | "mp4" | "aac" => " Only AAC-LC and ALAC are supported in M4A files.",
        _ => " Supported formats: MP3, WAV, OGG, FLAC, M4A/AAC, AIFF.",
    };
    AppError::Unsupported(format!("Blurt can't read this file.{hint}"))
}

/// Downmix/upmix arbitrary channel layouts to stereo.
fn push_stereo(out: &mut Vec<f32>, interleaved: &[f32], channels: usize) {
    match channels {
        0 => {}
        1 => interleaved.iter().for_each(|&s| out.extend_from_slice(&[s, s])),
        2 => out.extend_from_slice(interleaved),
        // shortcut: surround keeps front L/R only; mix centre/rear in if 5.1 sources become common.
        n => interleaved.chunks_exact(n).for_each(|f| out.extend_from_slice(&f[..2])),
    }
}

/// Linear-interpolation resampler for interleaved stereo.
/// shortcut: linear interpolation is fine for 44.1k↔48k voice-chat output; swap in a
/// windowed-sinc resampler if users route to high-fidelity recording.
pub fn resample_stereo(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.len() < 4 {
        return input.to_vec();
    }
    let in_frames = input.len() / 2;
    let out_frames = (in_frames as u64 * to as u64 / from as u64) as usize;
    let step = from as f64 / to as f64;
    let mut out = Vec::with_capacity(out_frames * 2);
    for i in 0..out_frames {
        let pos = i as f64 * step;
        let idx = (pos as usize).min(in_frames - 1);
        let next = (idx + 1).min(in_frames - 1);
        let frac = (pos - idx as f64) as f32;
        for c in 0..2 {
            let a = input[idx * 2 + c];
            let b = input[next * 2 + c];
            out.push(a + (b - a) * frac);
        }
    }
    out
}

/// `bins` peak values scaled to 0–255 for compact storage and drawing.
pub fn waveform(pcm: &Pcm, bins: usize) -> Vec<u8> {
    let frames = pcm.frames();
    if frames == 0 || bins == 0 {
        return vec![0; bins];
    }
    let peak = pcm.peak.max(1e-6);
    (0..bins)
        .map(|b| {
            let start = b * frames / bins;
            let end = ((b + 1) * frames / bins).max(start + 1).min(frames);
            let m = pcm.samples[start * 2..end * 2].iter().fold(0f32, |m, s| m.max(s.abs()));
            ((m / peak).sqrt() * 255.0).round() as u8
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// Minimal 16-bit PCM WAV writer for fixtures.
    pub fn write_wav(path: &Path, rate: u32, channels: u16, samples: &[i16]) {
        let data_len = (samples.len() * 2) as u32;
        let mut f = File::create(path).unwrap();
        let block = channels * 2;
        f.write_all(b"RIFF").unwrap();
        f.write_all(&(36 + data_len).to_le_bytes()).unwrap();
        f.write_all(b"WAVEfmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&channels.to_le_bytes()).unwrap();
        f.write_all(&rate.to_le_bytes()).unwrap();
        f.write_all(&(rate * block as u32).to_le_bytes()).unwrap();
        f.write_all(&block.to_le_bytes()).unwrap();
        f.write_all(&16u16.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data_len.to_le_bytes()).unwrap();
        for s in samples {
            f.write_all(&s.to_le_bytes()).unwrap();
        }
    }

    pub fn sine(rate: u32, secs: f32, amp: f32) -> Vec<i16> {
        (0..(rate as f32 * secs) as usize)
            .map(|i| ((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * amp * 32767.0) as i16)
            .collect()
    }

    #[test]
    fn decodes_mono_wav_to_stereo() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tone.wav");
        write_wav(&p, 22050, 1, &sine(22050, 0.5, 0.5));
        let d = decode_file(&p).unwrap();
        assert_eq!(d.source_channels, 1);
        assert_eq!(d.pcm.rate, 22050);
        assert_eq!(d.pcm.frames(), 11025);
        assert_eq!(d.pcm.duration_ms(), 500);
        assert!((d.pcm.peak - 0.5).abs() < 0.01);
        assert_eq!(d.pcm.samples[200], d.pcm.samples[201], "mono is duplicated to both channels");
    }

    #[test]
    fn rejects_garbage_with_actionable_message() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("fake.mp3");
        std::fs::write(&p, b"definitely not audio").unwrap();
        let err = decode_file(&p).unwrap_err();
        assert!(matches!(err, AppError::Unsupported(_) | AppError::Audio(_)), "{err}");
    }

    #[test]
    fn resample_changes_length_and_keeps_level() {
        let input: Vec<f32> = (0..44100).flat_map(|_| [0.25f32, -0.25]).collect();
        let out = resample_stereo(&input, 44100, 48000);
        assert_eq!(out.len() / 2, 48000);
        assert!(out.chunks(2).all(|f| (f[0] - 0.25).abs() < 1e-6 && (f[1] + 0.25).abs() < 1e-6));
        assert_eq!(resample_stereo(&input, 48000, 48000).len(), input.len());
    }

    #[test]
    fn waveform_has_requested_bins() {
        let pcm = Pcm { samples: vec![0.0, 0.0, 1.0, 1.0, 0.5, 0.5, 0.0, 0.0], rate: 4, peak: 1.0 };
        let w = waveform(&pcm, 4);
        assert_eq!(w, vec![0, 255, 180, 0]);
    }
}
