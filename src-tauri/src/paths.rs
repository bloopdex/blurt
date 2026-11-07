use std::path::{Path, PathBuf};

/// Every on-disk location Blurt uses, rooted in the OS app-data directory.
/// Database rows only store file *names*; absolute paths are derived here.
#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn new(root: PathBuf) -> std::io::Result<Self> {
        let p = Self { root };
        for dir in [p.sounds_dir(), p.covers_dir(), p.trash_dir(), p.tmp_dir(), p.tools_dir()] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(p)
    }
    pub fn db_file(&self) -> PathBuf {
        self.root.join("blurt.db")
    }
    pub fn library_dir(&self) -> PathBuf {
        self.root.join("library")
    }
    pub fn sounds_dir(&self) -> PathBuf {
        self.library_dir().join("sounds")
    }
    pub fn covers_dir(&self) -> PathBuf {
        self.library_dir().join("covers")
    }
    pub fn trash_dir(&self) -> PathBuf {
        self.library_dir().join("trash")
    }
    pub fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }
    pub fn tools_dir(&self) -> PathBuf {
        self.root.join("tools")
    }
    /// A restore is staged here and swapped in on the next launch, before the DB opens.
    pub fn restore_staging_dir(&self) -> PathBuf {
        self.root.join("restore-pending")
    }
    pub fn sound_file(&self, file_name: &str) -> PathBuf {
        self.sounds_dir().join(file_name)
    }
    pub fn cover_file(&self, file_name: &str) -> PathBuf {
        self.covers_dir().join(file_name)
    }
}

pub fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}
