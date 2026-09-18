//! Composer attachments: ingesting picked, dropped or pasted files and handing
//! their stored copies back to the UI.

use super::CmdResult;
use crate::errors::AppError;
use crate::services::attachments::{base64_decode, Attachment};
use crate::state::AppState;
use std::path::PathBuf;
use tauri::State;

/// Result of attaching several files at once: whatever succeeded, plus a
/// per-file message for whatever did not, so one bad file cannot lose the rest.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachResult {
    pub attachments: Vec<Attachment>,
    pub failures: Vec<String>,
}

#[tauri::command]
pub fn attach_files(state: State<'_, AppState>, paths: Vec<String>) -> CmdResult<AttachResult> {
    let mut out = AttachResult::default();
    for path in paths {
        match state.attachments.ingest_path(&PathBuf::from(&path)) {
            Ok(a) => out.attachments.push(a),
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "attachment rejected");
                out.failures.push(e.to_string());
            }
        }
    }
    Ok(out)
}

/// Attaches bytes that never existed as a file, e.g. an image pasted from the
/// clipboard. `data` is base64, optionally still wrapped in a `data:` URL.
#[tauri::command]
pub fn attach_bytes(state: State<'_, AppState>, name: String, mime: Option<String>, data: String) -> CmdResult<Attachment> {
    let bytes = base64_decode(&data).map_err(AppError::Invalid)?;
    state.attachments.ingest_bytes(&name, mime.as_deref(), bytes)
}

/// Attaches a long block of text as a file, the way pasting a large document
/// into the composer does.
#[tauri::command]
pub fn attach_text(state: State<'_, AppState>, name: String, text: String) -> CmdResult<Attachment> {
    state.attachments.ingest_text(&name, &text)
}

#[tauri::command]
pub fn attach_remove(state: State<'_, AppState>, id: String) {
    state.attachments.discard(&id);
}

/// `data:` URL for an attachment, used by the UI to show image previews for
/// messages loaded back from history.
#[tauri::command]
pub fn attachment_data_url(state: State<'_, AppState>, id: String, mime: String) -> CmdResult<String> {
    let bytes = state.attachments.bytes(&id)?;
    Ok(format!("data:{};base64,{}", mime, crate::services::attachments::base64_encode(&bytes)))
}

/// Extracted text of an attachment, for the "show contents" preview.
#[tauri::command]
pub fn attachment_text(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    state
        .attachments
        .text(&id)
        .ok_or_else(|| AppError::NotFound(format!("attachment {id}")))
}
