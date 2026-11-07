//! Auto push-to-talk: hold the user's voice-chat key while a sound plays on the virtual mic, like
//! Soundpad's "auto keys". Without it, push-to-talk chats (Discord, Valorant, CS2) send nothing.
//! Keys go out as scan codes so games reading raw input see them; some anti-cheat games ignore
//! simulated input entirely.

use std::sync::Mutex;

#[derive(Debug, PartialEq)]
enum Input {
    /// Virtual-key code.
    Key(u16),
    /// Mouse button: (down flag, up flag, x-button number).
    Mouse(u32, u32, u32),
}

const MIDDLE: (u32, u32) = (0x0020, 0x0040);
const XBUTTON: (u32, u32) = (0x0080, 0x0100);

fn parse(name: &str) -> Option<Input> {
    let fixed = match name {
        "Mouse3" => Some(Input::Mouse(MIDDLE.0, MIDDLE.1, 0)),
        "Mouse4" => Some(Input::Mouse(XBUTTON.0, XBUTTON.1, 1)),
        "Mouse5" => Some(Input::Mouse(XBUTTON.0, XBUTTON.1, 2)),
        "CapsLock" => Some(Input::Key(0x14)),
        "Backquote" => Some(Input::Key(0xC0)),
        "Space" => Some(Input::Key(0x20)),
        "Tab" => Some(Input::Key(0x09)),
        "ShiftLeft" => Some(Input::Key(0xA0)),
        "ControlLeft" => Some(Input::Key(0xA2)),
        "AltLeft" => Some(Input::Key(0xA4)),
        _ => None,
    };
    if fixed.is_some() {
        return fixed;
    }
    let one = |rest: &str| (rest.len() == 1).then(|| rest.as_bytes()[0]);
    if let Some(c) = name.strip_prefix("Key").and_then(one).filter(u8::is_ascii_uppercase) {
        return Some(Input::Key(c as u16));
    }
    if let Some(c) = name.strip_prefix("Digit").and_then(one).filter(u8::is_ascii_digit) {
        return Some(Input::Key(c as u16));
    }
    let f: u16 = name.strip_prefix('F')?.parse().ok()?;
    (1..=24).contains(&f).then(|| Input::Key(0x6F + f))
}

pub fn is_supported(name: &str) -> bool {
    parse(name).is_some()
}

static HELD: Mutex<Option<String>> = Mutex::new(None);

/// Make `key` be held exactly while `want` is true. Called on every voice update; cheap when nothing changes.
pub fn set_held(key: Option<&str>, want: bool) {
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    let target = key.filter(|_| want);
    if held.as_deref() == target {
        return;
    }
    if let Some(k) = held.take() {
        send(&k, false);
    }
    if let Some(k) = target {
        send(k, true);
        *held = Some(k.to_owned());
    }
}

#[cfg(windows)]
fn send(name: &str, down: bool) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
        MAPVK_VK_TO_VSC, MOUSE_EVENT_FLAGS, MOUSEINPUT, MapVirtualKeyW, SendInput,
    };
    let input = match parse(name) {
        Some(Input::Key(vk)) => {
            let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
            let flags = if down { KEYEVENTF_SCANCODE } else { KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP };
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wScan: scan, dwFlags: KEYBD_EVENT_FLAGS(flags.0), ..Default::default() } },
            }
        }
        Some(Input::Mouse(d, u, x)) => INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT { mouseData: x, dwFlags: MOUSE_EVENT_FLAGS(if down { d } else { u }), ..Default::default() },
            },
        },
        None => return,
    };
    if unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) } != 1 {
        log::warn!("push-to-talk: SendInput was blocked for {name}");
    }
}

#[cfg(not(windows))]
fn send(_name: &str, _down: bool) {
    // shortcut: macOS needs Accessibility permission and CGEvent; add when macOS is supported.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_keys_only() {
        assert_eq!(parse("KeyV"), Some(Input::Key(b'V' as u16)));
        assert_eq!(parse("Digit5"), Some(Input::Key(b'5' as u16)));
        assert_eq!(parse("F13"), Some(Input::Key(0x7C)));
        assert_eq!(parse("Mouse4"), Some(Input::Mouse(0x80, 0x100, 1)));
        for bad in ["Keyv", "KeyAB", "F25", "F0", "Mouse9", "", "Escape"] {
            assert!(parse(bad).is_none(), "{bad}");
        }
    }
}
