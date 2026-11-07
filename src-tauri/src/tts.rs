//! Text-to-speech into the mic: the system voice (Windows SAPI, macOS `say`) renders the text to a
//! file, which becomes a normal sound so it can be replayed or bound to a key.

use std::path::Path;

use crate::db::sounds::Sound;
use crate::downloader::ytdlp::command;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const MAX_CHARS: usize = 500;

pub fn speak(state: &AppState, text: &str, voice: Option<&str>) -> AppResult<Sound> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_CHARS {
        return Err(AppError::Invalid(format!("Type between 1 and {MAX_CHARS} characters.")));
    }
    let ext = if cfg!(windows) { "wav" } else { "aiff" };
    let out = state.paths.tmp_dir().join(format!("tts-{}.{ext}", uuid::Uuid::new_v4().simple()));
    render(text, voice, &out)?;
    let name: String = format!("TTS: {text}").chars().take(60).collect();
    crate::library::import_generated(&state.db, &state.paths, &out, name, state.settings().default_sound_volume as f64)
}

// Text, voice and path travel in environment variables, never inside the script, so nothing typed can run as code.
#[cfg(windows)]
fn render(text: &str, voice: Option<&str>, out: &Path) -> AppResult<()> {
    const SCRIPT: &str = "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
        if ($env:BLURT_TTS_VOICE) { $s.SelectVoice($env:BLURT_TTS_VOICE) }; \
        $s.SetOutputToWaveFile($env:BLURT_TTS_OUT); $s.Speak($env:BLURT_TTS_TEXT); $s.Dispose()";
    let o = command(Path::new("powershell"))
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("BLURT_TTS_TEXT", text)
        .env("BLURT_TTS_VOICE", voice.unwrap_or(""))
        .env("BLURT_TTS_OUT", out)
        .output()
        .map_err(|e| AppError::Audio(format!("Couldn't start Windows speech: {e}")))?;
    if !o.status.success() || !out.exists() {
        return Err(AppError::Audio(format!("Windows speech failed: {}", String::from_utf8_lossy(&o.stderr).trim())));
    }
    Ok(())
}

#[cfg(not(windows))]
fn render(text: &str, voice: Option<&str>, out: &Path) -> AppResult<()> {
    let mut cmd = command(Path::new("say"));
    if let Some(v) = voice {
        cmd.args(["-v", v]);
    }
    let o = cmd.arg("-o").arg(out).arg("--").arg(text).output().map_err(|e| AppError::Audio(format!("Couldn't run say: {e}")))?;
    if !o.status.success() {
        return Err(AppError::Audio(format!("Speech failed: {}", String::from_utf8_lossy(&o.stderr).trim())));
    }
    Ok(())
}

/// Installed system voices; empty means "use the default voice".
pub fn voices() -> Vec<String> {
    #[cfg(windows)]
    {
        const SCRIPT: &str = "Add-Type -AssemblyName System.Speech; (New-Object System.Speech.Synthesis.SpeechSynthesizer).GetInstalledVoices() | Where-Object Enabled | ForEach-Object { $_.VoiceInfo.Name }";
        command(Path::new("powershell"))
            .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
            .unwrap_or_default()
    }
    #[cfg(not(windows))]
    Vec::new()
}
