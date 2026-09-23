//! Past dictations, kept locally so a lost or rejected insert can be recovered.

use super::Db;
use crate::errors::AppResult;
use rusqlite::params;
use serde::Serialize;

/// Oldest entries beyond this many are dropped on every save.
pub const HISTORY_LIMIT: usize = 100;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DictationEntry {
    pub id: String,
    pub created_at: String,
    pub raw: String,
    pub text: String,
    pub corrected: bool,
    /// False when typing or pasting into the target application failed.
    pub inserted: bool,
}

impl Db {
    pub fn dictation_add(&self, raw: &str, text: &str, corrected: bool, inserted: bool) -> AppResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO dictation_history(id, created_at, raw, text, corrected, inserted) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![super::conversations::new_id(), super::conversations::now(), raw, text, corrected, inserted],
        )?;
        conn.execute(
            "DELETE FROM dictation_history WHERE id NOT IN (SELECT id FROM dictation_history ORDER BY created_at DESC, rowid DESC LIMIT ?1)",
            [HISTORY_LIMIT as i64],
        )?;
        Ok(())
    }

    pub fn dictation_list(&self) -> AppResult<Vec<DictationEntry>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, created_at, raw, text, corrected, inserted FROM dictation_history ORDER BY created_at DESC, rowid DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(DictationEntry { id: r.get(0)?, created_at: r.get(1)?, raw: r.get(2)?, text: r.get(3)?, corrected: r.get(4)?, inserted: r.get(5)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn dictation_delete(&self, id: &str) -> AppResult<()> {
        self.conn().execute("DELETE FROM dictation_history WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn dictation_clear(&self) -> AppResult<()> {
        self.conn().execute("DELETE FROM dictation_history", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_roundtrip_and_trim() {
        let db = Db::open_in_memory().unwrap();
        db.dictation_add("hello wrld", "Hello world", true, true).unwrap();
        let list = db.dictation_list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].raw.as_str(), list[0].text.as_str(), list[0].corrected, list[0].inserted), ("hello wrld", "Hello world", true, true));

        for i in 0..HISTORY_LIMIT + 5 {
            db.dictation_add(&i.to_string(), &i.to_string(), false, false).unwrap();
        }
        let list = db.dictation_list().unwrap();
        assert_eq!(list.len(), HISTORY_LIMIT);
        assert_eq!(list[0].text, (HISTORY_LIMIT + 4).to_string(), "newest first");

        db.dictation_delete(&list[0].id).unwrap();
        assert_eq!(db.dictation_list().unwrap().len(), HISTORY_LIMIT - 1);
        db.dictation_clear().unwrap();
        assert!(db.dictation_list().unwrap().is_empty());
    }
}
