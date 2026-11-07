//! Audio endpoint metadata from the OS device tree, so classification never rests on names.
//!
//! Windows: every endpoint is a `SWD\MMDEVAPI\{0.0.F.00000000}.{guid}` device node, where F is the
//! data flow (0 = render/playback, 1 = capture/recording). Its `DEVPKEY_Device_Parent` is the
//! adapter that owns it: `HDAUDIO\…`/`USB\…`/`BTHENUM\…` for hardware, `ROOT\…` for
//! software-only drivers (VB-CABLE, NVIDIA Broadcast, Steam, …). Render and capture endpoints on
//! the same software adapter *may* be a loopback pair; only a signal test proves it.

#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub name: String,
    pub capture: bool,
    /// Instance id of the owning adapter, e.g. `ROOT\VBAudioVACWDM\0000`.
    pub adapter: String,
}

impl Endpoint {
    /// Root-enumerated adapters have no hardware bus: they are software (virtual) audio drivers.
    pub fn software(&self) -> bool {
        self.adapter.to_ascii_uppercase().starts_with("ROOT\\")
    }
}

/// Data flow encoded in an MMDevice endpoint instance id.
pub fn flow_is_capture(instance_id: &str) -> Option<bool> {
    let upper = instance_id.to_ascii_uppercase();
    let rest = upper.strip_prefix("SWD\\MMDEVAPI\\{0.0.")?;
    match rest.as_bytes().first() {
        Some(b'0') => Some(false),
        Some(b'1') => Some(true),
        _ => None,
    }
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn string_prop(devinst: u32, key: &windows::Win32::Foundation::DEVPROPKEY) -> Option<String> {
    use windows::Win32::Devices::DeviceAndDriverInstallation::*;
    use windows::Win32::Devices::Properties::*;
    let mut ty = DEVPROPTYPE(0);
    let mut buf = [0u16; 512];
    let mut size = std::mem::size_of_val(&buf) as u32;
    // SAFETY: buffer and size describe the same stack allocation.
    let cr = unsafe { CM_Get_DevNode_PropertyW(devinst, key, &mut ty, Some(buf.as_mut_ptr().cast()), &mut size, 0) };
    if cr != CR_SUCCESS || ty != DEVPROP_TYPE_STRING {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

/// The name apps see for an endpoint (MMDevice property store), e.g. "CABLE Input (VB-Audio
/// Virtual Cable)". The device node's own FriendlyName can differ ("Speakers (VB-Audio…)"), so
/// names always come from here.
///
/// SAFETY: caller has initialised COM on this thread.
#[cfg(windows)]
unsafe fn endpoint_name(dev: &windows::Win32::Media::Audio::IMMDevice) -> windows::core::Result<String> {
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::System::Com::STGM_READ;
    let store = unsafe { dev.OpenPropertyStore(STGM_READ)? };
    Ok(unsafe { store.GetValue(&PKEY_Device_FriendlyName)? }.to_string())
}

/// Owning adapter of an endpoint device node, e.g. `ROOT\VBAudioVACWDM\0000`.
#[cfg(windows)]
fn parent(instance_id: &str) -> Option<String> {
    use windows::Win32::Devices::DeviceAndDriverInstallation::*;
    use windows::Win32::Devices::Properties::DEVPKEY_Device_Parent;
    use windows::core::PCWSTR;
    let mut devinst = 0u32;
    let wid = wide(instance_id);
    // SAFETY: NUL-terminated id, out-parameter devinst.
    if unsafe { CM_Locate_DevNodeW(&mut devinst, PCWSTR(wid.as_ptr()), CM_LOCATE_DEVNODE_NORMAL) } != CR_SUCCESS {
        return None;
    }
    string_prop(devinst, &DEVPKEY_Device_Parent)
}

/// Runs COM work on a fresh thread so the apartment is known, whatever the caller's thread did.
#[cfg(windows)]
fn on_com_thread<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    std::thread::spawn(move || {
        // SAFETY: initialising COM for this new thread only.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        f()
    })
    .join()
    .map_err(|_| "Audio device query failed.".to_string())?
}

/// Raises a recording endpoint's Windows input level (Sound settings → Input → Volume) by
/// `by_db`, capped at the device's maximum. Some virtual cables (Steam Streaming Microphone)
/// lose ~26 dB internally; their input level goes up to +30 dB and makes it back.
/// Returns (before, after) in dB.
#[cfg(windows)]
pub fn raise_capture_level(name: &str, by_db: f32) -> Result<(f32, f32), String> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;
    let name = name.to_owned();
    on_com_thread(move || {
        let err = |e: windows::core::Error| format!("Couldn't change the input level: {e}");
        // SAFETY: COM is initialised on this thread; all out-pointers are locals.
        unsafe {
            let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(err)?;
            let all = en.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE).map_err(err)?;
            for i in 0..all.GetCount().map_err(err)? {
                let dev = all.Item(i).map_err(err)?;
                if endpoint_name(&dev).ok().as_deref() != Some(name.as_str()) {
                    continue;
                }
                let vol: IAudioEndpointVolume = dev.Activate(CLSCTX_ALL, None).map_err(err)?;
                let (mut min, mut max, mut step) = (0f32, 0f32, 0f32);
                vol.GetVolumeRange(&mut min, &mut max, &mut step).map_err(err)?;
                let before = vol.GetMasterVolumeLevel().map_err(err)?;
                let after = (before + by_db).clamp(min, max);
                vol.SetMasterVolumeLevel(after, std::ptr::null()).map_err(err)?;
                vol.SetMute(false, std::ptr::null()).map_err(err)?;
                return Ok((before, vol.GetMasterVolumeLevel().map_err(err)?));
            }
            Err(format!("“{name}” isn't an active recording device."))
        }
    })
}

/// Names of the active playback (or recording) endpoints, in Windows' order. Milliseconds,
/// where cpal's per-direction enumeration opens every device to query formats (~2 s here).
pub fn active_names(capture: bool) -> Option<Vec<String>> {
    let all = list();
    (!all.is_empty()).then(|| all.into_iter().filter(|e| e.capture == capture).map(|e| e.name).collect())
}

#[cfg(not(windows))]
pub fn raise_capture_level(_name: &str, _by_db: f32) -> Result<(f32, f32), String> {
    Err("Raise the virtual microphone's input level in Audio MIDI Setup.".into())
}

/// Active endpoints with direction (from the endpoint id) and owning adapter (device tree).
#[cfg(windows)]
pub fn list() -> Vec<Endpoint> {
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;
    on_com_thread(|| {
        let err = |e: windows::core::Error| e.to_string();
        let mut out = Vec::new();
        // SAFETY: COM is initialised on this thread; all out-pointers are locals.
        unsafe {
            let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(err)?;
            let all = en.EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE).map_err(err)?;
            for i in 0..all.GetCount().map_err(err)? {
                let dev = all.Item(i).map_err(err)?;
                let instance = format!(r"SWD\MMDEVAPI\{}", dev.GetId().map_err(err)?.to_string().unwrap_or_default());
                let (Some(capture), Ok(name), Some(adapter)) = (flow_is_capture(&instance), endpoint_name(&dev), parent(&instance)) else {
                    continue;
                };
                out.push(Endpoint { name, capture, adapter });
            }
        }
        Ok(out)
    })
    .unwrap_or_default()
}

/// shortcut: macOS has no adapter metadata here yet; Core Audio's kAudioDevicePropertyTransportType
/// (== kAudioDeviceTransportTypeVirtual) is the equivalent to add when macOS is brought up.
#[cfg(not(windows))]
pub fn list() -> Vec<Endpoint> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_flow_from_endpoint_id() {
        assert_eq!(flow_is_capture("SWD\\MMDEVAPI\\{0.0.0.00000000}.{89DAD816-B161-414A-A858-19A98A31E58F}"), Some(false));
        assert_eq!(flow_is_capture("SWD\\MMDEVAPI\\{0.0.1.00000000}.{2F4F063B-84BB-4915-8F23-D6883F834C4E}"), Some(true));
        assert_eq!(flow_is_capture("USB\\VID_054C"), None);
    }

    #[test]
    fn software_adapter_is_root_enumerated() {
        let e = |a: &str| Endpoint { name: "x".into(), capture: false, adapter: a.into() };
        assert!(e("ROOT\\UNNAMED_DEVICE\\0000").software());
        assert!(!e("HDAUDIO\\FUNC_01&VEN_10EC\\4&1&0001").software());
        assert!(!e("USB\\VID_054C&PID_09CC\\7&2").software());
    }

    /// Runs against the real device tree (Windows only; harmless elsewhere).
    #[test]
    fn real_endpoints_have_adapters() {
        for ep in list() {
            assert!(!ep.adapter.is_empty(), "{ep:?}");
        }
    }
}
