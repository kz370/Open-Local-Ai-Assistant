//! Generic MCP (Model Context Protocol) client manager.
//!
//! Servers are never installed or enabled automatically. Only enabled servers
//! are connected; their tools are classified and filtered by permissions
//! before being offered to the local LLM.

pub mod config;
pub mod permissions;
pub mod sources;

use crate::database::Db;
use crate::errors::{AppError, AppResult};
use crate::services::chat::tools::{Permission, ToolCategory, ToolOutput, ToolProvider, ToolSpec};
use async_trait::async_trait;
use config::McpServerConfig;
use permissions::{clamp_permission, classify, default_permission};
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolView {
    pub name: String,
    pub description: String,
    pub category: ToolCategory,
    pub permission: Permission,
    pub default_permission: Permission,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub config: McpServerConfig,
    /// "disabled" | "connecting" | "connected" | "error" | "offline"
    pub state: String,
    pub error: Option<String>,
    pub tools: Vec<ToolView>,
    pub internet: bool,
}

struct Connection {
    service: RunningService<RoleClient, ()>,
    tools: Vec<rmcp::model::Tool>,
}

#[derive(Default)]
struct Inner {
    connections: HashMap<String, Arc<Connection>>,
    states: HashMap<String, (String, Option<String>)>,
}

pub struct McpManager {
    db: Arc<Db>,
    inner: RwLock<Inner>,
    on_change: Arc<dyn Fn() + Send + Sync>,
}

impl McpManager {
    pub fn new(db: Arc<Db>, on_change: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self { db, inner: RwLock::new(Inner::default()), on_change }
    }

    /// Connects every enabled server (called at startup, in the background).
    pub async fn connect_enabled(self: &Arc<Self>) {
        let Ok(servers) = self.db.list_mcp_servers() else { return };
        for s in servers.into_iter().filter(|s| s.enabled) {
            let me = self.clone();
            tokio::spawn(async move {
                let _ = me.connect(&s.id).await;
            });
        }
    }

    pub async fn connect(&self, id: &str) -> AppResult<()> {
        let cfg = self.db.get_mcp_server(id)?;
        self.disconnect(id).await;
        if !cfg.enabled {
            return Ok(());
        }
        self.set_state(id, "connecting", None).await;
        let result = tokio::time::timeout(CONNECT_TIMEOUT, open(&cfg)).await;
        match result {
            Ok(Ok(conn)) => {
                tracing::info!(server = %cfg.name, tools = conn.tools.len(), "MCP server connected");
                let mut inner = self.inner.write().await;
                inner.connections.insert(id.into(), Arc::new(conn));
                inner.states.insert(id.into(), ("connected".into(), None));
                drop(inner);
                (self.on_change)();
                Ok(())
            }
            Ok(Err(e)) => {
                let offline = is_offline_error(&e);
                tracing::warn!(server = %cfg.name, error = %e, "MCP connection failed");
                self.set_state(id, if offline { "offline" } else { "error" }, Some(e.to_string())).await;
                Err(e)
            }
            Err(_) => {
                let e = AppError::Timeout(format!("MCP server {} did not respond", cfg.name));
                self.set_state(id, "error", Some(e.to_string())).await;
                Err(e)
            }
        }
    }

    pub async fn disconnect(&self, id: &str) {
        let conn = {
            let mut inner = self.inner.write().await;
            inner.states.remove(id);
            inner.connections.remove(id)
        };
        if let Some(conn) = conn {
            if let Ok(conn) = Arc::try_unwrap(conn) {
                let _ = conn.service.cancel().await;
            }
        }
        (self.on_change)();
    }

    pub async fn shutdown(&self) {
        let ids: Vec<String> = self.inner.read().await.connections.keys().cloned().collect();
        for id in ids {
            self.disconnect(&id).await;
        }
    }

    async fn set_state(&self, id: &str, state: &str, error: Option<String>) {
        self.inner.write().await.states.insert(id.into(), (state.into(), error));
        (self.on_change)();
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> AppResult<()> {
        self.db.set_mcp_enabled(id, enabled)?;
        if enabled {
            // Connection errors are reflected in status, not returned.
            let _ = self.connect(id).await;
        } else {
            self.disconnect(id).await;
        }
        Ok(())
    }

    pub async fn set_permission(&self, server_id: &str, tool: &str, requested: Permission) -> AppResult<Permission> {
        let conn = self.inner.read().await.connections.get(server_id).cloned();
        let category = conn
            .and_then(|c| c.tools.iter().find(|t| t.name == tool).map(tool_category))
            .unwrap_or(ToolCategory::Other);
        let effective = clamp_permission(category, requested);
        self.db.set_tool_permission(server_id, tool, effective)?;
        (self.on_change)();
        Ok(effective)
    }

    pub async fn statuses(&self) -> AppResult<Vec<ServerStatus>> {
        let servers = self.db.list_mcp_servers()?;
        let inner = self.inner.read().await;
        let mut out = Vec::new();
        for cfg in servers {
            let perms = self.db.tool_permissions(&cfg.id)?;
            let tools: Vec<ToolView> = inner
                .connections
                .get(&cfg.id)
                .map(|c| {
                    c.tools
                        .iter()
                        .map(|t| {
                            let category = tool_category(t);
                            let default = default_permission(category);
                            ToolView {
                                name: t.name.to_string(),
                                description: t.description.as_deref().unwrap_or("").to_string(),
                                category,
                                permission: perms.get(t.name.as_ref()).copied().map(|p| clamp_permission(category, p)).unwrap_or(default),
                                default_permission: default,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            let (state, error) = if !cfg.enabled {
                ("disabled".to_string(), None)
            } else {
                inner.states.get(&cfg.id).cloned().unwrap_or(("connecting".into(), None))
            };
            let internet = cfg.transport == "http" || tools.iter().any(|t| matches!(t.category, ToolCategory::Search | ToolCategory::Fetch));
            out.push(ServerStatus { config: cfg, state, error, tools, internet });
        }
        Ok(out)
    }

    /// True when an enabled, connected server exposes internet tools.
    pub async fn internet_available(&self) -> bool {
        self.statuses()
            .await
            .map(|s| s.iter().any(|x| x.state == "connected" && x.tools.iter().any(|t| matches!(t.category, ToolCategory::Search | ToolCategory::Fetch) && t.permission != Permission::Deny)))
            .unwrap_or(false)
    }
}

fn tool_category(t: &rmcp::model::Tool) -> ToolCategory {
    let ann = t.annotations.as_ref();
    classify(
        &t.name,
        t.description.as_deref().unwrap_or(""),
        ann.and_then(|a| a.read_only_hint),
        ann.and_then(|a| a.destructive_hint),
    )
}

fn is_offline_error(e: &AppError) -> bool {
    let s = e.to_string().to_lowercase();
    ["dns", "resolve", "network is unreachable", "no route", "connection refused", "connect error", "timed out", "offline"]
        .iter()
        .any(|k| s.contains(k))
}

/// Sanitized, unique tool name for the LLM: `<server>__<tool>` limited to 64 chars.
pub fn llm_tool_name(server: &str, tool: &str, taken: &mut Vec<String>) -> String {
    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect::<String>()
            .trim_matches('_')
            .to_ascii_lowercase()
    };
    let mut base = format!("{}__{}", clean(server), clean(tool));
    base.truncate(60);
    let mut name = base.clone();
    let mut n = 2;
    while taken.contains(&name) {
        name = format!("{base}{n}");
        n += 1;
    }
    taken.push(name.clone());
    name
}

#[cfg(windows)]
fn command_for(cfg: &McpServerConfig) -> tokio::process::Command {
    // npx/npm/yarn/pnpm are .cmd shims on Windows and need cmd.exe.
    let program = cfg.command.clone().unwrap_or_default();
    let needs_shell = std::path::Path::new(&program).extension().is_none()
        && matches!(program.to_ascii_lowercase().as_str(), "npx" | "npm" | "pnpm" | "yarn" | "bunx" | "corepack");
    let mut cmd = if needs_shell {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(&program);
        c
    } else {
        tokio::process::Command::new(&program)
    };
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.args(&cfg.args);
    cmd
}

#[cfg(not(windows))]
fn command_for(cfg: &McpServerConfig) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(cfg.command.clone().unwrap_or_default());
    cmd.args(&cfg.args);
    cmd
}

async fn open(cfg: &McpServerConfig) -> AppResult<Connection> {
    let service = match cfg.transport.as_str() {
        "stdio" => {
            let mut cmd = command_for(cfg);
            cmd.envs(&cfg.env);
            let (transport, _stderr) = rmcp::transport::TokioChildProcess::builder(cmd)
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| AppError::Mcp(format!("could not start '{}': {e}", cfg.command.as_deref().unwrap_or(""))))?;
            ().serve(transport).await.map_err(|e| AppError::Mcp(e.to_string()))?
        }
        "http" => {
            use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
            let mut headers = reqwest::header::HeaderMap::new();
            for (k, v) in &cfg.headers {
                if let (Ok(name), Ok(value)) = (reqwest::header::HeaderName::from_bytes(k.as_bytes()), reqwest::header::HeaderValue::from_str(v)) {
                    headers.insert(name, value);
                }
            }
            let client = reqwest::Client::builder().default_headers(headers).build().map_err(|e| AppError::Mcp(e.to_string()))?;
            let url = cfg.url.clone().unwrap_or_default();
            let transport = rmcp::transport::StreamableHttpClientTransport::with_client(client, StreamableHttpClientTransportConfig::with_uri(url));
            ().serve(transport).await.map_err(|e| AppError::Mcp(e.to_string()))?
        }
        other => return Err(AppError::Invalid(format!("unknown transport {other}"))),
    };
    let tools = service.list_all_tools().await.map_err(|e| AppError::Mcp(format!("tools/list failed: {e}")))?;
    Ok(Connection { service, tools })
}

/// Flattens MCP content blocks into text for the LLM.
fn result_to_text(result: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(items) = result.get("content").and_then(Value::as_array) {
        for item in items {
            match item.get("type").and_then(Value::as_str) {
                Some("text") => parts.push(item["text"].as_str().unwrap_or("").to_string()),
                Some("resource") => {
                    let r = &item["resource"];
                    let uri = r["uri"].as_str().unwrap_or("");
                    let text = r["text"].as_str().unwrap_or("[binary resource]");
                    parts.push(format!("Resource {uri}:\n{text}"));
                }
                Some("resource_link") => parts.push(format!(
                    "Link: {} {}",
                    item["name"].as_str().or(item["title"].as_str()).unwrap_or(""),
                    item["uri"].as_str().unwrap_or("")
                )),
                Some("image") => parts.push("[image omitted]".into()),
                Some("audio") => parts.push("[audio omitted]".into()),
                _ => {}
            }
        }
    }
    if parts.is_empty() {
        if let Some(s) = result.get("structuredContent") {
            parts.push(s.to_string());
        }
    }
    parts.join("\n\n")
}

#[async_trait]
impl ToolProvider for McpManager {
    async fn available_tools(&self) -> Vec<ToolSpec> {
        let Ok(servers) = self.db.list_mcp_servers() else { return Vec::new() };
        let inner = self.inner.read().await;
        let mut taken = Vec::new();
        let mut out = Vec::new();
        for cfg in servers.iter().filter(|s| s.enabled) {
            let Some(conn) = inner.connections.get(&cfg.id) else { continue };
            let perms = self.db.tool_permissions(&cfg.id).unwrap_or_default();
            for t in &conn.tools {
                let category = tool_category(t);
                let permission = perms.get(t.name.as_ref()).copied().map(|p| clamp_permission(category, p)).unwrap_or_else(|| default_permission(category));
                if permission == Permission::Deny {
                    continue;
                }
                out.push(ToolSpec {
                    llm_name: llm_tool_name(&cfg.name, &t.name, &mut taken),
                    server_id: cfg.id.clone(),
                    server_name: cfg.name.clone(),
                    tool_name: t.name.to_string(),
                    description: t.description.as_deref().unwrap_or(&t.name).to_string(),
                    input_schema: Value::Object((*t.input_schema).clone()),
                    category,
                    permission,
                });
            }
        }
        out
    }

    async fn call_tool(&self, spec: &ToolSpec, args: Value) -> AppResult<ToolOutput> {
        let conn = self
            .inner
            .read()
            .await
            .connections
            .get(&spec.server_id)
            .cloned()
            .ok_or_else(|| AppError::Mcp(format!("{} is not connected", spec.server_name)))?;
        // Re-check permission at call time (it may have changed mid-turn).
        let perms = self.db.tool_permissions(&spec.server_id)?;
        if perms.get(&spec.tool_name) == Some(&Permission::Deny) {
            return Err(AppError::PermissionDenied(spec.tool_name.clone()));
        }
        let mut params = CallToolRequestParams::new(spec.tool_name.clone());
        if let Value::Object(map) = args {
            params = params.with_arguments(map);
        }
        let result = conn.service.call_tool(params).await.map_err(|e| {
            let msg = e.to_string();
            if conn.service.is_transport_closed() {
                AppError::Mcp(format!("{} disconnected: {msg}", spec.server_name))
            } else {
                AppError::Mcp(msg)
            }
        })?;
        let value = serde_json::to_value(&result)?;
        let text = result_to_text(&value);
        let sources = if matches!(spec.category, ToolCategory::Search | ToolCategory::Fetch) {
            sources::extract_sources(&text, value.get("structuredContent"))
        } else {
            Vec::new()
        };
        Ok(ToolOutput { text, is_error: result.is_error.unwrap_or(false), sources })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_are_sanitized_and_unique() {
        let mut taken = Vec::new();
        assert_eq!(llm_tool_name("Web Search", "searxng_web_search", &mut taken), "web_search__searxng_web_search");
        assert_eq!(llm_tool_name("Web Search", "searxng_web_search", &mut taken), "web_search__searxng_web_search2");
        assert!(llm_tool_name(&"x".repeat(100), "y", &mut taken).len() <= 64);
    }

    #[test]
    fn content_flattening() {
        let v = serde_json::json!({"content":[{"type":"text","text":"a"},{"type":"image","data":"..."},{"type":"resource","resource":{"uri":"https://x","text":"b"}}]});
        assert_eq!(result_to_text(&v), "a\n\n[image omitted]\n\nResource https://x:\nb");
    }
}
