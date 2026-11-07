//! SQLite persistence. One connection behind a mutex: the app is single-user and
//! every query is sub-millisecond, so a pool would only add moving parts.

pub mod downloads;
pub mod groups;
pub mod sounds;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::error::{AppResult, now_ms};

pub struct Db {
    conn: Mutex<Connection>,
}

struct Migration {
    version: i64,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration { version: 1, sql: include_str!("migrations/001_init.sql") }];

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> AppResult<Self> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 3000;")?;
        migrate(&mut conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock can't leave SQLite itself inconsistent, so recover.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get_setting(&self, key: &str) -> AppResult<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .conn()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn put_setting(&self, key: &str, value: &str) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    /// Consistent single-file snapshot of the live database (used by backups).
    pub fn snapshot_to(&self, dest: &Path) -> AppResult<()> {
        if dest.exists() {
            std::fs::remove_file(dest)?;
        }
        self.conn().execute("VACUUM INTO ?1", [dest.to_string_lossy()])?;
        Ok(())
    }
}

fn migrate(conn: &mut Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for m in MIGRATIONS.iter().filter(|m| m.version > current) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        tx.execute_batch(&format!("PRAGMA user_version = {}", m.version))?;
        tx.commit()?;
        log::info!("applied database migration {}", m.version);
    }
    if current == 0 {
        seed_categories(conn)?;
    }
    Ok(())
}

fn seed_categories(conn: &Connection) -> AppResult<()> {
    let defaults = [
        ("Memes", "laugh", "#FF5B3A"),
        ("Reactions", "zap", "#F5B83D"),
        ("Gaming", "gamepad-2", "#8B6CFF"),
        ("Music", "music", "#2BC38A"),
        ("Meetings", "briefcase", "#3BA7F5"),
    ];
    for (i, (name, icon, color)) in defaults.iter().enumerate() {
        conn.execute(
            "INSERT INTO groups (id, kind, name, icon, color, position, created_at) VALUES (?1, 'category', ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), name, icon, color, i as i64, now_ms()],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_once_and_seed_categories() {
        let db = Db::in_memory().unwrap();
        let version: i64 = db.conn().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, MIGRATIONS.last().unwrap().version);
        let count: i64 = db.conn().query_row("SELECT COUNT(*) FROM groups", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 5);
        // Re-running is a no-op.
        migrate(&mut db.conn()).unwrap();
        let count: i64 = db.conn().query_row("SELECT COUNT(*) FROM groups", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn settings_round_trip() {
        let db = Db::in_memory().unwrap();
        assert_eq!(db.get_setting("x").unwrap(), None);
        db.put_setting("x", "1").unwrap();
        db.put_setting("x", "2").unwrap();
        assert_eq!(db.get_setting("x").unwrap().as_deref(), Some("2"));
    }
}
