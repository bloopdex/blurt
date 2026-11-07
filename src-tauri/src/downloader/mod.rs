//! URL → audio file, via yt-dlp. Runs entirely outside the audio engine: it only produces a file
//! that then goes through the normal import path.
//!
//! Policy: no cookies, no account sign-in, no DRM circumvention. Content that needs any of those
//! fails with an explanation.

pub mod sources;
pub mod ytdlp;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::Mutex;

use serde::Serialize;

use crate::error::{AppError, AppResult};
use sources::SourceInfo;

/// Formats Blurt can decode without ffmpeg, best first. Opus/WebM is deliberately avoided.
const FORMAT: &str = "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]/bestaudio[ext=mp3]/best[ext=mp4][acodec^=mp4a]/best[ext=mp4]";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub url: String,
    pub source: SourceInfo,
    pub title: String,
    pub thumbnail: Option<String>,
    pub duration_ms: Option<i64>,
    pub uploader: Option<String>,
}

#[derive(Default)]
pub struct Downloader {
    running: Mutex<HashMap<String, Child>>,
}

impl Downloader {
    /// Metadata only; nothing is downloaded.
    pub fn probe(bin: &Path, input: &str) -> AppResult<MediaInfo> {
        let (url, source) = sources::parse(input)?;
        let out = ytdlp::command(bin)
            .args(["--dump-single-json", "--no-playlist", "--no-warnings", "--socket-timeout", "15", "--"])
            .arg(url.as_str())
            .output()
            .map_err(|e| AppError::Download(format!("Couldn't start yt-dlp: {e}")))?;
        if !out.status.success() {
            return Err(AppError::Download(ytdlp::friendly_error(&String::from_utf8_lossy(&out.stderr))));
        }
        parse_info(&out.stdout, url.as_str(), source)
    }

    /// Blocking download into `dir`, named `<id>.<ext>`. `on_progress` receives 0–100.
    pub fn download(&self, bin: &Path, id: &str, url: &str, dir: &Path, on_progress: impl Fn(f32)) -> AppResult<PathBuf> {
        let (url, _) = sources::parse(url)?;
        std::fs::create_dir_all(dir)?;
        let mut child = ytdlp::command(bin)
            .args([
                "--no-playlist",
                "--no-warnings",
                "--newline",
                "--fixup",
                "never",
                "--socket-timeout",
                "20",
                "--max-filesize",
                "500M",
                "-f",
                FORMAT,
                "--progress-template",
                "download:BLURT %(progress._percent_str)s",
                "-o",
            ])
            .arg(dir.join(format!("{id}.%(ext)s")))
            .arg("--")
            .arg(url.as_str())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Download(format!("Couldn't start yt-dlp: {e}")))?;

        let stdout = child.stdout.take();
        let mut stderr = child.stderr.take();
        self.lock().insert(id.to_owned(), child);

        // Drain stderr on its own thread so a chatty process can't deadlock on a full pipe.
        let err_reader = std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(e) = stderr.as_mut() {
                let _ = e.read_to_string(&mut s);
            }
            s
        });
        if let Some(out) = stdout {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(p) = line.strip_prefix("BLURT ").and_then(|p| p.trim().trim_end_matches('%').parse::<f32>().ok()) {
                    on_progress(p.clamp(0.0, 100.0));
                }
            }
        }
        let stderr_text = err_reader.join().unwrap_or_default();
        let Some(mut child) = self.lock().remove(id) else {
            cleanup(dir, id);
            return Err(AppError::Download("Download cancelled.".into()));
        };
        let status = child.wait()?;
        if !status.success() {
            cleanup(dir, id);
            return Err(AppError::Download(ytdlp::friendly_error(&stderr_text)));
        }
        find_output(dir, id).ok_or_else(|| AppError::Download("yt-dlp finished but produced no audio file.".into()))
    }

    /// Kill a running download. The download thread notices and reports it as cancelled.
    pub fn cancel(&self, id: &str) -> bool {
        match self.lock().remove(id) {
            Some(mut child) => {
                if let Err(e) = child.kill() {
                    log::warn!("couldn't kill download {id}: {e}");
                }
                let _ = child.wait();
                true
            }
            None => false,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Child>> {
        self.running.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn parse_info(json: &[u8], url: &str, source: SourceInfo) -> AppResult<MediaInfo> {
    let v: serde_json::Value =
        serde_json::from_slice(json).map_err(|_| AppError::Download("yt-dlp returned unreadable metadata.".into()))?;
    if v.get("_type").and_then(|t| t.as_str()) == Some("playlist") {
        return Err(AppError::Download("That's a playlist. Paste a link to a single video or sound.".into()));
    }
    if v.get("is_live").and_then(|t| t.as_bool()) == Some(true) {
        return Err(AppError::Download("Live streams can't be downloaded.".into()));
    }
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    Ok(MediaInfo {
        url: url.to_owned(),
        source,
        title: s("title").unwrap_or_else(|| "Untitled".into()),
        thumbnail: s("thumbnail").filter(|t| t.starts_with("https://")),
        duration_ms: v.get("duration").and_then(|d| d.as_f64()).map(|d| (d * 1000.0) as i64),
        uploader: s("uploader").or_else(|| s("channel")),
    })
}

fn find_output(dir: &Path, id: &str) -> Option<PathBuf> {
    let prefix = format!("{id}.");
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix) && !n.ends_with(".part"))
    })
}

fn cleanup(dir: &Path, id: &str) {
    let prefix = format!("{id}.");
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_name().to_str().is_some_and(|n| n.starts_with(&prefix)) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_metadata_and_rejects_playlists() {
        let src = SourceInfo { id: "youtube", label: "YouTube" };
        let info = parse_info(
            br#"{"title":"Bruh","thumbnail":"https://i.ytimg.com/x.jpg","duration":2.5,"uploader":"u"}"#,
            "https://youtu.be/x",
            src.clone(),
        )
        .unwrap();
        assert_eq!((info.title.as_str(), info.duration_ms), ("Bruh", Some(2500)));
        assert!(parse_info(br#"{"_type":"playlist"}"#, "u", src.clone()).is_err());
        assert!(parse_info(br#"{"is_live":true}"#, "u", src.clone()).is_err());
        assert!(parse_info(b"nope", "u", src.clone()).is_err());
        let insecure = parse_info(br#"{"title":"t","thumbnail":"http://x/y.jpg"}"#, "u", src).unwrap();
        assert_eq!(insecure.thumbnail, None);
    }

    #[test]
    fn output_lookup_and_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("abc.m4a"), b"x").unwrap();
        std::fs::write(dir.path().join("abcd.m4a"), b"x").unwrap();
        assert_eq!(find_output(dir.path(), "abc").unwrap().file_name().unwrap(), "abc.m4a");
        cleanup(dir.path(), "abc");
        assert!(find_output(dir.path(), "abc").is_none());
        assert!(dir.path().join("abcd.m4a").exists());
    }
}
