//! Thin IPC layer: validates input, delegates to services.

pub mod app;
pub mod attachments;
pub mod chat;
pub mod mcp;
pub mod voice;

use crate::errors::AppError;

pub type CmdResult<T> = Result<T, AppError>;
