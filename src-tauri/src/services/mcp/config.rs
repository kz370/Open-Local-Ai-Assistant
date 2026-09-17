//! MCP server configuration persistence and LM Studio `mcp.json` import.

use crate::database::conversations::{new_id, now};
use crate::database::Db;
use crate::errors::{AppError, AppResult};
use crate::services::chat::tools::Permission;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// "stdio" | "http"
    pub transport: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub enabled: bool,
    /// "user" | "lmstudio"
    #[serde(default = "user_source")]
    pub source: String,
    #[serde(default)]
    pub created_at: String,
}

fn user_source() -> String {
    "user".into()
}

impl McpServerConfig {
    pub fn validate(&self) -> AppResult<()> {
        if self.name.trim().is_empty() {
            return Err(AppError::Invalid("server name is required".into()));
        }
        match self.transport.as_str() {
            "stdio" => {
                if self.command.as_deref().map(str::trim).unwrap_or("").is_empty() {
                    return Err(AppError::Invalid("command is required for local (stdio) servers".into()));
                }
            }
            "http" => {
                let url = self.url.as_deref().unwrap_or("");
                let parsed = url::Url::parse(url).map_err(|_| AppError::Invalid("a valid URL is required".into()))?;
                if !matches!(parsed.scheme(), "http" | "https") {
                    return Err(AppError::Invalid("URL must use http or https".into()));
                }
            }
            _ => return Err(AppError::Invalid("transport must be stdio or http".into())),
        }
        Ok(())
    }
}

fn row_to_config(r: &rusqlite::Row) -> rusqlite::Result<McpServerConfig> {
    let json_map = |s: Option<String>| -> BTreeMap<String, String> { s.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default() };
    Ok(McpServerConfig {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        transport: r.get(3)?,
        command: r.get(4)?,
        args: r.get::<_, Option<String>>(5)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
        env: json_map(r.get(6)?),
        url: r.get(7)?,
        headers: json_map(r.get(8)?),
        enabled: r.get::<_, i64>(9)? != 0,
        source: r.get(10)?,
        created_at: r.get(11)?,
    })
}

const COLS: &str = "id, name, description, transport, command, args_json, env_json, url, headers_json, enabled, source, created_at";

impl Db {
    pub fn list_mcp_servers(&self) -> AppResult<Vec<McpServerConfig>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM mcp_servers ORDER BY created_at"))?;
        let rows = stmt.query_map([], row_to_config)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn get_mcp_server(&self, id: &str) -> AppResult<McpServerConfig> {
        self.conn()
            .query_row(&format!("SELECT {COLS} FROM mcp_servers WHERE id = ?1"), [id], row_to_config)
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("MCP server {id}")))
    }

    pub fn save_mcp_server(&self, c: &McpServerConfig) -> AppResult<McpServerConfig> {
        c.validate()?;
        let mut c = c.clone();
        if c.id.is_empty() {
            c.id = new_id();
        }
        if c.created_at.is_empty() {
            c.created_at = now();
        }
        self.conn().execute(
            &format!(
                "INSERT INTO mcp_servers ({COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, description=excluded.description, transport=excluded.transport,
                 command=excluded.command, args_json=excluded.args_json, env_json=excluded.env_json, url=excluded.url,
                 headers_json=excluded.headers_json, enabled=excluded.enabled"
            ),
            params![
                c.id,
                c.name.trim(),
                c.description,
                c.transport,
                c.command,
                serde_json::to_string(&c.args)?,
                serde_json::to_string(&c.env)?,
                c.url,
                serde_json::to_string(&c.headers)?,
                c.enabled as i64,
                c.source,
                c.created_at
            ],
        )?;
        Ok(c)
    }

    pub fn set_mcp_enabled(&self, id: &str, enabled: bool) -> AppResult<()> {
        self.conn().execute("UPDATE mcp_servers SET enabled = ?1 WHERE id = ?2", params![enabled as i64, id])?;
        Ok(())
    }

    pub fn delete_mcp_server(&self, id: &str) -> AppResult<()> {
        self.conn().execute("DELETE FROM mcp_servers WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn tool_permissions(&self, server_id: &str) -> AppResult<BTreeMap<String, Permission>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT tool_name, permission FROM mcp_tool_permissions WHERE server_id = ?1")?;
        let rows = stmt.query_map([server_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = BTreeMap::new();
        for row in rows {
            let (tool, perm) = row?;
            if let Ok(p) = serde_json::from_value::<Permission>(serde_json::Value::String(perm)) {
                out.insert(tool, p);
            }
        }
        Ok(out)
    }

    pub fn set_tool_permission(&self, server_id: &str, tool: &str, permission: Permission) -> AppResult<()> {
        let p = serde_json::to_value(permission)?.as_str().unwrap_or("ask").to_string();
        self.conn().execute(
            "INSERT INTO mcp_tool_permissions (server_id, tool_name, permission) VALUES (?1, ?2, ?3)
             ON CONFLICT(server_id, tool_name) DO UPDATE SET permission = excluded.permission",
            params![server_id, tool, p],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub env_keys: Vec<String>,
    pub already_configured: bool,
}

pub fn lmstudio_mcp_json_path() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let p = PathBuf::from(home).join(".lmstudio").join("mcp.json");
    p.exists().then_some(p)
}

/// Parses `{"mcpServers": {name: {command, args, env} | {url, headers}}}`.
pub fn parse_mcp_json(json: &str) -> AppResult<Vec<McpServerConfig>> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| AppError::Invalid(format!("invalid mcp.json: {e}")))?;
    let servers = v.get("mcpServers").and_then(|s| s.as_object()).ok_or_else(|| AppError::Invalid("mcp.json has no mcpServers".into()))?;
    let str_map = |v: Option<&serde_json::Value>| -> BTreeMap<String, String> {
        v.and_then(|m| m.as_object())
            .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
            .unwrap_or_default()
    };
    let mut out = Vec::new();
    for (name, cfg) in servers {
        let url = cfg.get("url").or_else(|| cfg.get("serverUrl")).and_then(|u| u.as_str()).map(str::to_string);
        let command = cfg.get("command").and_then(|c| c.as_str()).map(str::to_string);
        let transport = if url.is_some() && command.is_none() { "http" } else { "stdio" };
        out.push(McpServerConfig {
            id: String::new(),
            name: name.clone(),
            description: String::new(),
            transport: transport.into(),
            command,
            args: cfg.get("args").and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            env: str_map(cfg.get("env")),
            url,
            headers: str_map(cfg.get("headers")),
            enabled: false,
            source: "lmstudio".into(),
            created_at: String::new(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"mcpServers":{
        "searxng":{"command":"npx","args":["-y","mcp-searxng"],"env":{"SEARXNG_URL":"http://localhost:8080"}},
        "notion":{"url":"https://mcp.notion.com/mcp"},
        "bad":{"foo":1}
    }}"#;

    #[test]
    fn parses_lmstudio_config() {
        let servers = parse_mcp_json(SAMPLE).unwrap();
        let s = servers.iter().find(|s| s.name == "searxng").unwrap();
        assert_eq!(s.transport, "stdio");
        assert_eq!(s.args, vec!["-y", "mcp-searxng"]);
        assert_eq!(s.env["SEARXNG_URL"], "http://localhost:8080");
        assert!(!s.enabled, "imported servers must start disabled");
        let n = servers.iter().find(|s| s.name == "notion").unwrap();
        assert_eq!(n.transport, "http");
        assert!(servers.iter().find(|s| s.name == "bad").unwrap().validate().is_err());
    }

    #[test]
    fn crud_and_permissions() {
        let db = Db::open_in_memory().unwrap();
        let mut cfg = parse_mcp_json(SAMPLE).unwrap().into_iter().find(|s| s.name == "searxng").unwrap();
        cfg = db.save_mcp_server(&cfg).unwrap();
        assert_eq!(db.list_mcp_servers().unwrap().len(), 1);
        db.set_mcp_enabled(&cfg.id, true).unwrap();
        assert!(db.get_mcp_server(&cfg.id).unwrap().enabled);
        db.set_tool_permission(&cfg.id, "search", Permission::Deny).unwrap();
        assert_eq!(db.tool_permissions(&cfg.id).unwrap()["search"], Permission::Deny);
        db.delete_mcp_server(&cfg.id).unwrap();
        assert!(db.tool_permissions(&cfg.id).unwrap().is_empty());
    }

    #[test]
    fn validation() {
        let mut c = parse_mcp_json(SAMPLE).unwrap().remove(0);
        c.transport = "http".into();
        c.url = Some("file:///x".into());
        assert!(c.validate().is_err());
        c.url = Some("https://example.com/mcp".into());
        assert!(c.validate().is_ok());
    }
}
