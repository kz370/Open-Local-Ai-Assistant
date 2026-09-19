use super::CmdResult;
use crate::errors::AppError;
use crate::services::chat::tools::Permission;
use crate::services::mcp::config::{lmstudio_mcp_json_path, parse_mcp_json, ImportCandidate, McpServerConfig};
use crate::services::mcp::ServerStatus;
use crate::services::search::PublicInstance;
use crate::state::AppState;
use tauri::{State, WebviewWindow};

#[tauri::command]
pub async fn mcp_list(state: State<'_, AppState>) -> CmdResult<Vec<ServerStatus>> {
    state.mcp.statuses().await
}

/// Saves a server. New servers are always stored disabled; the user enables
/// them explicitly afterwards.
#[tauri::command]
pub async fn mcp_save(state: State<'_, AppState>, config: McpServerConfig) -> CmdResult<McpServerConfig> {
    let mut c = config;
    let is_new = c.id.is_empty();
    if is_new {
        c.enabled = false;
        c.source = "user".into();
    } else {
        c.enabled = state.db.get_mcp_server(&c.id)?.enabled;
    }
    let saved = state.db.save_mcp_server(&c)?;
    if saved.enabled {
        let _ = state.mcp.connect(&saved.id).await;
    }
    Ok(saved)
}

#[tauri::command]
pub async fn mcp_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.mcp.disconnect(&id).await;
    state.db.delete_mcp_server(&id)
}

#[tauri::command]
pub async fn mcp_set_enabled(state: State<'_, AppState>, id: String, enabled: bool) -> CmdResult<()> {
    state.mcp.set_enabled(&id, enabled).await
}

#[tauri::command]
pub async fn mcp_reconnect(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let _ = state.mcp.connect(&id).await;
    Ok(())
}

#[tauri::command]
pub async fn mcp_set_permission(state: State<'_, AppState>, server_id: String, tool: String, permission: Permission) -> CmdResult<Permission> {
    state.mcp.set_permission(&server_id, &tool, permission).await
}

/// `permission: None` resets every tool of the server to its default.
#[tauri::command]
pub async fn mcp_set_all_permissions(state: State<'_, AppState>, server_id: String, permission: Option<Permission>) -> CmdResult<()> {
    state.mcp.set_all_permissions(&server_id, permission).await
}

#[tauri::command]
pub fn mcp_safe_mode(state: State<'_, AppState>) -> bool {
    state.mcp.safe_mode()
}

/// Turning safe mode off requires Windows Hello verification; turning it on
/// never does. It resets to on at every launch.
#[tauri::command]
pub async fn mcp_set_safe_mode(window: WebviewWindow, state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    if !enabled && state.mcp.safe_mode() {
        crate::desktop::verify::verify_user(&window, "Confirm it's you to turn off MCP safe mode").await?;
    }
    state.mcp.set_safe_mode(enabled);
    Ok(())
}

#[tauri::command]
pub fn mcp_import_preview(state: State<'_, AppState>) -> CmdResult<Vec<ImportCandidate>> {
    let Some(path) = lmstudio_mcp_json_path() else { return Ok(vec![]) };
    let existing: Vec<String> = state.db.list_mcp_servers()?.into_iter().map(|s| s.name.to_lowercase()).collect();
    Ok(parse_mcp_json(&std::fs::read_to_string(path)?)?
        .into_iter()
        .map(|c| ImportCandidate {
            already_configured: existing.contains(&c.name.to_lowercase()),
            env_keys: c.env.keys().cloned().collect(),
            name: c.name,
            transport: c.transport,
            command: c.command,
            args: c.args,
            url: c.url,
        })
        .collect())
}

/// Imports the selected servers from LM Studio's mcp.json — always disabled.
#[tauri::command]
pub fn mcp_import(state: State<'_, AppState>, names: Vec<String>) -> CmdResult<usize> {
    let path = lmstudio_mcp_json_path().ok_or_else(|| AppError::NotFound("LM Studio mcp.json".into()))?;
    let existing: Vec<String> = state.db.list_mcp_servers()?.into_iter().map(|s| s.name.to_lowercase()).collect();
    let mut count = 0;
    for mut c in parse_mcp_json(&std::fs::read_to_string(path)?)? {
        if !names.contains(&c.name) || existing.contains(&c.name.to_lowercase()) {
            continue;
        }
        c.enabled = false;
        c.source = "lmstudio".into();
        if c.validate().is_ok() {
            state.db.save_mcp_server(&c)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Public SearXNG instances from searx.space; `refresh` reloads the list.
#[tauri::command]
pub async fn search_public_instances(state: State<'_, AppState>, refresh: bool) -> CmdResult<Vec<PublicInstance>> {
    state.web_search.public_instances(refresh).await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchTestResult {
    /// The engine that answered, e.g. "SearXNG (https://searx.be/)".
    pub engine: String,
    pub count: usize,
    pub first_title: Option<String>,
}

/// Runs one real search with the saved settings, skipping the cache.
#[tauri::command]
pub async fn search_test(state: State<'_, AppState>) -> CmdResult<SearchTestResult> {
    let (results, engine) = state.web_search.search_uncached("open source search engine").await?;
    Ok(SearchTestResult { engine, count: results.len(), first_title: results.first().map(|r| r.title.clone()) })
}
