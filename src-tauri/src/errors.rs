//! Application error type.
//!
//! Every error carries a stable `code` that the UI maps to a friendly,
//! translated message, plus a technical `detail` that is logged locally and
//! shown only in Diagnostics / Developer Mode.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("LM Studio is unavailable: {0}")]
    LmStudioUnavailable(String),
    #[error("LM Studio error: {0}")]
    LmStudio(String),
    #[error("No suitable model is available in LM Studio")]
    NoModel,
    #[error("Request timed out: {0}")]
    Timeout(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Local speech recognition unavailable: {0}")]
    Stt(String),
    #[error("Local text-to-speech unavailable: {0}")]
    Tts(String),
    #[error("Audio device error: {0}")]
    Audio(String),
    #[error("MCP error: {0}")]
    Mcp(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Model download failed: {0}")]
    Download(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid input: {0}")]
    Invalid(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::LmStudioUnavailable(_) => "lmstudio_unavailable",
            AppError::LmStudio(_) => "lmstudio_error",
            AppError::NoModel => "no_model",
            AppError::Timeout(_) => "timeout",
            AppError::Database(_) => "database",
            AppError::Stt(_) => "stt_unavailable",
            AppError::Tts(_) => "tts_unavailable",
            AppError::Audio(_) => "audio",
            AppError::Mcp(_) => "mcp",
            AppError::PermissionDenied(_) => "permission_denied",
            AppError::Download(_) => "download",
            AppError::NotFound(_) => "not_found",
            AppError::Invalid(_) => "invalid",
            AppError::Io(_) => "io",
            AppError::Cancelled => "cancelled",
            AppError::Other(_) => "other",
        }
    }
}

#[derive(Serialize)]
struct WireError<'a> {
    code: &'a str,
    detail: String,
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        WireError {
            code: self.code(),
            detail: self.to_string(),
        }
        .serialize(serializer)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Database(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Invalid(e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

/// Map a reqwest error from an LM Studio call to a user-meaningful variant.
pub fn from_lmstudio_http(e: reqwest::Error) -> AppError {
    if e.is_timeout() {
        AppError::Timeout(e.to_string())
    } else if e.is_connect() || e.is_request() {
        AppError::LmStudioUnavailable(e.to_string())
    } else {
        AppError::LmStudio(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
