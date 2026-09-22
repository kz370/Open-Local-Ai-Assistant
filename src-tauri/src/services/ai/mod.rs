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
    /// Hosted providers only: the model costs nothing to call.
    pub free: bool,
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

/// One part of a multimodal message, in OpenAI content-part format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageUrl {
    pub url: String,
}

/// Message body: plain text, or parts when images are attached.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    /// The readable text of this body; image parts contribute nothing.
    pub fn as_text(&self) -> String {
        match self {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(text.as_str()),
                    ContentPart::ImageUrl { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// Serialized size, used when trimming history to the context window.
    pub fn len(&self) -> usize {
        match self {
            MessageContent::Text(t) => t.len(),
            MessageContent::Parts(parts) => parts
                .iter()
                .map(|p| match p {
                    ContentPart::Text { text } => text.len(),
                    ContentPart::ImageUrl { image_url } => image_url.url.len(),
                })
                .sum(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            MessageContent::Text(t) => t.is_empty(),
            MessageContent::Parts(parts) => parts.is_empty(),
        }
    }
}

/// A message in OpenAI chat format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn text(role: &str, content: impl Into<String>) -> Self {
        Self { role: role.into(), content: Some(MessageContent::Text(content.into())), tool_calls: None, tool_call_id: None }
    }

    pub fn parts(role: &str, parts: Vec<ContentPart>) -> Self {
        Self { role: role.into(), content: Some(MessageContent::Parts(parts)), tool_calls: None, tool_call_id: None }
    }

    /// The message's text, or an empty string when it carries none.
    pub fn content_text(&self) -> String {
        self.content.as_ref().map(MessageContent::as_text).unwrap_or_default()
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
    /// Token counts reported by the server, when it reports them.
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    /// Streamed content/reasoning chunks; roughly one token each, used when
    /// the server reports no usage.
    pub chunks: u32,
    /// From sending the request to the first streamed token.
    pub first_token_ms: Option<u64>,
    /// From the first streamed token to the end of the reply.
    pub generation_ms: Option<u64>,
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
