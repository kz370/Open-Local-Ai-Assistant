use super::Db;
use crate::errors::{AppError, AppResult};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub model: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    /// "user" | "assistant" | "tool"
    pub role: String,
    pub content: String,
    pub language: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub sources: Option<serde_json::Value>,
    #[serde(default)]
    pub tool_activity: Option<serde_json::Value>,
    /// OpenAI-format tool calls requested by the assistant (kept so the
    /// conversation can be replayed to LM Studio faithfully).
    #[serde(default)]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// Files and images the user attached to this message, as stored by
    /// [`crate::services::attachments`].
    #[serde(default)]
    pub attachments: Option<serde_json::Value>,
    /// Generation stats of an assistant reply (speed, tokens, context use).
    #[serde(default)]
    pub stats: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub conversation: Conversation,
    pub snippet: String,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn conv_from_row(r: &Row) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: r.get(0)?,
        title: r.get(1)?,
        created_at: r.get(2)?,
        updated_at: r.get(3)?,
        model: r.get(4)?,
        language: r.get(5)?,
    })
}

fn json_col(v: Option<String>) -> Option<serde_json::Value> {
    v.and_then(|s| serde_json::from_str(&s).ok())
}

fn msg_from_row(r: &Row) -> rusqlite::Result<Message> {
    Ok(Message {
        id: r.get(0)?,
        conversation_id: r.get(1)?,
        role: r.get(2)?,
        content: r.get(3)?,
        language: r.get(4)?,
        reasoning: r.get(5)?,
        sources: json_col(r.get(6)?),
        tool_activity: json_col(r.get(7)?),
        tool_calls: json_col(r.get(8)?),
        tool_call_id: r.get(9)?,
        attachments: json_col(r.get(10)?),
        created_at: r.get(11)?,
        stats: json_col(r.get(12)?),
    })
}

const CONV_COLS: &str = "id, title, created_at, updated_at, model, language";
const MSG_COLS: &str = "id, conversation_id, role, content, language, reasoning, sources_json, tool_activity_json, tool_calls_json, tool_call_id, attachments_json, created_at, stats_json";

impl Db {
    pub fn create_conversation(&self, title: &str, model: Option<&str>) -> AppResult<Conversation> {
        let ts = now();
        let c = Conversation {
            id: new_id(),
            title: title.to_string(),
            created_at: ts.clone(),
            updated_at: ts,
            model: model.map(str::to_string),
            language: None,
        };
        self.insert_conversation(&c)?;
        Ok(c)
    }

    pub fn insert_conversation(&self, c: &Conversation) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO conversations (id, title, created_at, updated_at, model, language) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![c.id, c.title, c.created_at, c.updated_at, c.model, c.language],
        )?;
        Ok(())
    }

    pub fn get_conversation(&self, id: &str) -> AppResult<Conversation> {
        self.conn()
            .query_row(
                &format!("SELECT {CONV_COLS} FROM conversations WHERE id = ?1"),
                [id],
                conv_from_row,
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("conversation {id}")))
    }

    pub fn list_conversations(&self, limit: u32, offset: u32) -> AppResult<Vec<Conversation>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {CONV_COLS} FROM conversations ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit, offset], conv_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn rename_conversation(&self, id: &str, title: &str) -> AppResult<()> {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::Invalid("title must not be empty".into()));
        }
        let n = self.conn().execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now(), id],
        )?;
        if n == 0 {
            return Err(AppError::NotFound(format!("conversation {id}")));
        }
        Ok(())
    }

    pub fn touch_conversation(&self, id: &str, model: Option<&str>, language: Option<&str>) -> AppResult<()> {
        self.conn().execute(
            "UPDATE conversations SET updated_at = ?1,
                model = COALESCE(?2, model), language = COALESCE(?3, language)
             WHERE id = ?4",
            params![now(), model, language, id],
        )?;
        Ok(())
    }

    pub fn delete_conversation(&self, id: &str) -> AppResult<()> {
        self.conn().execute("DELETE FROM conversations WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Deletes every conversation (messages follow through the cascade) and
    /// returns how many were removed.
    pub fn delete_all_conversations(&self) -> AppResult<usize> {
        Ok(self.conn().execute("DELETE FROM conversations", [])?)
    }

    pub fn insert_message(&self, m: &Message) -> AppResult<()> {
        let to_s = |v: &Option<serde_json::Value>| v.as_ref().map(|v| v.to_string());
        self.conn().execute(
            &format!("INSERT INTO messages ({MSG_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"),
            params![
                m.id,
                m.conversation_id,
                m.role,
                m.content,
                m.language,
                m.reasoning,
                to_s(&m.sources),
                to_s(&m.tool_activity),
                to_s(&m.tool_calls),
                m.tool_call_id,
                to_s(&m.attachments),
                m.created_at,
                to_s(&m.stats)
            ],
        )?;
        Ok(())
    }

    pub fn list_messages(&self, conversation_id: &str) -> AppResult<Vec<Message>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {MSG_COLS} FROM messages WHERE conversation_id = ?1 ORDER BY created_at, rowid"
        ))?;
        let rows = stmt.query_map([conversation_id], msg_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn delete_messages_after(&self, conversation_id: &str, message_id: &str) -> AppResult<()> {
        self.conn().execute(
            "DELETE FROM messages WHERE conversation_id = ?1 AND rowid >= (SELECT rowid FROM messages WHERE id = ?2)",
            params![conversation_id, message_id],
        )?;
        Ok(())
    }

    /// Every attachment id referenced by a stored message. Used at start-up to
    /// delete attachment files that no conversation points at any more.
    pub fn attachment_ids(&self) -> AppResult<std::collections::HashSet<String>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT attachments_json FROM messages WHERE attachments_json IS NOT NULL")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut ids = std::collections::HashSet::new();
        for json in rows.flatten() {
            let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else { continue };
            ids.extend(values.iter().filter_map(|v| v["id"].as_str().map(str::to_string)));
        }
        Ok(ids)
    }

    /// Full-text search over message content plus title substring match.
    pub fn search_conversations(&self, query: &str, limit: u32) -> AppResult<Vec<SearchHit>> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(self
                .list_conversations(limit, 0)?
                .into_iter()
                .map(|c| SearchHit { conversation: c, snippet: String::new() })
                .collect());
        }
        // Quote every term so user input cannot inject FTS5 syntax.
        let fts_query = q
            .split_whitespace()
            .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" ");
        let like = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {cols}, snippet FROM (
                SELECT c.id, c.title, c.created_at, c.updated_at, c.model, c.language,
                       snippet(messages_fts, 0, '[', ']', '…', 12) AS snippet, 0 AS pri
                FROM messages_fts f JOIN conversations c ON c.id = f.conversation_id
                WHERE messages_fts MATCH ?1
                UNION ALL
                SELECT c.id, c.title, c.created_at, c.updated_at, c.model, c.language, '' AS snippet, 1 AS pri
                FROM conversations c WHERE c.title LIKE ?2 ESCAPE '\\'
            ) GROUP BY id ORDER BY MIN(pri), updated_at DESC LIMIT ?3",
            cols = CONV_COLS
        ))?;
        let rows = stmt.query_map(params![fts_query, like, limit], |r| {
            Ok(SearchHit { conversation: conv_from_row(r)?, snippet: r.get(6)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(conv: &str, role: &str, content: &str) -> Message {
        Message {
            id: new_id(),
            conversation_id: conv.into(),
            role: role.into(),
            content: content.into(),
            language: None,
            reasoning: None,
            sources: None,
            tool_activity: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: None,
            stats: None,
            created_at: now(),
        }
    }

    #[test]
    fn create_save_list_delete() {
        let db = Db::open_in_memory().unwrap();
        let c = db.create_conversation("Hello", Some("m")).unwrap();
        db.insert_message(&msg(&c.id, "user", "How do I organize files?")).unwrap();
        db.insert_message(&msg(&c.id, "assistant", "Use folders.")).unwrap();
        assert_eq!(db.list_messages(&c.id).unwrap().len(), 2);
        assert_eq!(db.list_conversations(10, 0).unwrap().len(), 1);
        db.rename_conversation(&c.id, "Files").unwrap();
        assert_eq!(db.get_conversation(&c.id).unwrap().title, "Files");
        db.delete_conversation(&c.id).unwrap();
        assert!(db.list_messages(&c.id).unwrap().is_empty());
        assert!(db.get_conversation(&c.id).is_err());
    }

    #[test]
    fn delete_all_clears_history() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_conversation("One", None).unwrap();
        let b = db.create_conversation("Two", None).unwrap();
        db.insert_message(&msg(&a.id, "user", "findable words")).unwrap();
        db.insert_message(&msg(&b.id, "user", "more words")).unwrap();
        assert_eq!(db.delete_all_conversations().unwrap(), 2);
        assert!(db.list_conversations(10, 0).unwrap().is_empty());
        assert!(db.list_messages(&a.id).unwrap().is_empty());
        assert!(db.search_conversations("findable", 10).unwrap().is_empty());
    }

    #[test]
    fn rename_rejects_empty() {
        let db = Db::open_in_memory().unwrap();
        let c = db.create_conversation("x", None).unwrap();
        assert!(db.rename_conversation(&c.id, "  ").is_err());
    }

    #[test]
    fn search_content_title_and_arabic() {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_conversation("PHP versions", None).unwrap();
        db.insert_message(&msg(&a.id, "user", "dependency injection explained")).unwrap();
        let b = db.create_conversation("Arabic", None).unwrap();
        db.insert_message(&msg(&b.id, "user", "كيف حالك اليوم؟")).unwrap();

        let hits = db.search_conversations("injection", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].conversation.id, a.id);
        assert!(hits[0].snippet.contains("[injection]"));

        assert_eq!(db.search_conversations("PHP", 10).unwrap()[0].conversation.id, a.id);
        assert_eq!(db.search_conversations("حالك", 10).unwrap()[0].conversation.id, b.id);
        // FTS syntax characters must not error
        assert!(db.search_conversations("\"AND OR*( ", 10).is_ok());
        assert_eq!(db.search_conversations("", 10).unwrap().len(), 2);
    }

    #[test]
    fn tool_messages_not_indexed() {
        let db = Db::open_in_memory().unwrap();
        let c = db.create_conversation("t", None).unwrap();
        db.insert_message(&msg(&c.id, "tool", "secretword")).unwrap();
        assert!(db.search_conversations("secretword", 10).unwrap().is_empty());
    }
}
