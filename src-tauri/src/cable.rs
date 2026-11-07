//! One-click virtual cable on Windows. VB-CABLE's licence forbids building it into another app's
//! installer, so Blurt downloads the official package from vb-audio.com, checks it is signed by
//! VB-Audio, and opens VB-Audio's own setup (the user approves Windows' prompt and clicks Install).

use std::path::Path;

use crate::error::{AppError, AppResult};

#[cfg(windows)]
const URL: &str = "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip";

/// Returns once VB-Audio's setup has closed and true if a new virtual cable appeared.
#[cfg(windows)]
pub fn install(tmp: &Path) -> AppResult<bool> {
    use crate::downloader::ytdlp::command;

    let before = crate::audio::devices::list().loopback_candidates().len();
    let dir = tmp.join("vbcable");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let zip_path = dir.join("pack.zip");
    let out = command(Path::new("curl"))
        .args(["--fail", "--location", "--silent", "--show-error", "--max-time", "120", "--output"])
        .arg(&zip_path)
        .arg(URL)
        .output()
        .map_err(|e| AppError::Download(format!("Couldn't run curl to fetch VB-CABLE: {e}")))?;
    if !out.status.success() {
        return Err(AppError::Download(format!(
            "Downloading VB-CABLE failed: {}. Check your internet connection.",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    zip::ZipArchive::new(std::fs::File::open(&zip_path)?)
        .and_then(|mut z| z.extract(&dir))
        .map_err(|e| AppError::Download(format!("The VB-CABLE download is damaged ({e}). Try again.")))?;

    // Signature check and elevation in one step; the path goes in through the environment, not the script text.
    let script = r#"
        $s = Get-AuthenticodeSignature -LiteralPath $env:BLURT_SETUP
        if ($s.Status -ne 'Valid' -or $s.SignerCertificate.Subject -notlike '*BUREL VINCENT*') { exit 3 }
        try { Start-Process -FilePath $env:BLURT_SETUP -WorkingDirectory (Split-Path $env:BLURT_SETUP) -Verb RunAs -Wait } catch { exit 4 }
    "#;
    let status = command(Path::new("powershell"))
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .env("BLURT_SETUP", dir.join("VBCABLE_Setup_x64.exe"))
        .status()
        .map_err(|e| AppError::Audio(format!("Couldn't start the VB-CABLE installer: {e}")))?;
    match status.code() {
        Some(3) => return Err(AppError::Audio("The downloaded installer isn't signed by VB-Audio, so Blurt didn't run it.".into())),
        Some(4) => return Err(AppError::Audio("Installation was cancelled. Windows needs your permission to add the virtual cable.".into())),
        _ => {}
    }
    // Windows can take a few seconds to publish the new endpoints.
    for _ in 0..30 {
        if crate::audio::devices::list().loopback_candidates().len() > before {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Ok(false)
}

#[cfg(not(windows))]
pub fn install(_tmp: &Path) -> AppResult<bool> {
    Err(AppError::Unsupported("Automatic install is Windows-only. On macOS, install BlackHole 2ch.".into()))
}
