use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Deserializer, Serialize};

use super::Db;
use crate::error::{AppError, AppResult, now_ms};

pub const PLAYBACK_MODES: &[&str] = &["default", "restart", "toggle", "overlap", "hold"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sound {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file_name: String,
    pub original_name: String,
    pub format: String,
    pub duration_ms: i64,
    pub file_size: i64,
    pub sample_rate: i64,
    pub channels: i64,
    pub cover_file: Option<String>,
    pub volume: f64,
    pub trim_start_ms: i64,
    pub trim_end_ms: Option<i64>,
    pub playback_mode: String,
    pub category_id: Option<String>,
    pub collection_ids: Vec<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub play_count: i64,
    pub last_played_at: Option<i64>,
    pub position: i64,
    pub shortcut: Option<String>,
    pub waveform: Vec<u8>,
    pub source_url: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Everything known about a file at import time.
pub struct NewSound {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub original_name: String,
    pub format: String,
    pub duration_ms: i64,
    pub file_size: i64,
    pub sample_rate: i64,
    pub channels: i64,
    pub volume: f64,
    pub category_id: Option<String>,
    pub waveform: Vec<u8>,
    pub source_url: Option<String>,
    pub trim_start_ms: i64,
    pub trim_end_ms: Option<i64>,
}

/// Partial update from the editor. `Option<Option<T>>` distinguishes "leave alone" from "clear".
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SoundPatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub volume: Option<f64>,
    pub trim_start_ms: Option<i64>,
    #[serde(default, deserialize_with = "double_option")]
    pub trim_end_ms: Option<Option<i64>>,
    pub playback_mode: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub category_id: Option<Option<String>>,
    pub is_favorite: Option<bool>,
    pub is_pinned: Option<bool>,
}

fn double_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
    Ok(Some(Option::deserialize(d)?))
}

const SELECT: &str = "SELECT s.id, s.name, s.description, s.file_name, s.original_name, s.format, s.duration_ms,
    s.file_size, s.sample_rate, s.channels, s.cover_file, s.volume, s.trim_start_ms, s.trim_end_ms,
    s.playback_mode, s.category_id,
    (SELECT group_concat(collection_id) FROM collection_items ci WHERE ci.sound_id = s.id),
    s.is_favorite, s.is_pinned, s.play_count, s.last_played_at, s.position, s.shortcut, s.waveform,
    s.source_url, s.created_at, s.updated_at
    FROM sounds s";

fn from_row(r: &Row) -> rusqlite::Result<Sound> {
    let collections: Option<String> = r.get(16)?;
    Ok(Sound {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        file_name: r.get(3)?,
        original_name: r.get(4)?,
        format: r.get(5)?,
        duration_ms: r.get(6)?,
        file_size: r.get(7)?,
        sample_rate: r.get(8)?,
        channels: r.get(9)?,
        cover_file: r.get(10)?,
        volume: r.get(11)?,
        trim_start_ms: r.get(12)?,
        trim_end_ms: r.get(13)?,
        playback_mode: r.get(14)?,
        category_id: r.get(15)?,
        collection_ids: collections.map(|c| c.split(',').map(str::to_owned).collect()).unwrap_or_default(),
        is_favorite: r.get(17)?,
        is_pinned: r.get(18)?,
        play_count: r.get(19)?,
        last_played_at: r.get(20)?,
        position: r.get(21)?,
        shortcut: r.get(22)?,
        waveform: r.get(23)?,
        source_url: r.get(24)?,
        created_at: r.get(25)?,
        updated_at: r.get(26)?,
    })
}

impl Db {
    pub fn list_sounds(&self) -> AppResult<Vec<Sound>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY s.position, s.created_at"))?;
        let rows = stmt.query_map([], from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_sound(&self, id: &str) -> AppResult<Sound> {
        self.conn()
            .query_row(&format!("{SELECT} WHERE s.id = ?1"), [id], from_row)
            .optional()?
            .ok_or_else(|| AppError::NotFound("That sound no longer exists.".into()))
    }

    pub fn insert_sound(&self, s: &NewSound) -> AppResult<Sound> {
        if let Some(cid) = &s.category_id {
            self.require_group(cid, "category")?;
        }
        let now = now_ms();
        {
            let conn = self.conn();
            let position: i64 =
                conn.query_row("SELECT COALESCE(MAX(position), -1) + 1 FROM sounds", [], |r| r.get(0))?;
            conn.execute(
                "INSERT INTO sounds (id, name, file_name, original_name, format, duration_ms, file_size, sample_rate,
                    channels, volume, category_id, waveform, source_url, trim_start_ms, trim_end_ms, position,
                    created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?17)",
                params![
                    s.id,
                    s.name,
                    s.file_name,
                    s.original_name,
                    s.format,
                    s.duration_ms,
                    s.file_size,
                    s.sample_rate,
                    s.channels,
                    s.volume,
                    s.category_id,
                    s.waveform,
                    s.source_url,
                    s.trim_start_ms,
                    s.trim_end_ms,
                    position,
                    now
                ],
            )?;
        }
        self.get_sound(&s.id)
    }

    pub fn update_sound(&self, id: &str, patch: SoundPatch) -> AppResult<Sound> {
        let mut s = self.get_sound(id)?;
        if let Some(name) = patch.name {
            s.name = validate_name(&name)?;
        }
        if let Some(d) = patch.description {
            if d.chars().count() > 500 {
                return Err(AppError::Invalid("Descriptions are limited to 500 characters.".into()));
            }
            s.description = d;
        }
        if let Some(v) = patch.volume {
            if !(0.0..=2.0).contains(&v) {
                return Err(AppError::Invalid("Volume must be between 0% and 200%.".into()));
            }
            s.volume = v;
        }
        if let Some(t) = patch.trim_start_ms {
            s.trim_start_ms = t;
        }
        if let Some(t) = patch.trim_end_ms {
            s.trim_end_ms = t;
        }
        validate_trim(s.trim_start_ms, s.trim_end_ms, s.duration_ms)?;
        if let Some(m) = patch.playback_mode {
            if !PLAYBACK_MODES.contains(&m.as_str()) {
                return Err(AppError::Invalid(format!("Unknown playback mode '{m}'.")));
            }
            s.playback_mode = m;
        }
        if let Some(c) = patch.category_id {
            if let Some(cid) = &c {
                self.require_group(cid, "category")?;
            }
            s.category_id = c;
        }
        if let Some(f) = patch.is_favorite {
            s.is_favorite = f;
        }
        if let Some(p) = patch.is_pinned {
            s.is_pinned = p;
        }
        self.conn().execute(
            "UPDATE sounds SET name = ?2, description = ?3, volume = ?4, trim_start_ms = ?5, trim_end_ms = ?6,
                playback_mode = ?7, category_id = ?8, is_favorite = ?9, is_pinned = ?10, updated_at = ?11 WHERE id = ?1",
            params![
                id,
                s.name,
                s.description,
                s.volume,
                s.trim_start_ms,
                s.trim_end_ms,
                s.playback_mode,
                s.category_id,
                s.is_favorite,
                s.is_pinned,
                now_ms()
            ],
        )?;
        self.get_sound(id)
    }

    pub fn set_cover(&self, id: &str, cover_file: Option<&str>) -> AppResult<()> {
        self.conn().execute(
            "UPDATE sounds SET cover_file = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, cover_file, now_ms()],
        )?;
        Ok(())
    }

    /// `None` clears. The UNIQUE constraint is the final guard against two sounds sharing a shortcut.
    pub fn set_shortcut(&self, id: &str, shortcut: Option<&str>) -> AppResult<()> {
        let res = self.conn().execute(
            "UPDATE sounds SET shortcut = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, shortcut, now_ms()],
        );
        match res {
            Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                Err(AppError::Shortcut("Another sound already uses that shortcut.".into()))
            }
            other => other.map(|_| ()).map_err(Into::into),
        }
    }

    pub fn delete_sound(&self, id: &str) -> AppResult<()> {
        self.conn().execute("DELETE FROM sounds WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn record_play(&self, id: &str) -> AppResult<()> {
        self.conn().execute(
            "UPDATE sounds SET play_count = play_count + 1, last_played_at = ?2 WHERE id = ?1",
            params![id, now_ms()],
        )?;
        Ok(())
    }

    /// Persist a full ordering in one transaction.
    pub fn reorder_sounds(&self, ordered_ids: &[String]) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE sounds SET position = ?2 WHERE id = ?1")?;
            for (i, id) in ordered_ids.iter().enumerate() {
                stmt.execute(params![id, i as i64])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn move_to_category(&self, ids: &[String], category_id: Option<&str>) -> AppResult<()> {
        if let Some(cid) = category_id {
            self.require_group(cid, "category")?;
        }
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE sounds SET category_id = ?2, updated_at = ?3 WHERE id = ?1")?;
            let now = now_ms();
            for id in ids {
                stmt.execute(params![id, category_id, now])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_collection_membership(&self, collection_id: &str, ids: &[String], member: bool) -> AppResult<()> {
        self.require_group(collection_id, "collection")?;
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let now = now_ms();
            if member {
                let mut stmt = tx.prepare(
                    "INSERT OR IGNORE INTO collection_items (collection_id, sound_id, added_at) VALUES (?1, ?2, ?3)",
                )?;
                for id in ids {
                    stmt.execute(params![collection_id, id, now])?;
                }
            } else {
                let mut stmt = tx.prepare("DELETE FROM collection_items WHERE collection_id = ?1 AND sound_id = ?2")?;
                for id in ids {
                    stmt.execute(params![collection_id, id])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn shortcuts(&self) -> AppResult<Vec<(String, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, shortcut FROM sounds WHERE shortcut IS NOT NULL")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

pub fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    match name.chars().count() {
        0 => Err(AppError::Invalid("Give the sound a name.".into())),
        n if n > 80 => Err(AppError::Invalid("Names are limited to 80 characters.".into())),
        _ => Ok(name.to_owned()),
    }
}

pub fn validate_trim(start: i64, end: Option<i64>, duration: i64) -> AppResult<()> {
    let end = end.unwrap_or(duration);
    if start < 0 || end > duration || end - start < 50 {
        return Err(AppError::Invalid("The trimmed clip must be at least 50 ms long and inside the sound.".into()));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn sample(id: &str) -> NewSound {
        NewSound {
            id: id.into(),
            name: format!("Sound {id}"),
            file_name: format!("{id}.wav"),
            original_name: "a.wav".into(),
            format: "wav".into(),
            duration_ms: 2000,
            file_size: 10,
            sample_rate: 48000,
            channels: 2,
            volume: 1.0,
            category_id: None,
            waveform: vec![1, 2, 3],
            source_url: None,
            trim_start_ms: 0,
            trim_end_ms: None,
        }
    }

    fn patch(json: &str) -> SoundPatch {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn insert_update_and_patch_validation() {
        let db = Db::in_memory().unwrap();
        let s = db.insert_sound(&sample("a")).unwrap();
        assert_eq!(s.position, 0);
        assert_eq!(db.insert_sound(&sample("b")).unwrap().position, 1);

        let s = db.update_sound("a", patch(r#"{"name":"  Airhorn ","trimEndMs":1500,"isFavorite":true}"#)).unwrap();
        assert_eq!((s.name.as_str(), s.trim_end_ms, s.is_favorite), ("Airhorn", Some(1500), true));

        // Explicit null clears the trim end.
        let s = db.update_sound("a", patch(r#"{"trimEndMs":null}"#)).unwrap();
        assert_eq!(s.trim_end_ms, None);

        assert!(db.update_sound("a", patch(r#"{"volume":3}"#)).is_err());
        assert!(db.update_sound("a", patch(r#"{"trimStartMs":1990}"#)).is_err());
        assert!(db.update_sound("a", patch(r#"{"name":"   "}"#)).is_err());
        assert!(db.update_sound("a", patch(r#"{"categoryId":"nope"}"#)).is_err());
        assert!(db.update_sound("a", patch(r#"{"playbackMode":"loop"}"#)).is_err());
        assert!(serde_json::from_str::<SoundPatch>(r#"{"filePath":"x"}"#).is_err());
    }

    #[test]
    fn shortcuts_are_unique() {
        let db = Db::in_memory().unwrap();
        db.insert_sound(&sample("a")).unwrap();
        db.insert_sound(&sample("b")).unwrap();
        db.set_shortcut("a", Some("control+alt+Digit1")).unwrap();
        assert!(matches!(db.set_shortcut("b", Some("control+alt+Digit1")), Err(AppError::Shortcut(_))));
        assert_eq!(db.shortcuts().unwrap(), vec![("a".to_string(), "control+alt+Digit1".to_string())]);
    }

    #[test]
    fn deleting_category_keeps_sound_and_collections_cascade() {
        let db = Db::in_memory().unwrap();
        let cat = db.create_group("category", "Temp", "folder", "#FFFFFF").unwrap();
        let col = db.create_group("collection", "Stream", "star", "#FFFFFF").unwrap();
        let mut n = sample("a");
        n.category_id = Some(cat.id.clone());
        db.insert_sound(&n).unwrap();
        db.set_collection_membership(&col.id, &["a".into()], true).unwrap();
        assert_eq!(db.get_sound("a").unwrap().collection_ids, vec![col.id.clone()]);

        db.delete_group(&cat.id).unwrap();
        assert_eq!(db.get_sound("a").unwrap().category_id, None);
        db.delete_sound("a").unwrap();
        let left: i64 = db.conn().query_row("SELECT COUNT(*) FROM collection_items", [], |r| r.get(0)).unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn reorder_move_and_play_count() {
        let db = Db::in_memory().unwrap();
        for id in ["a", "b", "c"] {
            db.insert_sound(&sample(id)).unwrap();
        }
        db.reorder_sounds(&["c".into(), "a".into(), "b".into()]).unwrap();
        let order: Vec<String> = db.list_sounds().unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(order, ["c", "a", "b"]);

        let cat = db.list_groups().unwrap().into_iter().find(|g| g.kind == "category").unwrap();
        db.move_to_category(&["a".into(), "b".into()], Some(&cat.id)).unwrap();
        assert_eq!(db.get_sound("b").unwrap().category_id.as_deref(), Some(cat.id.as_str()));

        db.record_play("a").unwrap();
        db.record_play("a").unwrap();
        let a = db.get_sound("a").unwrap();
        assert_eq!(a.play_count, 2);
        assert!(a.last_played_at.is_some());
    }
}
