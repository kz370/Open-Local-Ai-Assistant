//! Conversation export (JSON / Markdown / TXT) and import (JSON).

use super::conversations::{new_id, Conversation, Message};
use super::Db;
use crate::errors::{AppError, AppResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Markdown,
    Txt,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedConversation {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub conversations: Vec<ExportedConversation>,
}

const BUNDLE_FORMAT: &str = "local-assistant-conversations";

fn visible(messages: &[Message]) -> impl Iterator<Item = &Message> {
    messages
        .iter()
        .filter(|m| (m.role == "user" || m.role == "assistant") && !m.content.trim().is_empty())
}

impl Db {
    pub fn export_conversations(&self, ids: &[String], format: ExportFormat) -> AppResult<String> {
        let mut items = Vec::new();
        for id in ids {
            items.push(ExportedConversation {
                conversation: self.get_conversation(id)?,
                messages: self.list_messages(id)?,
            });
        }
        Ok(match format {
            ExportFormat::Json => serde_json::to_string_pretty(&ExportBundle {
                format: BUNDLE_FORMAT.into(),
                version: 1,
                exported_at: super::conversations::now(),
                conversations: items,
            })?,
            ExportFormat::Markdown => items
                .iter()
                .map(|e| {
                    let mut s = format!("# {}\n\n", e.conversation.title);
                    for m in visible(&e.messages) {
                        let who = if m.role == "user" { "You" } else { "Assistant" };
                        s.push_str(&format!("**{who}** — {}\n\n{}\n\n", m.created_at, m.content));
                        if let Some(serde_json::Value::Array(srcs)) = &m.sources {
                            if !srcs.is_empty() {
                                s.push_str("Sources:\n");
                                for src in srcs {
                                    let url = src.get("url").and_then(|v| v.as_str()).unwrap_or("");
                                    let title = src.get("title").and_then(|v| v.as_str()).unwrap_or(url);
                                    s.push_str(&format!("- [{title}]({url})\n"));
                                }
                                s.push('\n');
                            }
                        }
                    }
                    s
                })
                .collect::<Vec<_>>()
                .join("\n---\n\n"),
            ExportFormat::Txt => items
                .iter()
                .map(|e| {
                    let mut s = format!("{}\n{}\n\n", e.conversation.title, "=".repeat(e.conversation.title.chars().count().max(3)));
                    for m in visible(&e.messages) {
                        let who = if m.role == "user" { "You" } else { "Assistant" };
                        s.push_str(&format!("[{}] {who}:\n{}\n\n", m.created_at, m.content));
                    }
                    s
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
        })
    }

    /// Imports a JSON bundle. Imported conversations always receive fresh IDs
    /// so an import can never overwrite existing data. Returns the new IDs.
    pub fn import_conversations(&self, json: &str) -> AppResult<Vec<String>> {
        let bundle: ExportBundle = serde_json::from_str(json)
            .map_err(|e| AppError::Invalid(format!("not a valid conversation export: {e}")))?;
        if bundle.format != BUNDLE_FORMAT {
            return Err(AppError::Invalid("unsupported export format".into()));
        }
        let mut ids = Vec::new();
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        for item in bundle.conversations {
            let new_conv_id = new_id();
            tx.execute(
                "INSERT INTO conversations (id, title, created_at, updated_at, model, language) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    new_conv_id,
                    item.conversation.title,
                    item.conversation.created_at,
                    item.conversation.updated_at,
                    item.conversation.model,
                    item.conversation.language
                ],
            )?;
            for m in item.messages {
                if !matches!(m.role.as_str(), "user" | "assistant" | "assistant_tool_calls" | "tool") {
                    continue;
                }
                let to_s = |v: &Option<serde_json::Value>| v.as_ref().map(|v| v.to_string());
                tx.execute(
                    "INSERT INTO messages (id, conversation_id, role, content, language, reasoning, sources_json, tool_activity_json, tool_calls_json, tool_call_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    rusqlite::params![
                        new_id(), new_conv_id, m.role, m.content, m.language, m.reasoning,
                        to_s(&m.sources), to_s(&m.tool_activity), to_s(&m.tool_calls), m.tool_call_id, m.created_at
                    ],
                )?;
            }
            ids.push(new_conv_id);
        }
        tx.commit()?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::conversations::now;

    fn seed(db: &Db) -> String {
        let c = db.create_conversation("Deutsch Test", None).unwrap();
        for (role, content) in [("user", "Wie geht es dir?"), ("assistant", "Gut, danke!"), ("tool", "{}")] {
            db.insert_message(&Message {
                id: new_id(),
                conversation_id: c.id.clone(),
                role: role.into(),
                content: content.into(),
                language: Some("de".into()),
                reasoning: None,
                sources: if role == "assistant" {
                    Some(serde_json::json!([{"url": "https://php.net", "title": "PHP"}]))
                } else {
                    None
                },
                tool_activity: None,
                tool_calls: None,
                tool_call_id: None,
                created_at: now(),
            })
            .unwrap();
        }
        c.id
    }

    #[test]
    fn export_formats() {
        let db = Db::open_in_memory().unwrap();
        let id = seed(&db);
        let md = db.export_conversations(&[id.clone()], ExportFormat::Markdown).unwrap();
        assert!(md.contains("# Deutsch Test") && md.contains("Gut, danke!") && md.contains("[PHP](https://php.net)"));
        assert!(!md.contains("{}"), "tool messages are not exported to markdown");
        let txt = db.export_conversations(&[id.clone()], ExportFormat::Txt).unwrap();
        assert!(txt.contains("You:\nWie geht es dir?"));
        let json = db.export_conversations(&[id], ExportFormat::Json).unwrap();
        assert!(json.contains(BUNDLE_FORMAT));
    }

    #[test]
    fn import_roundtrip_creates_new_ids() {
        let db = Db::open_in_memory().unwrap();
        let id = seed(&db);
        let json = db.export_conversations(&[id.clone()], ExportFormat::Json).unwrap();
        let ids = db.import_conversations(&json).unwrap();
        assert_eq!(ids.len(), 1);
        assert_ne!(ids[0], id);
        assert_eq!(db.list_messages(&ids[0]).unwrap().len(), 3);
        assert_eq!(db.list_conversations(10, 0).unwrap().len(), 2);
    }

    #[test]
    fn import_rejects_garbage() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.import_conversations("{\"format\":\"x\"}").is_err());
        assert!(db.import_conversations("nope").is_err());
    }
}
