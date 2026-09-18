//! Chat turn orchestration:
//! user text -> language detection -> history -> LM Studio (streaming) ->
//! optional MCP tool rounds (with permissions) -> persisted answer -> speech.

use super::attach;
use super::freshness::needs_fresh_info;
use super::prompt::{build_system_prompt, freshness_hint, PromptContext};
use super::resolver::ModelResolver;
use super::think::ThinkFilter;
use super::tools::{Permission, Source, SpeechSink, ToolCategory, ToolProvider, ToolSpec};
use crate::database::conversations::{new_id, now, Message};
use crate::database::Db;
use crate::errors::{AppError, AppResult};
use crate::services::ai::{AiService, ChatMessage, ChatRequest, FunctionDefinition, MessageContent, StreamChunk, ToolCall, ToolDefinition};
use crate::services::attachments::AttachmentStore;
use crate::services::language::{detect, Lang};
use crate::settings::SettingsStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

const MAX_TOOL_ROUNDS: usize = 6;
const TOOL_RESULT_LIMIT: usize = 16_000;
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(180);
const TOOL_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendInput {
    pub turn_id: String,
    pub conversation_id: Option<String>,
    pub text: String,
    /// Language reported by speech recognition, if the text was spoken.
    pub spoken_language: Option<String>,
    #[serde(default)]
    pub voice: bool,
    /// Ids of attachments staged by the composer for this turn.
    #[serde(default)]
    pub attachment_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ChatEvent {
    Started { turn_id: String, conversation_id: String, user_message: Message, model: String, language: String, new_conversation: bool },
    Delta { turn_id: String, text: String },
    Reasoning { turn_id: String, text: String },
    ToolStarted { turn_id: String, call_id: String, server_name: String, tool_name: String, category: ToolCategory, args: Value },
    ToolAwaitingConfirmation { turn_id: String, call_id: String, server_name: String, tool_name: String, category: ToolCategory, args: Value },
    ToolFinished { turn_id: String, call_id: String, ok: bool, duration_ms: u64, result_preview: String, sources: Vec<Source>, denied: bool },
    Done { turn_id: String, message: Message },
    Error { turn_id: String, code: String, detail: String, partial_message: Option<Message> },
}

pub type Emit = Arc<dyn Fn(ChatEvent) + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivityRecord {
    call_id: String,
    server_name: String,
    tool_name: String,
    category: ToolCategory,
    args: Value,
    ok: bool,
    denied: bool,
    duration_ms: u64,
    result_preview: String,
}

pub struct ChatEngine {
    db: Arc<Db>,
    settings: Arc<SettingsStore>,
    ai: Arc<dyn AiService>,
    resolver: Arc<ModelResolver>,
    tools: Arc<dyn ToolProvider>,
    speech: Arc<dyn SpeechSink>,
    attachments: Arc<AttachmentStore>,
    active: Mutex<HashMap<String, CancellationToken>>,
    confirmations: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl ChatEngine {
    pub fn new(
        db: Arc<Db>,
        settings: Arc<SettingsStore>,
        ai: Arc<dyn AiService>,
        resolver: Arc<ModelResolver>,
        tools: Arc<dyn ToolProvider>,
        speech: Arc<dyn SpeechSink>,
        attachments: Arc<AttachmentStore>,
    ) -> Self {
        Self {
            db,
            settings,
            ai,
            resolver,
            tools,
            speech,
            attachments,
            active: Mutex::new(HashMap::new()),
            confirmations: Mutex::new(HashMap::new()),
        }
    }

    pub fn stop(&self, turn_id: &str) {
        if let Some(t) = self.active.lock().unwrap_or_else(|p| p.into_inner()).get(turn_id) {
            t.cancel();
        }
        self.speech.cancel(turn_id);
    }

    pub fn stop_all(&self) {
        for (id, t) in self.active.lock().unwrap_or_else(|p| p.into_inner()).iter() {
            t.cancel();
            self.speech.cancel(id);
        }
    }

    pub fn is_busy(&self) -> bool {
        !self.active.lock().unwrap_or_else(|p| p.into_inner()).is_empty()
    }

    pub fn confirm_tool(&self, call_id: &str, approved: bool) -> bool {
        match self.confirmations.lock().unwrap_or_else(|p| p.into_inner()).remove(call_id) {
            Some(tx) => tx.send(approved).is_ok(),
            None => false,
        }
    }

    pub async fn send(&self, input: SendInput, emit: Emit) -> AppResult<()> {
        let text = input.text.trim().to_string();
        if text.is_empty() && input.attachment_ids.is_empty() {
            return Err(AppError::Invalid("message is empty".into()));
        }
        let token = CancellationToken::new();
        self.active.lock().unwrap_or_else(|p| p.into_inner()).insert(input.turn_id.clone(), token.clone());
        let result = self.run_turn(&input, text, token, emit.clone()).await;
        self.active.lock().unwrap_or_else(|p| p.into_inner()).remove(&input.turn_id);
        result
    }

    async fn run_turn(&self, input: &SendInput, text: String, cancel: CancellationToken, emit: Emit) -> AppResult<()> {
        let turn_id = input.turn_id.clone();
        let settings = self.settings.get();

        // Attachments staged by the composer become part of this user message.
        let attachments = self.attachments.claim(&input.attachment_ids);
        let title_source = if text.is_empty() {
            attachments.first().map(|a| a.name.clone()).unwrap_or_default()
        } else {
            text.clone()
        };

        // Conversation
        let (conversation, new_conversation) = match input.conversation_id.as_deref() {
            Some(id) => match self.db.get_conversation(id) {
                Ok(c) => (c, false),
                Err(AppError::NotFound(_)) => (self.db.create_conversation(&title_from(&title_source), None)?, true),
                Err(e) => return Err(e),
            },
            None => (self.db.create_conversation(&title_from(&title_source), None)?, true),
        };
        if new_conversation {
            let _ = self.settings.update(|s| s.last_conversation_id = Some(conversation.id.clone()));
        }

        // Language
        let conv_lang = conversation.language.as_deref().and_then(Lang::from_code);
        let detected = input
            .spoken_language
            .as_deref()
            .and_then(Lang::from_code)
            .or_else(|| detect(&text).filter(|d| d.confidence >= 0.4).map(|d| d.lang))
            .or(conv_lang);
        let forced = Lang::from_code(&settings.language.response_language);
        let response_lang = forced.or(detected).unwrap_or(Lang::En);

        let user_message = Message {
            id: new_id(),
            conversation_id: conversation.id.clone(),
            role: "user".into(),
            content: text.clone(),
            language: detected.map(|l| l.code().to_string()),
            reasoning: None,
            sources: None,
            tool_activity: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: if attachments.is_empty() { None } else { Some(serde_json::to_value(&attachments)?) },
            created_at: now(),
        };
        self.db.insert_message(&user_message)?;
        if !attachments.is_empty() {
            tracing::info!(
                conversation = %conversation.id,
                files = attachments.len(),
                bytes = attachments.iter().map(|a| a.size_bytes).sum::<u64>(),
                "user message attachments"
            );
        }
        // Conversation content is only logged when the user explicitly opted in.
        if settings.general.log_conversation_content {
            tracing::info!(conversation = %conversation.id, language = ?detected, content = %text, "user message");
        } else {
            tracing::info!(conversation = %conversation.id, language = ?detected, chars = text.chars().count(), "user message");
        }

        let emit_error = |e: &AppError, partial: Option<Message>| {
            emit(ChatEvent::Error {
                turn_id: turn_id.clone(),
                code: e.code().into(),
                detail: e.to_string(),
                partial_message: partial,
            })
        };

        // Model
        let model = match self.resolver.resolve(&settings.ai).await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(error = %e, "model resolution failed");
                emit(ChatEvent::Started {
                    turn_id: turn_id.clone(),
                    conversation_id: conversation.id.clone(),
                    user_message: user_message.clone(),
                    model: String::new(),
                    language: response_lang.code().into(),
                    new_conversation,
                });
                emit_error(&e, None);
                return Err(e);
            }
        };
        self.db.touch_conversation(&conversation.id, Some(&model.id), Some(response_lang.code()))?;

        emit(ChatEvent::Started {
            turn_id: turn_id.clone(),
            conversation_id: conversation.id.clone(),
            user_message: user_message.clone(),
            model: model.id.clone(),
            language: response_lang.code().into(),
            new_conversation,
        });

        // Tools & prompt
        let specs = self.tools.available_tools().await;
        let tool_map: HashMap<String, ToolSpec> = specs.iter().map(|s| (s.llm_name.clone(), s.clone())).collect();
        let has_web_tool = specs.iter().any(|s| s.category == ToolCategory::Search);
        let tool_desc: Vec<(String, String)> = specs.iter().map(|s| (s.llm_name.clone(), s.description.clone())).collect();
        // Whether this reply will be spoken aloud: voice input always replies by
        // voice, and "speak responses" also reads out replies to typed messages.
        let speak = input.voice || settings.tts.speak_responses;
        let mut system = build_system_prompt(&PromptContext {
            date: chrono::Local::now(),
            forced_language: forced,
            detected_language: detected,
            tools: &tool_desc,
            has_web_tool,
            voice_mode: speak,
            assistant_name: &settings.general.assistant_name,
            custom_prompt: &settings.ai.system_prompt,
        });
        if needs_fresh_info(&text) {
            system.push_str("\n## Note for this turn\n");
            system.push_str(&freshness_hint(has_web_tool));
            system.push('\n');
        }

        let ctx_tokens = settings
            .ai
            .context_length
            .or(model.info.as_ref().and_then(|i| i.loaded_context_length))
            .unwrap_or(8192);
        let history = self.db.list_messages(&conversation.id)?;
        let vision = model.info.as_ref().map(|i| i.vision).unwrap_or(false);
        let mut messages = vec![ChatMessage::text("system", system.clone())];
        messages.extend(build_history(&history, ctx_tokens, system.len(), &self.attachments, vision));

        let tool_defs: Vec<ToolDefinition> = specs
            .iter()
            .map(|s| ToolDefinition {
                kind: "function".into(),
                function: FunctionDefinition {
                    name: s.llm_name.clone(),
                    description: s.description.chars().take(1024).collect(),
                    parameters: normalize_schema(&s.input_schema),
                },
            })
            .collect();

        if speak {
            self.speech.begin(&turn_id);
        }

        let mut full_content = String::new();
        let mut full_reasoning = String::new();
        let mut sources: Vec<Source> = Vec::new();
        let mut activity: Vec<ActivityRecord> = Vec::new();

        for round in 0..=MAX_TOOL_ROUNDS {
            let last_round = round == MAX_TOOL_ROUNDS;
            let req = ChatRequest {
                model: model.id.clone(),
                messages: messages.clone(),
                tools: if last_round { vec![] } else { tool_defs.clone() },
                temperature: settings.ai.temperature,
                max_tokens: settings.ai.max_tokens,
                stream: settings.ai.streaming,
            };
            let mut filter = ThinkFilter::default();
            let mut round_visible = String::new();
            let mut on_chunk = |chunk: StreamChunk| match chunk {
                StreamChunk::Content(c) => {
                    let (vis, rea) = filter.push(&c);
                    if !rea.is_empty() {
                        full_reasoning.push_str(&rea);
                        emit(ChatEvent::Reasoning { turn_id: turn_id.clone(), text: rea });
                    }
                    if !vis.is_empty() {
                        round_visible.push_str(&vis);
                        if speak {
                            self.speech.push_text(&turn_id, &vis);
                        }
                        emit(ChatEvent::Delta { turn_id: turn_id.clone(), text: vis });
                    }
                }
                StreamChunk::Reasoning(r) => {
                    full_reasoning.push_str(&r);
                    emit(ChatEvent::Reasoning { turn_id: turn_id.clone(), text: r });
                }
            };
            let result = self.ai.chat(req, cancel.clone(), &mut on_chunk).await;
            let (vis, rea) = filter.finish();
            if !rea.is_empty() {
                full_reasoning.push_str(&rea);
            }
            if !vis.is_empty() {
                round_visible.push_str(&vis);
                if speak {
                    self.speech.push_text(&turn_id, &vis);
                }
                emit(ChatEvent::Delta { turn_id: turn_id.clone(), text: vis });
            }

            let completion = match result {
                Ok(c) => c,
                Err(e) => {
                    if speak {
                        self.speech.cancel(&turn_id);
                    }
                    full_content.push_str(&round_visible);
                    let partial = if full_content.trim().is_empty() {
                        None
                    } else {
                        let m = self.assistant_message(&conversation.id, &full_content, &full_reasoning, &sources, &activity, response_lang)?;
                        self.db.insert_message(&m)?;
                        Some(m)
                    };
                    if !matches!(e, AppError::Cancelled) {
                        tracing::warn!(error = %e, code = e.code(), "chat turn failed");
                    }
                    emit_error(&e, partial);
                    return if matches!(e, AppError::Cancelled) { Ok(()) } else { Err(e) };
                }
            };

            if completion.tool_calls.is_empty() || last_round {
                full_content.push_str(&round_visible);
                break;
            }

            // Tool round: persist the assistant tool-call message.
            let round_text = round_visible.clone();
            full_content.push_str(&round_visible);
            if !full_content.is_empty() && !full_content.ends_with('\n') {
                full_content.push_str("\n\n");
            }
            let calls = completion.tool_calls.clone();
            messages.push(ChatMessage {
                role: "assistant".into(),
                content: if round_text.is_empty() { None } else { Some(MessageContent::Text(round_text.clone())) },
                tool_calls: Some(calls.clone()),
                tool_call_id: None,
            });
            self.db.insert_message(&Message {
                id: new_id(),
                conversation_id: conversation.id.clone(),
                role: "assistant_tool_calls".into(),
                content: round_text,
                language: None,
                reasoning: None,
                sources: None,
                tool_activity: None,
                tool_calls: Some(serde_json::to_value(&calls)?),
                tool_call_id: None,
                attachments: None,
                created_at: now(),
            })?;

            for call in calls {
                if cancel.is_cancelled() {
                    break;
                }
                let (record, output_text, new_sources) = self.execute_call(&turn_id, &call, &tool_map, &cancel, &emit).await;
                for s in new_sources {
                    if !sources.iter().any(|x| x.url == s.url) {
                        sources.push(s);
                    }
                }
                activity.push(record);
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: Some(MessageContent::Text(output_text.clone())),
                    tool_calls: None,
                    tool_call_id: Some(call.id.clone()),
                });
                self.db.insert_message(&Message {
                    id: new_id(),
                    conversation_id: conversation.id.clone(),
                    role: "tool".into(),
                    content: output_text,
                    language: None,
                    reasoning: None,
                    sources: None,
                    tool_activity: None,
                    tool_calls: None,
                    tool_call_id: Some(call.id.clone()),
                    attachments: None,
                    created_at: now(),
                })?;
            }
            if cancel.is_cancelled() {
                if speak {
                    self.speech.cancel(&turn_id);
                }
                emit_error(&AppError::Cancelled, None);
                return Ok(());
            }
        }

        if speak {
            self.speech.finish(&turn_id);
        }

        let final_message = self.assistant_message(&conversation.id, full_content.trim(), &full_reasoning, &sources, &activity, response_lang)?;
        self.db.insert_message(&final_message)?;
        self.db.touch_conversation(&conversation.id, None, None)?;
        emit(ChatEvent::Done { turn_id, message: final_message });
        Ok(())
    }

    fn assistant_message(
        &self,
        conversation_id: &str,
        content: &str,
        reasoning: &str,
        sources: &[Source],
        activity: &[ActivityRecord],
        fallback_lang: Lang,
    ) -> AppResult<Message> {
        let lang = detect(content).map(|d| d.lang).unwrap_or(fallback_lang);
        Ok(Message {
            id: new_id(),
            conversation_id: conversation_id.into(),
            role: "assistant".into(),
            content: content.into(),
            language: Some(lang.code().into()),
            reasoning: if reasoning.trim().is_empty() { None } else { Some(reasoning.trim().into()) },
            sources: if sources.is_empty() { None } else { Some(serde_json::to_value(sources)?) },
            tool_activity: if activity.is_empty() { None } else { Some(serde_json::to_value(activity)?) },
            tool_calls: None,
            tool_call_id: None,
            attachments: None,
            created_at: now(),
        })
    }

    async fn execute_call(
        &self,
        turn_id: &str,
        call: &ToolCall,
        tool_map: &HashMap<String, ToolSpec>,
        cancel: &CancellationToken,
        emit: &Emit,
    ) -> (ActivityRecord, String, Vec<Source>) {
        let started = Instant::now();
        let args: Value = serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);
        let Some(spec) = tool_map.get(&call.function.name) else {
            let msg = format!("Error: tool '{}' does not exist. Available tools: {}", call.function.name, tool_map.keys().cloned().collect::<Vec<_>>().join(", "));
            return (record(call, None, &args, false, false, 0, &msg), msg, vec![]);
        };
        if !args.is_object() {
            let msg = "Error: tool arguments must be a JSON object.".to_string();
            return (record(call, Some(spec), &args, false, false, 0, &msg), msg, vec![]);
        }

        let needs_confirmation = spec.permission == Permission::Ask;
        if spec.permission == Permission::Deny {
            let msg = "Error: the user has not permitted this tool.".to_string();
            return (record(call, Some(spec), &args, false, true, 0, &msg), msg, vec![]);
        }
        if needs_confirmation {
            let (tx, rx) = oneshot::channel();
            self.confirmations.lock().unwrap_or_else(|p| p.into_inner()).insert(call.id.clone(), tx);
            emit(ChatEvent::ToolAwaitingConfirmation {
                turn_id: turn_id.into(),
                call_id: call.id.clone(),
                server_name: spec.server_name.clone(),
                tool_name: spec.tool_name.clone(),
                category: spec.category,
                args: args.clone(),
            });
            let approved = tokio::select! {
                r = rx => r.unwrap_or(false),
                _ = tokio::time::sleep(CONFIRM_TIMEOUT) => false,
                _ = cancel.cancelled() => false,
            };
            self.confirmations.lock().unwrap_or_else(|p| p.into_inner()).remove(&call.id);
            if !approved {
                let msg = "The user declined to run this tool. Do not retry it; answer without it or explain what you would need.".to_string();
                emit(ChatEvent::ToolFinished {
                    turn_id: turn_id.into(),
                    call_id: call.id.clone(),
                    ok: false,
                    duration_ms: 0,
                    result_preview: String::new(),
                    sources: vec![],
                    denied: true,
                });
                return (record(call, Some(spec), &args, false, true, 0, &msg), msg, vec![]);
            }
        }

        emit(ChatEvent::ToolStarted {
            turn_id: turn_id.into(),
            call_id: call.id.clone(),
            server_name: spec.server_name.clone(),
            tool_name: spec.tool_name.clone(),
            category: spec.category,
            args: args.clone(),
        });

        let outcome = tokio::select! {
            r = tokio::time::timeout(TOOL_TIMEOUT, self.tools.call_tool(spec, args.clone())) => match r {
                Ok(r) => r,
                Err(_) => Err(AppError::Timeout(format!("tool {} timed out", spec.tool_name))),
            },
            _ = cancel.cancelled() => Err(AppError::Cancelled),
        };
        let duration_ms = started.elapsed().as_millis() as u64;
        let (ok, text, sources) = match outcome {
            Ok(out) => (!out.is_error, truncate_chars(&out.text, TOOL_RESULT_LIMIT), out.sources),
            Err(e) => {
                tracing::warn!(server = %spec.server_name, tool = %spec.tool_name, error = %e, "tool call failed");
                (false, format!("Error: the tool failed ({}). Tell the user the tool is unavailable; do not invent its results.", e), vec![])
            }
        };
        let preview = truncate_chars(&text, 2000);
        emit(ChatEvent::ToolFinished {
            turn_id: turn_id.into(),
            call_id: call.id.clone(),
            ok,
            duration_ms,
            result_preview: preview.clone(),
            sources: sources.clone(),
            denied: false,
        });
        (record(call, Some(spec), &args, ok, false, duration_ms, &preview), text, sources)
    }
}

fn record(call: &ToolCall, spec: Option<&ToolSpec>, args: &Value, ok: bool, denied: bool, duration_ms: u64, preview: &str) -> ActivityRecord {
    ActivityRecord {
        call_id: call.id.clone(),
        server_name: spec.map(|s| s.server_name.clone()).unwrap_or_default(),
        tool_name: spec.map(|s| s.tool_name.clone()).unwrap_or_else(|| call.function.name.clone()),
        category: spec.map(|s| s.category).unwrap_or(ToolCategory::Other),
        args: args.clone(),
        ok,
        denied,
        duration_ms,
        result_preview: truncate_chars(preview, 2000),
    }
}

pub fn title_from(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let t: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        format!("{}…", t.trim_end())
    } else if t.is_empty() {
        "New conversation".into()
    } else {
        t
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}\n…[truncated]", s.chars().take(max).collect::<String>())
    }
}

/// LM Studio requires a JSON-schema object for `parameters`.
fn normalize_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(o) if o.get("type").and_then(Value::as_str) == Some("object") => {
            let mut o = o.clone();
            o.entry("properties").or_insert_with(|| json!({}));
            Value::Object(o)
        }
        _ => json!({"type": "object", "properties": {}}),
    }
}

/// Converts stored messages to LM Studio chat messages, dropping incomplete
/// tool exchanges and trimming old turns to fit the context window.
///
/// Attachments are re-rendered from the store on every turn. Only the newest
/// user message may carry images: replaying every image of a long conversation
/// would fill the context window several times over.
pub fn build_history(history: &[Message], ctx_tokens: u32, system_chars: usize, store: &AttachmentStore, vision: bool) -> Vec<ChatMessage> {
    let last_user = history.iter().rposition(|m| m.role == "user");
    // Pass 1: map rows, keeping tool results only when their call exists.
    let mut out: Vec<ChatMessage> = Vec::new();
    let mut i = 0;
    while i < history.len() {
        let m = &history[i];
        match m.role.as_str() {
            "user" => {
                let attachments = attach::from_json(&m.attachments);
                out.push(attach::user_message(&m.content, &attachments, store, vision && last_user == Some(i)));
            }
            "assistant" => {
                if !m.content.trim().is_empty() {
                    out.push(ChatMessage::text("assistant", m.content.clone()));
                }
            }
            "assistant_tool_calls" => {
                let calls: Vec<ToolCall> = m.tool_calls.clone().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
                let mut results = Vec::new();
                let mut j = i + 1;
                while j < history.len() && history[j].role == "tool" {
                    results.push(&history[j]);
                    j += 1;
                }
                let complete = !calls.is_empty()
                    && calls.iter().all(|c| results.iter().any(|r| r.tool_call_id.as_deref() == Some(c.id.as_str())));
                if complete {
                    out.push(ChatMessage {
                        role: "assistant".into(),
                        content: if m.content.is_empty() { None } else { Some(MessageContent::Text(m.content.clone())) },
                        tool_calls: Some(calls),
                        tool_call_id: None,
                    });
                    for r in results {
                        out.push(ChatMessage {
                            role: "tool".into(),
                            content: Some(MessageContent::Text(r.content.clone())),
                            tool_calls: None,
                            tool_call_id: r.tool_call_id.clone(),
                        });
                    }
                }
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }

    // Pass 2: keep the newest messages within ~60% of the context window
    // (≈3 chars/token), never splitting an assistant tool call from its results.
    let budget = ((ctx_tokens as usize) * 3 * 6 / 10).saturating_sub(system_chars).max(2000);
    let mut used = 0;
    let mut start = out.len();
    while start > 0 {
        let msg = &out[start - 1];
        let len = msg.content.as_ref().map(MessageContent::len).unwrap_or(0)
            + msg.tool_calls.as_ref().map(|c| c.iter().map(|t| t.function.arguments.len() + 40).sum()).unwrap_or(0);
        let is_latest = start == out.len();
        if used + len > budget && !is_latest {
            break;
        }
        used += len;
        start -= 1;
    }
    // Do not begin with orphaned tool results or an assistant message.
    while start < out.len() && out[start].role != "user" {
        start += 1;
    }
    out.split_off(start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai::{ChatCompletion, ConnectionStatus, ModelInfo};
    use crate::services::attachments::AttachmentStore;
    use crate::services::chat::tools::{NoSpeech, ToolOutput};
    use crate::services::hardware::HardwareInfo;
    use async_trait::async_trait;
    use std::sync::Mutex as StdMutex;

    /// Scripted fake LM Studio: returns queued completions, records requests.
    struct FakeAi {
        script: StdMutex<Vec<ChatCompletion>>,
        requests: StdMutex<Vec<ChatRequest>>,
        fail: Option<fn() -> AppError>,
    }

    #[async_trait]
    impl AiService for FakeAi {
        async fn test_connection(&self) -> AppResult<ConnectionStatus> {
            unimplemented!()
        }
        async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
            if let Some(f) = self.fail {
                return Err(f());
            }
            Ok(vec![ModelInfo { id: "test-model".into(), kind: "llm".into(), loaded: true, tool_use: true, ..Default::default() }])
        }
        async fn load_model(&self, _: &str, _: Option<u32>) -> AppResult<()> {
            Ok(())
        }
        async fn chat(&self, req: ChatRequest, _c: CancellationToken, on_chunk: &mut (dyn FnMut(StreamChunk) + Send)) -> AppResult<ChatCompletion> {
            self.requests.lock().unwrap().push(req);
            let next = self.script.lock().unwrap().remove(0);
            for word in next.content.split_inclusive(' ') {
                on_chunk(StreamChunk::Content(word.to_string()));
            }
            Ok(next)
        }
    }

    struct FakeTools {
        permission: Permission,
    }

    #[async_trait]
    impl ToolProvider for FakeTools {
        async fn available_tools(&self) -> Vec<ToolSpec> {
            vec![ToolSpec {
                llm_name: "web__search".into(),
                server_id: "s1".into(),
                server_name: "Web Search".into(),
                tool_name: "search".into(),
                description: "Search the web".into(),
                input_schema: json!({"type":"object","properties":{"query":{"type":"string"}}}),
                category: ToolCategory::Search,
                permission: self.permission,
            }]
        }
        async fn call_tool(&self, _spec: &ToolSpec, args: Value) -> AppResult<ToolOutput> {
            Ok(ToolOutput {
                text: format!("PHP 8.5 released. query={}", args["query"]),
                is_error: false,
                sources: vec![Source { url: "https://www.php.net/releases/".into(), title: Some("PHP releases".into()) }],
            })
        }
    }

    fn store() -> (Arc<AttachmentStore>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (Arc::new(AttachmentStore::new(dir.path().join("attachments"))), dir)
    }

    fn engine(script: Vec<ChatCompletion>, permission: Permission) -> (Arc<ChatEngine>, Arc<FakeAi>, Arc<Db>, Arc<AttachmentStore>, tempfile::TempDir) {
        let db = Arc::new(Db::open_in_memory().unwrap());
        let settings = Arc::new(SettingsStore::load(db.clone()).unwrap());
        let ai = Arc::new(FakeAi { script: StdMutex::new(script), requests: StdMutex::new(vec![]), fail: None });
        let resolver = Arc::new(ModelResolver::with_hardware(ai.clone(), HardwareInfo::default()));
        let (files, dir) = store();
        let e = ChatEngine::new(db.clone(), settings, ai.clone(), resolver, Arc::new(FakeTools { permission }), Arc::new(NoSpeech), files.clone());
        (Arc::new(e), ai, db, files, dir)
    }

    fn collector() -> (Emit, Arc<StdMutex<Vec<ChatEvent>>>) {
        let events = Arc::new(StdMutex::new(Vec::new()));
        let e2 = events.clone();
        (Arc::new(move |ev| e2.lock().unwrap().push(ev)), events)
    }

    fn input(text: &str) -> SendInput {
        SendInput { turn_id: "t1".into(), conversation_id: None, text: text.into(), spoken_language: None, voice: false, attachment_ids: vec![] }
    }

    fn tool_call(args: &str) -> ToolCall {
        ToolCall { id: "call1".into(), kind: "function".into(), function: crate::services::ai::sse::ToolCallFunction { name: "web__search".into(), arguments: args.into() } }
    }

    #[tokio::test]
    async fn simple_turn_persists_and_detects_language() {
        let (e, ai, db, _files, _dir) = engine(vec![ChatCompletion { content: "مرحبا! أنا بخير.".into(), ..Default::default() }], Permission::Allow);
        let (emit, events) = collector();
        e.send(input("كيف حالك اليوم؟"), emit).await.unwrap();
        let ev = events.lock().unwrap();
        let ChatEvent::Done { message, .. } = ev.last().unwrap() else { panic!("expected done") };
        assert_eq!(message.language.as_deref(), Some("ar"));
        let req = &ai.requests.lock().unwrap()[0];
        assert!(req.messages[0].content_text().contains("respond in Arabic"));
        let convs = db.list_conversations(10, 0).unwrap();
        assert_eq!(db.list_messages(&convs[0].id).unwrap().len(), 2);
    }

    #[tokio::test]
    async fn tool_round_collects_sources() {
        let script = vec![
            ChatCompletion { tool_calls: vec![tool_call(r#"{"query":"latest php"}"#)], finish_reason: Some("tool_calls".into()), ..Default::default() },
            ChatCompletion { content: "PHP 8.5 is the latest. [php.net](https://www.php.net/releases/)".into(), ..Default::default() },
        ];
        let (e, ai, db, files, _dir) = engine(script, Permission::Allow);
        let (emit, events) = collector();
        e.send(input("What is the latest PHP version?"), emit).await.unwrap();
        let ev = events.lock().unwrap();
        assert!(ev.iter().any(|e| matches!(e, ChatEvent::ToolStarted { tool_name, .. } if tool_name == "search")));
        let ChatEvent::Done { message, .. } = ev.last().unwrap() else { panic!() };
        assert_eq!(message.sources.as_ref().unwrap()[0]["url"], "https://www.php.net/releases/");
        let reqs = ai.requests.lock().unwrap();
        assert!(reqs[0].messages[0].content_text().contains("use the web search tool"));
        let second = &reqs[1].messages;
        assert_eq!(second[second.len() - 1].role, "tool");
        assert_eq!(second[second.len() - 2].tool_calls.as_ref().unwrap()[0].id, "call1");
        // History replay keeps the complete tool exchange
        let conv = &db.list_conversations(1, 0).unwrap()[0];
        let hist = build_history(&db.list_messages(&conv.id).unwrap(), 8192, 0, &files, false);
        assert_eq!(hist.iter().map(|m| m.role.as_str()).collect::<Vec<_>>(), vec!["user", "assistant", "tool", "assistant"]);
    }

    #[tokio::test]
    async fn attachments_reach_the_prompt_and_the_stored_message() {
        let (e, ai, db, files, _dir) = engine(vec![ChatCompletion { content: "It lists two steps.".into(), ..Default::default() }], Permission::Allow);
        let attachment = files.ingest_text("plan.md", "step one
step two").unwrap();
        let (emit, events) = collector();
        let mut input = input("what does this say?");
        input.attachment_ids = vec![attachment.id.clone()];
        e.send(input, emit).await.unwrap();

        let prompt = ai.requests.lock().unwrap()[0].messages.last().unwrap().content_text();
        assert!(prompt.contains("plan.md"), "attachment missing from prompt: {prompt}");
        assert!(prompt.contains("step two"));

        let ChatEvent::Started { user_message, .. } = &events.lock().unwrap()[0] else { panic!("expected started") };
        assert_eq!(user_message.attachments.as_ref().unwrap()[0]["name"], "plan.md");
        // Claiming it once means a second turn cannot silently resend the file.
        assert!(files.claim(&[attachment.id]).is_empty());
        let conv = &db.list_conversations(1, 0).unwrap()[0];
        assert_eq!(db.attachment_ids().unwrap().len(), 1);
        assert_eq!(db.list_messages(&conv.id).unwrap().len(), 2);
    }

    #[tokio::test]
    async fn ask_permission_declined() {
        let script = vec![
            ChatCompletion { tool_calls: vec![tool_call(r#"{"query":"x"}"#)], ..Default::default() },
            ChatCompletion { content: "Okay, I won't search.".into(), ..Default::default() },
        ];
        let (e, ai, _db, _files, _dir) = engine(script, Permission::Ask);
        let (emit, events) = collector();
        let e2 = e.clone();
        let events2 = events.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
                let id = events2.lock().unwrap().iter().find_map(|ev| match ev {
                    ChatEvent::ToolAwaitingConfirmation { call_id, .. } => Some(call_id.clone()),
                    _ => None,
                });
                if let Some(id) = id {
                    assert!(e2.confirm_tool(&id, false));
                    break;
                }
            }
        });
        e.send(input("search something current today"), emit).await.unwrap();
        assert!(events.lock().unwrap().iter().any(|ev| matches!(ev, ChatEvent::ToolFinished { denied: true, .. })));
        assert!(!events.lock().unwrap().iter().any(|ev| matches!(ev, ChatEvent::ToolStarted { .. })));
        let reqs = ai.requests.lock().unwrap();
        assert!(reqs[1].messages.last().unwrap().content_text().contains("declined"));
    }

    #[tokio::test]
    async fn invalid_tool_name_is_reported_to_model() {
        let mut bad = tool_call("{}");
        bad.function.name = "rm_rf".into();
        let script = vec![
            ChatCompletion { tool_calls: vec![bad], ..Default::default() },
            ChatCompletion { content: "Sorry.".into(), ..Default::default() },
        ];
        let (e, ai, _db, _files, _dir) = engine(script, Permission::Allow);
        let (emit, _events) = collector();
        e.send(input("hello there"), emit).await.unwrap();
        assert!(ai.requests.lock().unwrap()[1].messages.last().unwrap().content_text().contains("does not exist"));
    }

    #[tokio::test]
    async fn lmstudio_unavailable_emits_error() {
        let db = Arc::new(Db::open_in_memory().unwrap());
        let settings = Arc::new(SettingsStore::load(db.clone()).unwrap());
        let ai = Arc::new(FakeAi { script: StdMutex::new(vec![]), requests: StdMutex::new(vec![]), fail: Some(|| AppError::LmStudioUnavailable("refused".into())) });
        let resolver = Arc::new(ModelResolver::with_hardware(ai.clone(), HardwareInfo::default()));
        let (files, _dir) = store();
        let e = ChatEngine::new(db, settings, ai, resolver, Arc::new(super::super::tools::NoTools), Arc::new(NoSpeech), files);
        let (emit, events) = collector();
        assert!(e.send(input("hi"), emit).await.is_err());
        assert!(events.lock().unwrap().iter().any(|ev| matches!(ev, ChatEvent::Error { code, .. } if code == "lmstudio_unavailable")));
    }

    #[test]
    fn history_trimming_starts_with_user() {
        let mk = |role: &str, content: String| Message {
            id: new_id(), conversation_id: "c".into(), role: role.into(), content, language: None, reasoning: None,
            sources: None, tool_activity: None, tool_calls: None, tool_call_id: None, attachments: None, created_at: now(),
        };
        let mut hist = Vec::new();
        for i in 0..50 {
            hist.push(mk("user", format!("question {i} {}", "x".repeat(500))));
            hist.push(mk("assistant", format!("answer {i} {}", "y".repeat(500))));
        }
        hist.push(mk("user", "final".into()));
        let (files, _dir) = store();
        let out = build_history(&hist, 4096, 1000, &files, false);
        assert_eq!(out.first().unwrap().role, "user");
        assert_eq!(out.last().unwrap().content_text(), "final");
        assert!(out.len() < hist.len());
    }

    #[test]
    fn titles() {
        assert_eq!(title_from("\n  Hello world \nmore"), "Hello world");
        assert_eq!(title_from(&"a".repeat(80)).chars().count(), 61);
    }
}
