use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::Db;
use crate::error::{AppError, AppResult, now_ms};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRecord {
    pub id: String,
    pub url: String,
    pub source: String,
    pub title: Option<String>,
    pub thumbnail_url: Option<String>,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub error: Option<String>,
    pub temp_file: Option<String>,
    pub sound_id: Option<String>,
    pub created_at: i64,
}

const SELECT: &str = "SELECT id, url, source, title, thumbnail_url, duration_ms, status, error, temp_file, sound_id,
    created_at FROM downloads";

fn from_row(r: &rusqlite::Row) -> rusqlite::Result<DownloadRecord> {
    Ok(DownloadRecord {
        id: r.get(0)?,
        url: r.get(1)?,
        source: r.get(2)?,
        title: r.get(3)?,
        thumbnail_url: r.get(4)?,
        duration_ms: r.get(5)?,
        status: r.get(6)?,
        error: r.get(7)?,
        temp_file: r.get(8)?,
        sound_id: r.get(9)?,
        created_at: r.get(10)?,
    })
}

pub struct NewDownload<'a> {
    pub id: &'a str,
    pub url: &'a str,
    pub source: &'a str,
    pub title: Option<&'a str>,
    pub thumbnail_url: Option<&'a str>,
    pub duration_ms: Option<i64>,
}

impl Db {
    pub fn list_downloads(&self) -> AppResult<Vec<DownloadRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY created_at DESC LIMIT 200"))?;
        let rows = stmt.query_map([], from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_download(&self, id: &str) -> AppResult<DownloadRecord> {
        self.conn()
            .query_row(&format!("{SELECT} WHERE id = ?1"), [id], from_row)
            .optional()?
            .ok_or_else(|| AppError::NotFound("That download no longer exists.".into()))
    }

    pub fn insert_download(&self, d: &NewDownload) -> AppResult<()> {
        let now = now_ms();
        self.conn().execute(
            "INSERT INTO downloads (id, url, source, title, thumbnail_url, duration_ms, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'downloading', ?7, ?7)",
            params![d.id, d.url, d.source, d.title, d.thumbnail_url, d.duration_ms, now],
        )?;
        Ok(())
    }

    pub fn finish_download(
        &self,
        id: &str,
        status: &str,
        error: Option<&str>,
        temp_file: Option<&str>,
    ) -> AppResult<()> {
        self.conn().execute(
            "UPDATE downloads SET status = ?2, error = ?3, temp_file = ?4, updated_at = ?5 WHERE id = ?1",
            params![id, status, error, temp_file, now_ms()],
        )?;
        Ok(())
    }

    pub fn mark_download_imported(&self, id: &str, sound_id: &str) -> AppResult<()> {
        self.conn().execute(
            "UPDATE downloads SET status = 'imported', sound_id = ?2, temp_file = NULL, updated_at = ?3 WHERE id = ?1",
            params![id, sound_id, now_ms()],
        )?;
        Ok(())
    }

    pub fn delete_download(&self, id: &str) -> AppResult<()> {
        self.conn().execute("DELETE FROM downloads WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Downloads interrupted by a crash or quit can't resume; mark them so the UI offers a retry.
    pub fn fail_interrupted_downloads(&self) -> AppResult<()> {
        self.conn().execute(
            "UPDATE downloads SET status = 'failed', error = 'Interrupted when Blurt closed. Try again.'
             WHERE status = 'downloading'",
            [],
        )?;
        Ok(())
    }

    pub fn ready_temp_files(&self) -> AppResult<Vec<String>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT temp_file FROM downloads WHERE status = 'ready' AND temp_file IS NOT NULL")?;
        let rows = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_and_interrupt_recovery() {
        let db = Db::in_memory().unwrap();
        let new = |id| NewDownload {
            id,
            url: "https://youtu.be/x",
            source: "youtube",
            title: Some("T"),
            thumbnail_url: None,
            duration_ms: Some(1000),
        };
        db.insert_download(&new("a")).unwrap();
        db.insert_download(&new("b")).unwrap();
        db.finish_download("a", "ready", None, Some("a.m4a")).unwrap();
        assert_eq!(db.ready_temp_files().unwrap(), vec!["a.m4a"]);

        db.fail_interrupted_downloads().unwrap();
        let b = db.get_download("b").unwrap();
        assert_eq!(b.status, "failed");
        assert!(b.error.is_some());
        assert_eq!(db.get_download("a").unwrap().status, "ready");
    }
}
