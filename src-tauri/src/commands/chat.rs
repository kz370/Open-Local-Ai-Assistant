use super::CmdResult;
use crate::database::conversations::{Conversation, Message, SearchHit};
use crate::database::export::ExportFormat;
use crate::errors::AppError;
use crate::services::chat::{ChatEvent, SendInput};
use crate::state::AppState;
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::State;

#[tauri::command]
pub async fn chat_send(state: State<'_, AppState>, input: SendInput, on_event: Channel<ChatEvent>) -> CmdResult<()> {
    let chat = state.chat.clone();
    let emit = Arc::new(move |ev: ChatEvent| {
        let _ = on_event.send(ev);
    });
    // Errors are also delivered as events; the command result only signals completion.
    let _ = chat.send(input, emit).await;
    Ok(())
}

#[tauri::command]
pub fn chat_stop(state: State<'_, AppState>, turn_id: String) {
    state.chat.stop(&turn_id);
}

#[tauri::command]
pub fn chat_confirm_tool(state: State<'_, AppState>, call_id: String, approved: bool) -> bool {
    state.chat.confirm_tool(&call_id, approved)
}

#[tauri::command]
pub fn conv_list(state: State<'_, AppState>, limit: Option<u32>, offset: Option<u32>) -> CmdResult<Vec<Conversation>> {
    state.db.list_conversations(limit.unwrap_or(100).min(1000), offset.unwrap_or(0))
}

#[tauri::command]
pub fn conv_search(state: State<'_, AppState>, query: String) -> CmdResult<Vec<SearchHit>> {
    state.db.search_conversations(&query, 100)
}

#[tauri::command]
pub fn conv_get(state: State<'_, AppState>, id: String) -> CmdResult<(Conversation, Vec<Message>)> {
    let c = state.db.get_conversation(&id)?;
    let msgs = state.db.list_messages(&id)?.into_iter().filter(|m| m.role == "user" || m.role == "assistant").collect();
    Ok((c, msgs))
}

#[tauri::command]
pub fn conv_rename(state: State<'_, AppState>, id: String, title: String) -> CmdResult<()> {
    state.db.rename_conversation(&id, &title)
}

#[tauri::command]
pub fn conv_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.db.delete_conversation(&id)?;
    state.settings.update(|s| {
        if s.last_conversation_id.as_deref() == Some(id.as_str()) {
            s.last_conversation_id = None;
        }
    })?;
    Ok(())
}

/// Deletes the whole conversation history and the attachment files it used.
#[tauri::command]
pub fn conv_clear_all(state: State<'_, AppState>) -> CmdResult<usize> {
    let removed = state.db.delete_all_conversations()?;
    state.settings.update(|s| s.last_conversation_id = None)?;
    state.attachments.gc(&state.db.attachment_ids()?);
    Ok(removed)
}

#[tauri::command]
pub fn conv_set_last(state: State<'_, AppState>, id: Option<String>) -> CmdResult<()> {
    state.settings.update(|s| s.last_conversation_id = id)?;
    Ok(())
}

#[tauri::command]
pub fn conv_export(state: State<'_, AppState>, ids: Vec<String>, format: ExportFormat, path: String) -> CmdResult<()> {
    if ids.is_empty() {
        return Err(AppError::Invalid("nothing to export".into()));
    }
    let content = state.db.export_conversations(&ids, format)?;
    std::fs::write(path, content)?;
    Ok(())
}

#[tauri::command]
pub fn conv_import(state: State<'_, AppState>, path: String) -> CmdResult<Vec<String>> {
    let meta = std::fs::metadata(&path)?;
    if meta.len() > 200 * 1024 * 1024 {
        return Err(AppError::Invalid("file is too large".into()));
    }
    let json = std::fs::read_to_string(path)?;
    state.db.import_conversations(&json)
}
