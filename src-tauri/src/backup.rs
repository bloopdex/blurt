//! Backup = one zip with a consistent DB snapshot plus the library files.
//! Restore is staged and applied on the next launch, before the database is opened, so the live
//! connection never has its file swapped underneath it.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::db::Db;
use crate::error::{AppError, AppResult, now_ms};
use crate::paths::Paths;

const MANIFEST: &str = "blurt-backup.json";

pub fn create(db: &Db, paths: &Paths, dest: &Path) -> AppResult<()> {
    let snapshot = paths.tmp_dir().join(format!("backup-{}.db", now_ms()));
    db.snapshot_to(&snapshot)?;
    let result = write_zip(paths, &snapshot, dest);
    let _ = std::fs::remove_file(&snapshot);
    if result.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    result
}

fn write_zip(paths: &Paths, snapshot: &Path, dest: &Path) -> AppResult<()> {
    let mut zip = zip::ZipWriter::new(File::create(dest)?);
    // Audio is already compressed; deflating it again only burns CPU.
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default();

    let manifest = serde_json::json!({ "app": "blurt", "version": env!("CARGO_PKG_VERSION"), "createdAt": now_ms() });
    zip.start_file(MANIFEST, deflated)?;
    zip.write_all(manifest.to_string().as_bytes())?;
    zip.start_file("blurt.db", deflated)?;
    std::io::copy(&mut File::open(snapshot)?, &mut zip)?;
    for (dir, prefix) in [(paths.sounds_dir(), "library/sounds/"), (paths.covers_dir(), "library/covers/")] {
        for entry in std::fs::read_dir(dir)?.flatten() {
            if entry.file_type()?.is_file() {
                zip.start_file(format!("{prefix}{}", entry.file_name().to_string_lossy()), stored)?;
                std::io::copy(&mut File::open(entry.path())?, &mut zip)?;
            }
        }
    }
    zip.finish()?;
    Ok(())
}

/// Validate and unpack into the staging folder. Takes effect after a restart.
pub fn stage_restore(paths: &Paths, src: &Path) -> AppResult<()> {
    let mut zip = zip::ZipArchive::new(File::open(src)?)?;
    let mut manifest = String::new();
    zip.by_name(MANIFEST)
        .map_err(|_| AppError::Invalid("That file isn't a Blurt backup.".into()))?
        .read_to_string(&mut manifest)?;
    let ok = serde_json::from_str::<serde_json::Value>(&manifest).ok().and_then(|v| v.get("app").cloned())
        == Some(serde_json::json!("blurt"));
    if !ok || zip.by_name("blurt.db").is_err() {
        return Err(AppError::Invalid("That file isn't a Blurt backup.".into()));
    }

    let staging = paths.restore_staging_dir();
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        // enclosed_name rejects absolute paths and `..` (zip-slip).
        let Some(rel) = entry.enclosed_name() else { continue };
        let allowed = rel == Path::new("blurt.db") || rel.starts_with("library/sounds") || rel.starts_with("library/covers");
        if !allowed || entry.is_dir() {
            continue;
        }
        let out = staging.join(rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::io::copy(&mut entry, &mut File::create(out)?)?;
    }
    Ok(())
}

/// Called at startup before the DB opens. The previous data is kept in `*.before-restore`.
pub fn apply_pending(root: &Path) -> std::io::Result<bool> {
    let staging = root.join("restore-pending");
    if !staging.join("blurt.db").is_file() {
        return Ok(false);
    }
    let old_db = root.join("blurt.db.before-restore");
    let old_lib = root.join("library.before-restore");
    let _ = std::fs::remove_file(&old_db);
    let _ = std::fs::remove_dir_all(&old_lib);
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(root.join(format!("blurt.db{suffix}")));
    }
    if root.join("blurt.db").exists() {
        std::fs::rename(root.join("blurt.db"), &old_db)?;
    }
    if root.join("library").exists() {
        std::fs::rename(root.join("library"), &old_lib)?;
    }
    std::fs::rename(staging.join("blurt.db"), root.join("blurt.db"))?;
    if staging.join("library").exists() {
        std::fs::rename(staging.join("library"), root.join("library"))?;
    }
    std::fs::remove_dir_all(&staging)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sounds::tests::sample;

    #[test]
    fn backup_restore_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("app");
        let paths = Paths::new(root.clone()).unwrap();
        let db = Db::open(&paths.db_file()).unwrap();
        db.insert_sound(&sample("a")).unwrap();
        std::fs::write(paths.sound_file("a.wav"), b"audio").unwrap();

        let zip_path = dir.path().join("b.zip");
        create(&db, &paths, &zip_path).unwrap();

        // Mutate after the backup, then restore.
        db.delete_sound("a").unwrap();
        std::fs::remove_file(paths.sound_file("a.wav")).unwrap();
        drop(db);
        stage_restore(&paths, &zip_path).unwrap();
        assert!(apply_pending(&root).unwrap());
        assert!(!apply_pending(&root).unwrap(), "only applies once");

        let db = Db::open(&paths.db_file()).unwrap();
        assert_eq!(db.get_sound("a").unwrap().name, "Sound a");
        assert_eq!(std::fs::read(paths.sound_file("a.wav")).unwrap(), b"audio");
        assert!(root.join("blurt.db.before-restore").exists());
    }

    #[test]
    fn rejects_foreign_zip() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("app")).unwrap();
        let z = dir.path().join("x.zip");
        let mut w = zip::ZipWriter::new(File::create(&z).unwrap());
        w.start_file("hello.txt", zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(b"hi").unwrap();
        w.finish().unwrap();
        assert!(stage_restore(&paths, &z).is_err());
    }
}
