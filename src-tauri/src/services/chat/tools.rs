//! Tool abstractions used by the chat orchestrator (implemented by the MCP manager).

use crate::errors::AppResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    /// Run without asking.
    Allow,
    /// Ask the user each time.
    Ask,
    /// Never offered to the model.
    Deny,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolCategory {
    Search,
    Fetch,
    Read,
    Write,
    Execute,
    Other,
}

impl ToolCategory {
    /// Categories that may change state outside the app.
    pub fn is_sensitive(self) -> bool {
        matches!(self, ToolCategory::Write | ToolCategory::Execute | ToolCategory::Other)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpec {
    /// Name exposed to the LLM (unique, sanitized).
    pub llm_name: String,
    pub server_id: String,
    pub server_name: String,
    pub tool_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub category: ToolCategory,
    pub permission: Permission,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ToolOutput {
    /// Text handed back to the LLM.
    pub text: String,
    pub is_error: bool,
    pub sources: Vec<Source>,
}

#[async_trait]
pub trait ToolProvider: Send + Sync {
    /// Tools currently available (connected, enabled, not denied).
    async fn available_tools(&self) -> Vec<ToolSpec>;
    async fn call_tool(&self, spec: &ToolSpec, args: serde_json::Value) -> AppResult<ToolOutput>;
}

/// No-op provider (tests, or when MCP is unavailable).
pub struct NoTools;

#[async_trait]
impl ToolProvider for NoTools {
    async fn available_tools(&self) -> Vec<ToolSpec> {
        Vec::new()
    }
    async fn call_tool(&self, spec: &ToolSpec, _args: serde_json::Value) -> AppResult<ToolOutput> {
        Err(crate::errors::AppError::Mcp(format!("tool {} unavailable", spec.llm_name)))
    }
}

/// Receives assistant text for speech while it streams.
pub trait SpeechSink: Send + Sync {
    fn begin(&self, turn_id: &str);
    fn push_text(&self, turn_id: &str, text: &str);
    fn finish(&self, turn_id: &str);
    fn cancel(&self, turn_id: &str);
}

pub struct NoSpeech;

impl SpeechSink for NoSpeech {
    fn begin(&self, _: &str) {}
    fn push_text(&self, _: &str, _: &str) {}
    fn finish(&self, _: &str) {}
    fn cancel(&self, _: &str) {}
}
