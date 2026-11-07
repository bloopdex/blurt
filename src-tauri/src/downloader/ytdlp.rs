//! Locating, installing and talking to yt-dlp. Blurt does not bundle it: it is fetched on demand
//! from the official GitHub release so it can be updated independently (sites change often).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{AppError, AppResult};

#[cfg(windows)]
const BINARY: &str = "yt-dlp.exe";
#[cfg(not(windows))]
const BINARY: &str = "yt-dlp";

#[cfg(windows)]
const RELEASE_ASSET: &str = "yt-dlp.exe";
#[cfg(target_os = "macos")]
const RELEASE_ASSET: &str = "yt-dlp_macos";
#[cfg(all(unix, not(target_os = "macos")))]
const RELEASE_ASSET: &str = "yt-dlp";

/// Command with no console window flashing up on Windows.
pub fn command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// App-managed copy first, then PATH.
pub fn locate(tools_dir: &Path) -> Option<PathBuf> {
    let managed = tools_dir.join(BINARY);
    if managed.is_file() {
        return Some(managed);
    }
    std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|p| p.join(BINARY)).find(|p| p.is_file()))
}

pub fn version(bin: &Path) -> Option<String> {
    let out = command(bin).arg("--version").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Download the official release binary with the OS's own curl (present on Windows 10+ and macOS).
pub fn install(tools_dir: &Path) -> AppResult<PathBuf> {
    let url = format!("https://github.com/yt-dlp/yt-dlp/releases/latest/download/{RELEASE_ASSET}");
    let dest = tools_dir.join(BINARY);
    let partial = tools_dir.join(format!("{BINARY}.part"));
    let out = command(Path::new("curl"))
        .args(["--fail", "--location", "--silent", "--show-error", "--max-time", "300", "--output"])
        .arg(&partial)
        .arg(&url)
        .output()
        .map_err(|e| AppError::Download(format!("Couldn't run curl to fetch yt-dlp: {e}")))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&partial);
        return Err(AppError::Download(format!(
            "Downloading yt-dlp failed: {}. Check your internet connection.",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&partial, &dest)?;
    version(&dest).ok_or_else(|| AppError::Download("yt-dlp was downloaded but won't run on this system.".into()))?;
    Ok(dest)
}

/// Turn yt-dlp's stderr into something a person can act on.
pub fn friendly_error(stderr: &str) -> String {
    let s = stderr.to_lowercase();
    let table: &[(&[&str], &str)] = &[
        (&["private video", "this video is private"], "This video is private, so it can't be downloaded."),
        (&["sign in to confirm your age", "age-restricted", "age restricted"], "This video is age-restricted and requires signing in; Blurt doesn't use your account, so it can't be downloaded."),
        (&["in your country", "geo restrict", "geo-restrict"], "This content isn't available in your region."),
        (&["drm"], "This content is DRM-protected. Blurt doesn't bypass copy protection."),
        (&["unavailable", "has been removed", "404", "does not exist"], "This content is unavailable — it may have been removed."),
        (&["unsupported url"], "This site or link type isn't supported."),
        (&["login required", "log in", "requires authentication", "cookies"], "This content requires signing in, which Blurt doesn't do."),
        (&["live event", "is live", "premieres in"], "Live streams and premieres can't be downloaded until they've finished."),
        (&["requested format is not available"], "No downloadable audio was found for this link."),
        (&["unable to download webpage", "getaddrinfo", "timed out", "connection"], "Couldn't reach the site. Check your internet connection and try again."),
        (&["http error 429", "too many requests"], "The site is rate-limiting downloads. Wait a few minutes and try again."),
    ];
    for (needles, msg) in table {
        if needles.iter().any(|n| s.contains(n)) {
            return (*msg).to_owned();
        }
    }
    let last = stderr.lines().rev().find(|l| l.contains("ERROR")).unwrap_or("").trim();
    if last.is_empty() {
        "The download failed. Updating yt-dlp in Settings → Storage often fixes this.".into()
    } else {
        format!("The download failed: {}. Updating yt-dlp often fixes site changes.", last.trim_start_matches("ERROR: "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_failures() {
        assert!(friendly_error("ERROR: [youtube] abc: Private video. Sign in").contains("private"));
        assert!(friendly_error("ERROR: Sign in to confirm your age").contains("age-restricted"));
        assert!(friendly_error("ERROR: The uploader has not made this video available in your country").contains("region"));
        assert!(friendly_error("ERROR: Unsupported URL: https://x.org").contains("isn't supported"));
        assert!(friendly_error("ERROR: [youtube] x: This video is unavailable").contains("removed"));
        assert!(friendly_error("ERROR: weird new thing").contains("weird new thing"));
        assert!(friendly_error("").contains("Updating yt-dlp"));
    }
}
