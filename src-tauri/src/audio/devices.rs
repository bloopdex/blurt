//! Device enumeration and stream opening (cpal: WASAPI on Windows, Core Audio on macOS).

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, FromSample, SampleFormat, SizedSample, Stream, StreamConfig, SupportedStreamConfig};
use serde::Serialize;

use super::endpoints::{self, Endpoint};
use super::routing::{known_loopback, paired_capture};

/// One endpoint. Its direction is which list it's in — playback (`outputs`) or recording
/// (`inputs`) — as reported by the OS audio API, never inferred from the name.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
    /// Owning adapter from OS metadata (Windows), e.g. `ROOT\VBAudioVACWDM\0000`.
    pub adapter: Option<String>,
    /// Belongs to a software (virtual) driver rather than hardware, per OS metadata.
    pub software: bool,
    /// Playback endpoints only: the recording endpoint this would loop into, if any (unverified).
    pub paired_capture: Option<String>,
    /// Name hint: a well-known loopback driver family ("VB-CABLE", "BlackHole", …).
    pub known_loopback: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceList {
    pub outputs: Vec<DeviceInfo>,
    pub inputs: Vec<DeviceInfo>,
}

pub fn list() -> DeviceList {
    let host = cpal::default_host();
    let default_out = host.default_output_device().and_then(|d| d.name().ok());
    let default_in = host.default_input_device().and_then(|d| d.name().ok());
    let (out_names, in_names) = (names(&host, false), names(&host, true));
    classify(&out_names, &in_names, &endpoints::list(), default_out.as_deref(), default_in.as_deref())
}

fn classify(
    out_names: &[String],
    in_names: &[String],
    eps: &[Endpoint],
    default_out: Option<&str>,
    default_in: Option<&str>,
) -> DeviceList {
    let info = |name: &String, capture: bool| {
        let ep = eps.iter().find(|e| e.capture == capture && &e.name == name);
        let known = known_loopback(name);
        DeviceInfo {
            name: name.clone(),
            is_default: Some(name.as_str()) == if capture { default_in } else { default_out },
            adapter: ep.map(|e| e.adapter.clone()),
            // Without metadata (macOS for now) a known loopback driver is the only software signal.
            software: ep.map_or(known.is_some(), Endpoint::software),
            paired_capture: if capture { None } else { paired_capture(name, eps, in_names) },
            known_loopback: known,
        }
    };
    DeviceList {
        outputs: out_names.iter().map(|n| info(n, false)).collect(),
        inputs: in_names.iter().map(|n| info(n, true)).collect(),
    }
}

/// Active device names in one direction: from the OS endpoint list where available (fast), else
/// cpal's per-direction enumeration (which opens every device to check its formats).
fn names(host: &cpal::Host, capture: bool) -> Vec<String> {
    endpoints::active_names(capture).unwrap_or_else(|| {
        let it = if capture { host.input_devices() } else { host.output_devices() };
        it.map(|i| i.filter_map(|d| d.name().ok()).collect()).unwrap_or_default()
    })
}

/// A device by name in one direction. Matches on the cheap unfiltered list, then checks only the
/// match for that direction (a playback and a recording endpoint can share a name).
fn find(name: &str, capture: bool) -> Result<Device, String> {
    let host = cpal::default_host();
    host.devices()
        .map_err(|e| e.to_string())?
        .filter(|d| d.name().is_ok_and(|x| x == name))
        .find(|d| if capture { d.default_input_config().is_ok() } else { d.default_output_config().is_ok() })
        .ok_or_else(|| format!("\"{name}\" is not connected."))
}

/// Cheap fingerprint of the device set and defaults, polled to notice configuration changes.
pub fn fingerprint() -> String {
    let host = cpal::default_host();
    format!(
        "{}#{}#{:?}#{:?}",
        names(&host, false).join("|"),
        names(&host, true).join("|"),
        host.default_output_device().and_then(|d| d.name().ok()),
        host.default_input_device().and_then(|d| d.name().ok()),
    )
}

pub fn find_output(name: Option<&str>) -> Result<Device, String> {
    let host = cpal::default_host();
    match name {
        None => host.default_output_device().ok_or_else(|| "No default output device is available.".into()),
        Some(n) => find(n, false),
    }
}

pub fn find_input(name: Option<&str>) -> Result<Device, String> {
    let host = cpal::default_host();
    match name {
        None => host.default_input_device().ok_or_else(|| "No microphone is available.".into()),
        Some(n) => find(n, true),
    }
}

/// The device's default format, switched to `preferred_rate` when the device supports it.
pub fn pick_config(device: &Device, output: bool, preferred_rate: Option<u32>) -> Result<SupportedStreamConfig, String> {
    let default = if output { device.default_output_config() } else { device.default_input_config() }
        .map_err(|e| format!("Can't read the device format: {e}"))?;
    let Some(rate) = preferred_rate.filter(|r| *r != default.sample_rate().0) else {
        return Ok(default);
    };
    let ranges: Vec<_> = if output {
        device.supported_output_configs().map(|i| i.collect()).unwrap_or_default()
    } else {
        device.supported_input_configs().map(|i| i.collect()).unwrap_or_default()
    };
    Ok(ranges
        .into_iter()
        .find(|r| {
            r.channels() == default.channels()
                && r.sample_format() == default.sample_format()
                && r.min_sample_rate().0 <= rate
                && rate <= r.max_sample_rate().0
        })
        .map(|r| r.with_sample_rate(cpal::SampleRate(rate)))
        .unwrap_or(default))
}

/// Open an output stream; `fill` always works in f32 and is converted to the device format.
pub fn open_output(
    device: &Device,
    config: &SupportedStreamConfig,
    fill: impl FnMut(&mut [f32], usize) + Send + 'static,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, String> {
    let cfg: StreamConfig = config.config();
    let r = match config.sample_format() {
        SampleFormat::F32 => build_out::<f32>(device, &cfg, fill, on_error),
        SampleFormat::I16 => build_out::<i16>(device, &cfg, fill, on_error),
        SampleFormat::U16 => build_out::<u16>(device, &cfg, fill, on_error),
        SampleFormat::I32 => build_out::<i32>(device, &cfg, fill, on_error),
        f => return Err(format!("Unsupported device sample format {f}.")),
    };
    r.map_err(|e| format!("Couldn't open the device: {e}"))
}

fn build_out<T: SizedSample + FromSample<f32>>(
    device: &Device,
    cfg: &StreamConfig,
    mut fill: impl FnMut(&mut [f32], usize) + Send + 'static,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, cpal::BuildStreamError> {
    let channels = cfg.channels as usize;
    let mut scratch: Vec<f32> = vec![0.0; 16384];
    device.build_output_stream(
        cfg,
        move |data: &mut [T], _| {
            if scratch.len() < data.len() {
                scratch.resize(data.len(), 0.0);
            }
            let buf = &mut scratch[..data.len()];
            fill(buf, channels);
            for (d, s) in data.iter_mut().zip(buf.iter()) {
                *d = T::from_sample(*s);
            }
        },
        on_error,
        None,
    )
}

/// Open an input stream delivering interleaved f32 blocks.
pub fn open_input(
    device: &Device,
    config: &SupportedStreamConfig,
    on_data: impl FnMut(&[f32], usize) + Send + 'static,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, String> {
    let cfg: StreamConfig = config.config();
    let r = match config.sample_format() {
        SampleFormat::F32 => build_in::<f32>(device, &cfg, on_data, on_error),
        SampleFormat::I16 => build_in::<i16>(device, &cfg, on_data, on_error),
        SampleFormat::U16 => build_in::<u16>(device, &cfg, on_data, on_error),
        SampleFormat::I32 => build_in::<i32>(device, &cfg, on_data, on_error),
        f => return Err(format!("Unsupported microphone sample format {f}.")),
    };
    r.map_err(|e| format!("Couldn't open the microphone: {e}"))
}

fn build_in<T: SizedSample>(
    device: &Device,
    cfg: &StreamConfig,
    mut on_data: impl FnMut(&[f32], usize) + Send + 'static,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, cpal::BuildStreamError>
where
    f32: FromSample<T>,
{
    let channels = cfg.channels as usize;
    let mut scratch: Vec<f32> = Vec::with_capacity(16384);
    device.build_input_stream(
        cfg,
        move |data: &[T], _| {
            scratch.clear();
            scratch.extend(data.iter().map(|s| s.to_sample::<f32>()));
            on_data(&scratch, channels);
        },
        on_error,
        None,
    )
}

impl DeviceList {
    /// Playback → recording pairs worth testing as a virtual microphone: every software output
    /// with a recording side, known cable families first. Hardware outputs never qualify.
    pub fn loopback_candidates(&self) -> Vec<(String, String)> {
        let mut c: Vec<_> = self
            .outputs
            .iter()
            .filter_map(|d| Some((d.known_loopback.is_none(), d.name.clone(), d.paired_capture.clone()?)))
            .collect();
        c.sort();
        c.into_iter().map(|(_, o, i)| (o, i)).collect()
    }

    /// The real microphone to mix in: the current choice, else the system default, else any
    /// other input — never the virtual microphone itself (that would loop).
    pub fn passthrough_source(&self, current: Option<&str>, capture: &str) -> Option<String> {
        let ok = |n: &str| n != capture && self.inputs.iter().any(|d| d.name == n);
        current
            .filter(|n| ok(n))
            .map(str::to_owned)
            .or_else(|| self.inputs.iter().find(|d| d.is_default && ok(&d.name)).map(|d| d.name.clone()))
            .or_else(|| self.inputs.iter().find(|d| ok(&d.name)).map(|d| d.name.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_setup_candidates_and_mic() {
        let ep = |n: &str, c: bool, a: &str| Endpoint { name: n.into(), capture: c, adapter: a.into() };
        let eps = vec![
            ep("Speakers (NVIDIA Broadcast)", false, r"ROOT\UNNAMED_DEVICE\0000"),
            ep("Microphone (NVIDIA Broadcast)", true, r"ROOT\UNNAMED_DEVICE\0000"),
            ep("Speakers (Steam Streaming Microphone)", false, r"ROOT\MEDIA\0001"),
            ep("Microphone (Steam Streaming Microphone)", true, r"ROOT\MEDIA\0001"),
            ep("Speakers (Realtek(R) Audio)", false, r"HDAUDIO\X\1"),
            ep("Microphone (Realtek(R) Audio)", true, r"HDAUDIO\X\1"),
        ];
        let names = |capture: bool| eps.iter().filter(|e| e.capture == capture).map(|e| e.name.clone()).collect::<Vec<_>>();
        let l = classify(&names(false), &names(true), &eps, None, Some("Microphone (Steam Streaming Microphone)"));
        let c = l.loopback_candidates();
        assert_eq!(c.len(), 2, "hardware speakers are never a candidate: {c:?}");
        assert_eq!(c[0].0, "Speakers (Steam Streaming Microphone)", "known cable first");
        let cap = "Microphone (Steam Streaming Microphone)";
        // Default input is the cable itself, so it falls through to another real mic.
        assert_eq!(l.passthrough_source(None, cap).as_deref(), Some("Microphone (NVIDIA Broadcast)"));
        assert_eq!(l.passthrough_source(Some("Microphone (Realtek(R) Audio)"), cap).as_deref(), Some("Microphone (Realtek(R) Audio)"));
        assert_eq!(l.passthrough_source(Some(cap), cap).as_deref(), Some("Microphone (NVIDIA Broadcast)"));
    }

    #[test]
    fn classifies_by_direction_and_metadata() {
        let ep = |n: &str, c: bool, a: &str| Endpoint { name: n.into(), capture: c, adapter: a.into() };
        let eps = vec![
            ep("Speakers (NVIDIA Broadcast)", false, r"ROOT\UNNAMED_DEVICE\0000"),
            ep("Microphone (NVIDIA Broadcast)", true, r"ROOT\UNNAMED_DEVICE\0000"),
            ep("Speakers (Realtek(R) Audio)", false, r"HDAUDIO\X\1"),
            ep("Microphone (Realtek(R) Audio)", true, r"HDAUDIO\X\1"),
        ];
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let outs = s(&["Speakers (NVIDIA Broadcast)", "Speakers (Realtek(R) Audio)"]);
        let ins = s(&["Microphone (NVIDIA Broadcast)", "Microphone (Realtek(R) Audio)"]);
        let l = classify(&outs, &ins, &eps, Some("Speakers (Realtek(R) Audio)"), None);

        let nv_out = &l.outputs[0];
        assert!(nv_out.software && nv_out.known_loopback.is_none(), "software, but not a known loopback");
        assert_eq!(nv_out.paired_capture.as_deref(), Some("Microphone (NVIDIA Broadcast)"), "candidate only");
        let nv_mic = &l.inputs[0];
        assert!(nv_mic.software && nv_mic.paired_capture.is_none(), "recognised as a (software) microphone");
        let rt = &l.outputs[1];
        assert!(!rt.software && rt.paired_capture.is_none() && rt.is_default);
        assert!(l.outputs.iter().all(|d| !l.inputs.contains(d)), "directions never mix");
    }
}
