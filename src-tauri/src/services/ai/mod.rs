//! AI inference. LM Studio is the only backend; the trait exists purely to keep
//! the chat orchestrator testable and the boundary clean.

pub mod lmstudio;
pub mod model_selector;
pub mod sse;

use crate::errors::AppResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

pub use sse::ToolCall;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    /// "llm" | "vlm" | "embedding" | "unknown"
    pub kind: String,
    pub size_bytes: Option<u64>,
    pub params: Option<String>,
    pub quantization: Option<String>,
    pub bits_per_weight: Option<f32>,
    pub max_context_length: Option<u32>,
    pub loaded: bool,
    pub loaded_context_length: Option<u32>,
    pub tool_use: bool,
    pub vision: bool,
    pub reasoning: bool,
}

impl ModelInfo {
    pub fn is_chat_model(&self) -> bool {
        self.kind != "embedding"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub connected: bool,
    pub server_url: String,
    pub model_count: usize,
    pub latency_ms: u64,
    /// "native-v1" | "native-v0" | "openai" — which discovery API answered.
    pub api: String,
}

/// A message in OpenAI chat format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn text(role: &str, content: impl Into<String>) -> Self {
        Self { role: role.into(), content: Some(content.into()), tool_calls: None, tool_call_id: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub temperature: f32,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamChunk {
    Content(String),
    Reasoning(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatCompletion {
    pub content: String,
    pub reasoning: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
}

#[async_trait]
pub trait AiService: Send + Sync {
    async fn test_connection(&self) -> AppResult<ConnectionStatus>;
    async fn list_models(&self) -> AppResult<Vec<ModelInfo>>;
    async fn load_model(&self, model_id: &str, context_length: Option<u32>) -> AppResult<()>;
    async fn chat(
        &self,
        req: ChatRequest,
        cancel: CancellationToken,
        on_chunk: &mut (dyn FnMut(StreamChunk) + Send),
    ) -> AppResult<ChatCompletion>;
}
