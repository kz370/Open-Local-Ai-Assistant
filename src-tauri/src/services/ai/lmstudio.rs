//! LM Studio client over its local HTTP APIs.
//!
//! * Chat: OpenAI-compatible `POST {base}/chat/completions` (streaming, tools).
//! * Discovery: native `GET /api/v1/models` (size, quantization, loaded state,
//!   capabilities), falling back to `/api/v0/models`, then `{base}/models`.
//! * Loading: native `POST /api/v1/models/load`.

use super::sse::{DeltaToolCall, SseDecoder, ToolCallAccumulator};
use super::*;
use crate::errors::{from_lmstudio_http, AppError, AppResult};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::sync::RwLock;
use std::time::{Duration, Instant};

pub struct LmStudioService {
    client: reqwest::Client,
    base_url: RwLock<String>,
    timeout: RwLock<Duration>,
}

impl LmStudioService {
    pub fn new(base_url: &str, timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(4))
            // LM Studio is local: never route through a system proxy.
            .no_proxy()
            .build()
            .expect("http client");
        Self {
            client,
            base_url: RwLock::new(normalize_base(base_url)),
            timeout: RwLock::new(Duration::from_secs(timeout_secs)),
        }
    }

    pub fn set_base_url(&self, url: &str) {
        *self.base_url.write().unwrap_or_else(|p| p.into_inner()) = normalize_base(url);
    }

    pub fn set_timeout(&self, secs: u64) {
        *self.timeout.write().unwrap_or_else(|p| p.into_inner()) = Duration::from_secs(secs);
    }

    pub fn base_url(&self) -> String {
        self.base_url.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn timeout(&self) -> Duration {
        *self.timeout.read().unwrap_or_else(|p| p.into_inner())
    }

    /// Server root without the OpenAI `/v1` suffix, for native endpoints.
    fn root(&self) -> String {
        let base = self.base_url();
        base.strip_suffix("/v1").unwrap_or(&base).to_string()
    }

    async fn get_json(&self, url: &str) -> AppResult<Value> {
        let resp = self
            .client
            .get(url)
            .timeout(Duration::from_secs(8))
            .send()
            .await
            .map_err(from_lmstudio_http)?;
        if !resp.status().is_success() {
            return Err(AppError::LmStudio(format!("{url} returned {}", resp.status())));
        }
        resp.json().await.map_err(|e| AppError::LmStudio(e.to_string()))
    }

    async fn discover(&self) -> AppResult<(Vec<ModelInfo>, &'static str)> {
        let root = self.root();
        let v1_err = match self.get_json(&format!("{root}/api/v1/models")).await {
            Ok(v) if v.get("models").is_some() => return Ok((parse_native_v1(&v), "native-v1")),
            Ok(_) => AppError::LmStudio("unexpected /api/v1/models payload".into()),
            Err(e) => e,
        };
        // A refused connection means the server is down: don't try other paths.
        if matches!(v1_err, AppError::LmStudioUnavailable(_) | AppError::Timeout(_)) {
            return Err(v1_err);
        }
        if let Ok(v) = self.get_json(&format!("{root}/api/v0/models")).await {
            if v.get("data").is_some() {
                return Ok((parse_native_v0(&v), "native-v0"));
            }
        }
        let v = self.get_json(&format!("{}/models", self.base_url())).await?;
        Ok((parse_openai_models(&v), "openai"))
    }
}

fn normalize_base(url: &str) -> String {
    let mut u = url.trim().trim_end_matches('/').to_string();
    if u.is_empty() {
        u = crate::settings::DEFAULT_LMSTUDIO_URL.into();
    }
    if !u.starts_with("http://") && !u.starts_with("https://") {
        u = format!("http://{u}");
    }
    u
}

fn as_u32(v: Option<&Value>) -> Option<u32> {
    v.and_then(Value::as_u64).map(|n| n.min(u32::MAX as u64) as u32)
}

pub fn parse_native_v1(v: &Value) -> Vec<ModelInfo> {
    v["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let id = m.get("key")?.as_str()?.to_string();
                    let loaded = m["loaded_instances"].as_array().map(|a| !a.is_empty()).unwrap_or(false);
                    let caps = &m["capabilities"];
                    Some(ModelInfo {
                        display_name: m["display_name"].as_str().unwrap_or(&id).to_string(),
                        kind: m["type"].as_str().unwrap_or("unknown").to_string(),
                        size_bytes: m["size_bytes"].as_u64(),
                        params: m["params_string"].as_str().map(str::to_string),
                        quantization: m["quantization"]["name"].as_str().map(str::to_string),
                        bits_per_weight: m["quantization"]["bits_per_weight"].as_f64().map(|b| b as f32),
                        max_context_length: as_u32(m.get("max_context_length")),
                        loaded,
                        loaded_context_length: as_u32(m["loaded_instances"].get(0).and_then(|i| i["config"].get("context_length"))),
                        tool_use: caps["trained_for_tool_use"].as_bool().unwrap_or(false),
                        vision: caps["vision"].as_bool().unwrap_or(false),
                        reasoning: caps.get("reasoning").map(|r| !r.is_null()).unwrap_or(false),
                        id,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_native_v0(v: &Value) -> Vec<ModelInfo> {
    v["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let id = m.get("id")?.as_str()?.to_string();
                    let caps: Vec<&str> = m["capabilities"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                    Some(ModelInfo {
                        display_name: id.clone(),
                        kind: m["type"].as_str().unwrap_or("unknown").to_string(),
                        size_bytes: None,
                        params: None,
                        quantization: m["quantization"].as_str().map(str::to_string),
                        bits_per_weight: None,
                        max_context_length: as_u32(m.get("max_context_length")),
                        loaded: m["state"].as_str() == Some("loaded"),
                        loaded_context_length: as_u32(m.get("loaded_context_length")),
                        tool_use: caps.contains(&"tool_use"),
                        vision: m["type"].as_str() == Some("vlm"),
                        reasoning: false,
                        id,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_openai_models(v: &Value) -> Vec<ModelInfo> {
    v["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id")?.as_str())
                .map(|id| ModelInfo {
                    id: id.to_string(),
                    display_name: id.to_string(),
                    kind: if id.contains("embed") { "embedding".into() } else { "unknown".into() },
                    ..Default::default()
                })
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl AiService for LmStudioService {
    async fn test_connection(&self) -> AppResult<ConnectionStatus> {
        let started = Instant::now();
        let (models, api) = self.discover().await?;
        Ok(ConnectionStatus {
            connected: true,
            server_url: self.base_url(),
            model_count: models.iter().filter(|m| m.is_chat_model()).count(),
            latency_ms: started.elapsed().as_millis() as u64,
            api: api.into(),
        })
    }

    async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
        Ok(self.discover().await?.0)
    }

    async fn load_model(&self, model_id: &str, context_length: Option<u32>) -> AppResult<()> {
        let mut body = json!({ "model": model_id });
        if let Some(ctx) = context_length {
            body["context_length"] = json!(ctx);
        }
        let resp = self
            .client
            .post(format!("{}/api/v1/models/load", self.root()))
            .json(&body)
            .timeout(Duration::from_secs(600))
            .send()
            .await
            .map_err(from_lmstudio_http)?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::LmStudio(format!("load failed ({status}): {}", truncate(&text, 300))));
        }
        Ok(())
    }

    async fn chat(
        &self,
        req: ChatRequest,
        cancel: CancellationToken,
        on_chunk: &mut (dyn FnMut(StreamChunk) + Send),
    ) -> AppResult<ChatCompletion> {
        let mut body = json!({
            "model": req.model,
            "messages": req.messages,
            "temperature": req.temperature,
            "stream": req.stream,
        });
        if let Some(mt) = req.max_tokens {
            body["max_tokens"] = json!(mt);
        }
        if !req.tools.is_empty() {
            body["tools"] = json!(req.tools);
            body["tool_choice"] = json!("auto");
        }

        let send = self
            .client
            .post(format!("{}/chat/completions", self.base_url()))
            .json(&body)
            .timeout(self.timeout())
            .send();
        let resp = tokio::select! {
            r = send => r.map_err(from_lmstudio_http)?,
            _ = cancel.cancelled() => return Err(AppError::Cancelled),
        };
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let detail = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().or(v["error"].as_str()).map(str::to_string))
                .unwrap_or_else(|| truncate(&text, 300));
            return Err(AppError::LmStudio(format!("{status}: {detail}")));
        }

        let mut out = ChatCompletion::default();

        if !req.stream {
            let v: Value = tokio::select! {
                r = resp.json() => r.map_err(|e| AppError::LmStudio(e.to_string()))?,
                _ = cancel.cancelled() => return Err(AppError::Cancelled),
            };
            let msg = &v["choices"][0]["message"];
            out.content = msg["content"].as_str().unwrap_or("").to_string();
            out.reasoning = msg["reasoning_content"].as_str().or(msg["reasoning"].as_str()).unwrap_or("").to_string();
            if !out.reasoning.is_empty() {
                on_chunk(StreamChunk::Reasoning(out.reasoning.clone()));
            }
            if !out.content.is_empty() {
                on_chunk(StreamChunk::Content(out.content.clone()));
            }
            if let Some(calls) = msg.get("tool_calls") {
                out.tool_calls = serde_json::from_value(calls.clone()).unwrap_or_default();
            }
            out.finish_reason = v["choices"][0]["finish_reason"].as_str().map(str::to_string);
            return Ok(out);
        }

        let mut stream = resp.bytes_stream();
        let mut decoder = SseDecoder::default();
        let mut tools = ToolCallAccumulator::default();
        let mut done = false;

        while !done {
            let chunk = tokio::select! {
                c = stream.next() => c,
                _ = cancel.cancelled() => return Err(AppError::Cancelled),
            };
            let events = match chunk {
                Some(Ok(bytes)) => decoder.push(&bytes),
                Some(Err(e)) => return Err(from_lmstudio_http(e)),
                None => {
                    done = true;
                    decoder.finish()
                }
            };
            for data in events {
                if data.trim() == "[DONE]" {
                    done = true;
                    break;
                }
                let Ok(v) = serde_json::from_str::<Value>(&data) else { continue };
                if let Some(err) = v.get("error") {
                    let msg = err["message"].as_str().or(err.as_str()).unwrap_or("stream error");
                    return Err(AppError::LmStudio(msg.to_string()));
                }
                let Some(choice) = v["choices"].get(0) else { continue };
                let delta = &choice["delta"];
                if let Some(r) = delta["reasoning_content"].as_str().or(delta["reasoning"].as_str()) {
                    if !r.is_empty() {
                        out.reasoning.push_str(r);
                        on_chunk(StreamChunk::Reasoning(r.to_string()));
                    }
                }
                if let Some(c) = delta["content"].as_str() {
                    if !c.is_empty() {
                        out.content.push_str(c);
                        on_chunk(StreamChunk::Content(c.to_string()));
                    }
                }
                if let Some(tc) = delta.get("tool_calls") {
                    if let Ok(parsed) = serde_json::from_value::<Vec<DeltaToolCall>>(tc.clone()) {
                        tools.push(parsed);
                    }
                }
                if let Some(fr) = choice["finish_reason"].as_str() {
                    out.finish_reason = Some(fr.to_string());
                }
            }
        }
        out.tool_calls = tools.finish();
        Ok(out)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::State,
        http::{header, StatusCode},
        response::{IntoResponse, Response},
        routing::{get, post},
        Json, Router,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Mock {
        last_body: Arc<Mutex<Option<Value>>>,
        stream_body: Arc<String>,
        native_v1: bool,
        delay_ms: u64,
    }

    async fn v1_models(State(m): State<Mock>) -> Response {
        if !m.native_v1 {
            return StatusCode::NOT_FOUND.into_response();
        }
        Json(json!({"models": [
            {"type":"llm","key":"qwen/qwen3-14b","display_name":"Qwen3 14B","size_bytes": 9000000000u64,
             "params_string":"14B","quantization":{"name":"Q4_K_M","bits_per_weight":4},
             "loaded_instances":[],"max_context_length":40960,
             "capabilities":{"vision":false,"trained_for_tool_use":true}},
            {"type":"llm","key":"loaded-7b","display_name":"Loaded","size_bytes": 5000000000u64,
             "params_string":"7B","loaded_instances":[{"id":"loaded-7b","config":{"context_length":8192}}],
             "max_context_length":32768,"capabilities":{"trained_for_tool_use":false}},
            {"type":"embedding","key":"nomic-embed","size_bytes": 80000000u64,"loaded_instances":[]}
        ]}))
        .into_response()
    }

    async fn v0_models() -> Response {
        Json(json!({"data":[{"id":"old-model","type":"llm","state":"loaded","capabilities":["tool_use"],"max_context_length":4096}]})).into_response()
    }

    async fn completions(State(m): State<Mock>, Json(body): Json<Value>) -> Response {
        *m.last_body.lock().unwrap() = Some(body.clone());
        if m.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(m.delay_ms)).await;
        }
        if body["model"] == "missing" {
            return (StatusCode::NOT_FOUND, Json(json!({"error":{"message":"model not found"}}))).into_response();
        }
        if body["stream"] == json!(false) {
            return Json(json!({"choices":[{"message":{"role":"assistant","content":"plain answer"},"finish_reason":"stop"}]})).into_response();
        }
        Response::builder()
            .header(header::CONTENT_TYPE, "text/event-stream")
            .body(Body::from(m.stream_body.to_string()))
            .unwrap()
    }

    async fn serve(mock: Mock) -> String {
        let app = Router::new()
            .route("/api/v1/models", get(v1_models))
            .route("/api/v0/models", get(v0_models))
            .route("/v1/chat/completions", post(completions))
            .with_state(mock);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/v1")
    }

    fn sse(chunks: &[Value]) -> String {
        let mut s: String = chunks.iter().map(|c| format!("data: {c}\n\n")).collect();
        s.push_str("data: [DONE]\n\n");
        s
    }

    fn req(model: &str, stream: bool) -> ChatRequest {
        ChatRequest {
            model: model.into(),
            messages: vec![ChatMessage::text("user", "hi")],
            tools: vec![],
            temperature: 0.5,
            max_tokens: Some(100),
            stream,
        }
    }

    #[tokio::test]
    async fn discovery_native_v1() {
        let url = serve(Mock { native_v1: true, ..Default::default() }).await;
        let svc = LmStudioService::new(&url, 30);
        let models = svc.list_models().await.unwrap();
        assert_eq!(models.len(), 3);
        let q = &models[0];
        assert_eq!(q.id, "qwen/qwen3-14b");
        assert!(q.tool_use && !q.loaded);
        assert_eq!(q.size_bytes, Some(9_000_000_000));
        assert!(models[1].loaded);
        assert_eq!(models[1].loaded_context_length, Some(8192));
        assert_eq!(models[2].kind, "embedding");
        let status = svc.test_connection().await.unwrap();
        assert!(status.connected);
        assert_eq!(status.model_count, 2);
        assert_eq!(status.api, "native-v1");
    }

    #[tokio::test]
    async fn discovery_falls_back_to_v0() {
        let url = serve(Mock::default()).await;
        let svc = LmStudioService::new(&url, 30);
        let models = svc.list_models().await.unwrap();
        assert_eq!(models[0].id, "old-model");
        assert!(models[0].loaded && models[0].tool_use);
    }

    #[tokio::test]
    async fn server_unavailable() {
        // Bind then drop to get a port that refuses connections.
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let svc = LmStudioService::new(&format!("http://127.0.0.1:{port}/v1"), 30);
        let err = svc.test_connection().await.unwrap_err();
        assert_eq!(err.code(), "lmstudio_unavailable");
    }

    #[tokio::test]
    async fn streaming_content_reasoning_and_tools() {
        let body = sse(&[
            json!({"choices":[{"delta":{"role":"assistant","reasoning":"think"}}]}),
            json!({"choices":[{"delta":{"content":"Hello"}}]}),
            json!({"choices":[{"delta":{"content":" world."}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"a","type":"function","function":{"name":"web_search","arguments":""}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"query\":\"php\"}"}}]}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
        ]);
        let mock = Mock { stream_body: Arc::new(body), ..Default::default() };
        let url = serve(mock.clone()).await;
        let svc = LmStudioService::new(&url, 30);
        let mut chunks = Vec::new();
        let mut cb = |c: StreamChunk| chunks.push(c);
        let mut r = req("m", true);
        r.tools = vec![ToolDefinition {
            kind: "function".into(),
            function: FunctionDefinition { name: "web_search".into(), description: "d".into(), parameters: json!({"type":"object"}) },
        }];
        let out = svc.chat(r, CancellationToken::new(), &mut cb).await.unwrap();
        assert_eq!(out.content, "Hello world.");
        assert_eq!(out.reasoning, "think");
        assert_eq!(out.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(out.tool_calls.len(), 1);
        assert_eq!(out.tool_calls[0].function.arguments, "{\"query\":\"php\"}");
        assert_eq!(chunks[0], StreamChunk::Reasoning("think".into()));
        assert_eq!(chunks[1], StreamChunk::Content("Hello".into()));
        let sent = mock.last_body.lock().unwrap().clone().unwrap();
        assert_eq!(sent["tools"][0]["function"]["name"], "web_search");
        assert_eq!(sent["max_tokens"], 100);
    }

    #[tokio::test]
    async fn non_streaming() {
        let url = serve(Mock::default()).await;
        let svc = LmStudioService::new(&url, 30);
        let out = svc.chat(req("m", false), CancellationToken::new(), &mut |_| {}).await.unwrap();
        assert_eq!(out.content, "plain answer");
    }

    #[tokio::test]
    async fn http_error_is_reported() {
        let url = serve(Mock::default()).await;
        let svc = LmStudioService::new(&url, 30);
        let err = svc.chat(req("missing", true), CancellationToken::new(), &mut |_| {}).await.unwrap_err();
        assert!(err.to_string().contains("model not found"));
    }

    #[tokio::test]
    async fn timeout() {
        let url = serve(Mock { delay_ms: 3000, ..Default::default() }).await;
        let svc = LmStudioService::new(&url, 1);
        let err = svc.chat(req("m", true), CancellationToken::new(), &mut |_| {}).await.unwrap_err();
        assert_eq!(err.code(), "timeout");
    }

    #[tokio::test]
    async fn cancellation() {
        let url = serve(Mock { delay_ms: 3000, ..Default::default() }).await;
        let svc = LmStudioService::new(&url, 30);
        let token = CancellationToken::new();
        let t2 = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            t2.cancel();
        });
        let err = svc.chat(req("m", true), token, &mut |_| {}).await.unwrap_err();
        assert_eq!(err.code(), "cancelled");
    }

    #[test]
    fn normalizes_urls() {
        assert_eq!(normalize_base("localhost:1234/v1/"), "http://localhost:1234/v1");
        assert_eq!(normalize_base(""), "http://localhost:1234/v1");
        let s = LmStudioService::new("http://127.0.0.1:9999/v1", 5);
        assert_eq!(s.root(), "http://127.0.0.1:9999");
    }
}
