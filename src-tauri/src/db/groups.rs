use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::Db;
use crate::error::{AppError, AppResult, now_ms};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub position: i64,
}

fn validate(kind: &str, name: &str, icon: &str, color: &str) -> AppResult<String> {
    if kind != "category" && kind != "collection" {
        return Err(AppError::Invalid(format!("Unknown group kind '{kind}'.")));
    }
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(AppError::Invalid("Names must be 1–60 characters.".into()));
    }
    let hex = color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !hex {
        return Err(AppError::Invalid("Colours must be #RRGGBB.".into()));
    }
    let icon_ok = !icon.is_empty()
        && icon.len() <= 40
        && icon.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !icon_ok {
        return Err(AppError::Invalid("Invalid icon name.".into()));
    }
    Ok(name.to_owned())
}

impl Db {
    pub fn list_groups(&self) -> AppResult<Vec<Group>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT id, kind, name, icon, color, position FROM groups ORDER BY kind, position, created_at")?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Group {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    name: r.get(2)?,
                    icon: r.get(3)?,
                    color: r.get(4)?,
                    position: r.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn create_group(&self, kind: &str, name: &str, icon: &str, color: &str) -> AppResult<Group> {
        let name = validate(kind, name, icon, color)?;
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn();
        let position: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM groups WHERE kind = ?1",
            [kind],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT INTO groups (id, kind, name, icon, color, position, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, kind, name, icon, color, position, now_ms()],
        )?;
        Ok(Group { id, kind: kind.into(), name, icon: icon.into(), color: color.into(), position })
    }

    pub fn update_group(&self, id: &str, name: &str, icon: &str, color: &str) -> AppResult<()> {
        let kind: String = self
            .conn()
            .query_row("SELECT kind FROM groups WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| AppError::NotFound("That category no longer exists.".into()))?;
        let name = validate(&kind, name, icon, color)?;
        self.conn().execute(
            "UPDATE groups SET name = ?2, icon = ?3, color = ?4 WHERE id = ?1",
            params![id, name, icon, color],
        )?;
        Ok(())
    }

    /// Sounds survive: their category is cleared (FK `ON DELETE SET NULL`) and collection links cascade.
    pub fn delete_group(&self, id: &str) -> AppResult<()> {
        self.conn().execute("DELETE FROM groups WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn reorder_groups(&self, ordered_ids: &[String]) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE groups SET position = ?2 WHERE id = ?1")?;
            for (i, id) in ordered_ids.iter().enumerate() {
                stmt.execute(params![id, i as i64])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub(super) fn require_group(&self, id: &str, kind: &str) -> AppResult<()> {
        let found: Option<String> =
            self.conn().query_row("SELECT kind FROM groups WHERE id = ?1", [id], |r| r.get(0)).optional()?;
        match found {
            Some(k) if k == kind => Ok(()),
            _ => Err(AppError::NotFound(format!("That {kind} no longer exists."))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_validate_reorder() {
        let db = Db::in_memory().unwrap();
        let g = db.create_group("collection", " Stream ", "radio", "#112233").unwrap();
        assert_eq!((g.name.as_str(), g.position), ("Stream", 0));
        assert!(db.create_group("category", "", "x", "#112233").is_err());
        assert!(db.create_group("category", "Ok", "x", "red").is_err());
        assert!(db.create_group("folder", "Ok", "x", "#112233").is_err());
        assert!(db.create_group("category", "Ok", "<svg>", "#112233").is_err());

        let cats = |db: &Db| -> Vec<String> {
            db.list_groups().unwrap().into_iter().filter(|g| g.kind == "category").map(|g| g.id).collect()
        };
        let reversed: Vec<String> = cats(&db).into_iter().rev().collect();
        db.reorder_groups(&reversed).unwrap();
        assert_eq!(cats(&db), reversed);
    }
}
