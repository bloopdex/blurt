//! Library file management: imports are copied into the app-managed library so sounds keep
//! working if the original file moves; deletions go to a trash folder first.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::audio::decode::{SUPPORTED_EXTENSIONS, decode_file, waveform};
use crate::db::Db;
use crate::db::sounds::{NewSound, Sound, validate_name};
use crate::error::{AppError, AppResult};
use crate::paths::Paths;

pub const WAVEFORM_BINS: usize = 96;
const MAX_IMPORT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_COVER_BYTES: u64 = 10 * 1024 * 1024;
const COVER_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: Vec<Sound>,
    pub failed: Vec<ImportFailure>,
}

pub struct ImportOptions {
    pub name: Option<String>,
    pub category_id: Option<String>,
    pub volume: f64,
    pub source_url: Option<String>,
    pub trim_start_ms: i64,
    pub trim_end_ms: Option<i64>,
}

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// Decode (validating the file), copy it into the library, then insert the row. If the insert
/// fails the copied file is removed so the library never holds orphans.
pub fn import_file(db: &Db, paths: &Paths, src: &Path, opts: ImportOptions) -> AppResult<Sound> {
    let ext = extension(src);
    if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::Unsupported(format!(
            "\".{ext}\" files aren't supported. Use MP3, WAV, OGG, FLAC, M4A/AAC or AIFF."
        )));
    }
    let meta = std::fs::metadata(src)?;
    if !meta.is_file() {
        return Err(AppError::Invalid("That isn't a file.".into()));
    }
    if meta.len() > MAX_IMPORT_BYTES {
        return Err(AppError::Invalid("Files over 512 MB can't be imported.".into()));
    }
    let decoded = decode_file(src)?;
    let duration_ms = decoded.pcm.duration_ms();
    let trim_end_ms = opts.trim_end_ms.filter(|&e| e < duration_ms);
    crate::db::sounds::validate_trim(opts.trim_start_ms, trim_end_ms, duration_ms)?;
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled");
    let name = match opts.name.as_deref() {
        Some(n) => validate_name(n)?,
        None => validate_name(&truncate(stem, 80)).unwrap_or_else(|_| "Untitled".into()),
    };

    let id = uuid::Uuid::new_v4().to_string();
    let file_name = format!("{id}.{ext}");
    let dest = paths.sound_file(&file_name);
    std::fs::copy(src, &dest)?;
    let new = NewSound {
        id,
        name,
        file_name,
        original_name: src.file_name().and_then(|s| s.to_str()).unwrap_or("").to_owned(),
        format: ext,
        duration_ms,
        file_size: meta.len() as i64,
        sample_rate: decoded.pcm.rate as i64,
        channels: decoded.source_channels as i64,
        volume: opts.volume,
        category_id: opts.category_id,
        waveform: waveform(&decoded.pcm, WAVEFORM_BINS),
        source_url: opts.source_url,
        trim_start_ms: opts.trim_start_ms,
        trim_end_ms,
    };
    db.insert_sound(&new).inspect_err(|_| {
        if let Err(e) = std::fs::remove_file(&dest) {
            log::error!("couldn't remove orphaned import {}: {e}", dest.display());
        }
    })
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Import a generated file (clip, speech) under `name`, then remove the temporary file.
pub fn import_generated(db: &Db, paths: &Paths, tmp: &Path, name: String, volume: f64) -> AppResult<Sound> {
    let opts = ImportOptions { name: Some(name), category_id: None, volume, source_url: None, trim_start_ms: 0, trim_end_ms: None };
    let result = import_file(db, paths, tmp, opts);
    let _ = std::fs::remove_file(tmp);
    result
}

/// Save the instant-clip buffer as a new sound named "Clip #N".
pub fn save_clip(db: &Db, paths: &Paths, clips: &crate::audio::clip::ClipRecorder, volume: f64) -> AppResult<Sound> {
    let tmp = paths.tmp_dir().join(format!("clip-{}.wav", uuid::Uuid::new_v4().simple()));
    clips.save(&tmp)?;
    let n = db.list_sounds()?.iter().filter(|s| s.name.starts_with("Clip #")).count() + 1;
    import_generated(db, paths, &tmp, format!("Clip #{n}"), volume)
}

pub fn duplicate(db: &Db, paths: &Paths, id: &str) -> AppResult<Sound> {
    let s = db.get_sound(id)?;
    let new_id = uuid::Uuid::new_v4().to_string();
    let file_name = format!("{new_id}.{}", s.format);
    std::fs::copy(paths.sound_file(&s.file_name), paths.sound_file(&file_name))?;
    let new = NewSound {
        id: new_id,
        name: truncate(&format!("{} (copy)", s.name), 80),
        file_name: file_name.clone(),
        original_name: s.original_name,
        format: s.format,
        duration_ms: s.duration_ms,
        file_size: s.file_size,
        sample_rate: s.sample_rate,
        channels: s.channels,
        volume: s.volume,
        category_id: s.category_id,
        waveform: s.waveform,
        source_url: s.source_url,
        trim_start_ms: s.trim_start_ms,
        trim_end_ms: s.trim_end_ms,
    };
    db.insert_sound(&new).inspect_err(|_| {
        let _ = std::fs::remove_file(paths.sound_file(&file_name));
    })
}

/// Row first, then the file moves to trash: a crash in between leaves a recoverable file, never a
/// row pointing at nothing.
pub fn delete(db: &Db, paths: &Paths, id: &str) -> AppResult<PathBuf> {
    let s = db.get_sound(id)?;
    db.delete_sound(id)?;
    let src = paths.sound_file(&s.file_name);
    let trashed = paths.trash_dir().join(&s.file_name);
    if src.exists() {
        std::fs::rename(&src, &trashed)?;
    }
    if let Some(cover) = s.cover_file {
        let _ = std::fs::remove_file(paths.cover_file(&cover));
    }
    Ok(src)
}

pub fn set_cover(db: &Db, paths: &Paths, id: &str, image: Option<&Path>) -> AppResult<Sound> {
    let s = db.get_sound(id)?;
    let new_file = match image {
        None => None,
        Some(img) => {
            let ext = extension(img);
            if !COVER_EXTENSIONS.contains(&ext.as_str()) {
                return Err(AppError::Unsupported("Covers must be PNG, JPG, WebP or GIF images.".into()));
            }
            if std::fs::metadata(img)?.len() > MAX_COVER_BYTES {
                return Err(AppError::Invalid("Cover images are limited to 10 MB.".into()));
            }
            // A fresh name per change also busts the webview's image cache.
            let name = format!("{id}-{}.{ext}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
            std::fs::copy(img, paths.cover_file(&name))?;
            Some(name)
        }
    };
    db.set_cover(id, new_file.as_deref())?;
    if let Some(old) = s.cover_file {
        let _ = std::fs::remove_file(paths.cover_file(&old));
    }
    db.get_sound(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::decode::tests::{sine, write_wav};

    fn setup() -> (tempfile::TempDir, Db, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("app")).unwrap();
        (dir, Db::in_memory().unwrap(), paths)
    }

    fn opts() -> ImportOptions {
        ImportOptions { name: None, category_id: None, volume: 1.0, source_url: None, trim_start_ms: 0, trim_end_ms: None }
    }

    #[test]
    fn import_copies_into_library_and_records_metadata() {
        let (dir, db, paths) = setup();
        let src = dir.path().join("Air Horn.wav");
        write_wav(&src, 44100, 2, &sine(44100, 1.0, 0.4));
        let s = import_file(&db, &paths, &src, opts()).unwrap();
        assert_eq!(s.name, "Air Horn");
        assert_eq!((s.duration_ms, s.sample_rate, s.format.as_str()), (500, 44100, "wav"));
        assert_eq!(s.waveform.len(), WAVEFORM_BINS);
        assert!(paths.sound_file(&s.file_name).exists());
        std::fs::remove_file(&src).unwrap();
        assert!(paths.sound_file(&s.file_name).exists(), "library copy is independent of the source");
    }

    #[test]
    fn import_rejects_unsupported_and_cleans_up_on_db_failure() {
        let (dir, db, paths) = setup();
        let txt = dir.path().join("notes.txt");
        std::fs::write(&txt, "hi").unwrap();
        assert!(matches!(import_file(&db, &paths, &txt, opts()), Err(AppError::Unsupported(_))));

        let src = dir.path().join("a.wav");
        write_wav(&src, 8000, 1, &sine(8000, 0.5, 0.4));
        let mut o = opts();
        o.category_id = Some("missing".into());
        assert!(import_file(&db, &paths, &src, o).is_err());
        assert_eq!(std::fs::read_dir(paths.sounds_dir()).unwrap().count(), 0, "no orphaned file");
    }

    #[test]
    fn delete_moves_file_to_trash_and_duplicate_copies() {
        let (dir, db, paths) = setup();
        let src = dir.path().join("a.wav");
        write_wav(&src, 8000, 1, &sine(8000, 0.5, 0.4));
        let s = import_file(&db, &paths, &src, opts()).unwrap();
        let copy = duplicate(&db, &paths, &s.id).unwrap();
        assert_eq!(copy.name, "a (copy)");
        assert_ne!(copy.file_name, s.file_name);

        delete(&db, &paths, &s.id).unwrap();
        assert!(db.get_sound(&s.id).is_err());
        assert!(paths.trash_dir().join(&s.file_name).exists());
        assert!(paths.sound_file(&copy.file_name).exists());
    }
}
