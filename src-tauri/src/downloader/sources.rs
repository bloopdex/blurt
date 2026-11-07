//! URL validation and source detection. Adding a source = adding a row to `SOURCES`.

use serde::Serialize;
use url::Url;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub id: &'static str,
    pub label: &'static str,
}

/// (id, label, host suffixes)
const SOURCES: &[(&str, &str, &[&str])] = &[
    ("youtube", "YouTube", &["youtube.com", "youtu.be", "youtube-nocookie.com"]),
    ("tiktok", "TikTok", &["tiktok.com"]),
    ("instagram", "Instagram", &["instagram.com"]),
    ("x", "X / Twitter", &["x.com", "twitter.com"]),
    ("soundcloud", "SoundCloud", &["soundcloud.com"]),
    ("reddit", "Reddit", &["reddit.com", "redd.it"]),
    ("twitch", "Twitch", &["twitch.tv"]),
    ("vimeo", "Vimeo", &["vimeo.com"]),
];

const OTHER: SourceInfo = SourceInfo { id: "other", label: "Other site" };

/// Only http(s) URLs with a real host. Anything else is rejected before it reaches yt-dlp.
pub fn parse(input: &str) -> AppResult<(Url, SourceInfo)> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AppError::Invalid("Paste a link to a video or sound.".into()));
    }
    let with_scheme = if input.contains("://") { input.to_owned() } else { format!("https://{input}") };
    let url = Url::parse(&with_scheme).map_err(|_| AppError::Invalid("That doesn't look like a valid link.".into()))?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(AppError::Invalid("Only http(s) links are supported.".into()));
    }
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    if !host.contains('.') {
        return Err(AppError::Invalid("That link is missing a website address.".into()));
    }
    Ok((url.clone(), detect(&host)))
}

fn detect(host: &str) -> SourceInfo {
    SOURCES
        .iter()
        .find(|(_, _, hosts)| hosts.iter().any(|h| host == *h || host.ends_with(&format!(".{h}"))))
        .map(|(id, label, _)| SourceInfo { id, label })
        .unwrap_or(OTHER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_sources_and_rejects_bad_input() {
        assert_eq!(parse("https://www.youtube.com/watch?v=abc").unwrap().1.id, "youtube");
        assert_eq!(parse("youtu.be/abc").unwrap().1.id, "youtube");
        assert_eq!(parse("https://m.tiktok.com/@a/video/1").unwrap().1.id, "tiktok");
        assert_eq!(parse("https://www.instagram.com/reel/x/").unwrap().1.id, "instagram");
        assert_eq!(parse("https://example.org/a.mp3").unwrap().1.id, "other");
        // Look-alike hosts are not the real site.
        assert_eq!(parse("https://notyoutube.com/x").unwrap().1.id, "other");
        assert!(parse("").is_err());
        assert!(parse("file:///etc/passwd").is_err());
        assert!(parse("--exec calc").is_err());
        assert!(parse("ftp://host.com/a").is_err());
    }
}
