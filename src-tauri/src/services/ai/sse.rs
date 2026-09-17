//! Minimal Server-Sent Events decoder for OpenAI-compatible streaming, plus
//! accumulation of streamed tool-call fragments.

use serde::{Deserialize, Serialize};

/// Incremental SSE decoder: feed raw bytes, receive complete `data:` payloads.
#[derive(Default)]
pub struct SseDecoder {
    buf: Vec<u8>,
}

impl SseDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut events = Vec::new();
        loop {
            // An event ends at a blank line; accept both \n\n and \r\n\r\n.
            let Some((end, sep_len)) = find_event_end(&self.buf) else { break };
            let raw: Vec<u8> = self.buf.drain(..end + sep_len).collect();
            let text = String::from_utf8_lossy(&raw[..end]);
            let data: Vec<&str> = text
                .lines()
                .filter_map(|l| l.strip_prefix("data:"))
                .map(|d| d.strip_prefix(' ').unwrap_or(d))
                .collect();
            if !data.is_empty() {
                events.push(data.join("\n"));
            }
        }
        events
    }

    /// Flush a trailing event that was not terminated by a blank line.
    pub fn finish(&mut self) -> Vec<String> {
        if self.buf.iter().all(|b| b.is_ascii_whitespace()) {
            self.buf.clear();
            return Vec::new();
        }
        self.buf.extend_from_slice(b"\n\n");
        self.push(&[])
    }
}

fn find_event_end(buf: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i + 1 < buf.len() {
        if buf[i] == b'\n' && buf[i + 1] == b'\n' {
            return Some((i, 2));
        }
        if i + 3 < buf.len() && &buf[i..i + 4] == b"\r\n\r\n" {
            return Some((i, 4));
        }
        i += 1;
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolCallFunction,
}

#[derive(Deserialize)]
pub struct DeltaToolCall {
    pub index: Option<usize>,
    pub id: Option<String>,
    pub function: Option<DeltaFunction>,
}

#[derive(Deserialize)]
pub struct DeltaFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

/// Accumulates `delta.tool_calls` fragments keyed by index.
#[derive(Default)]
pub struct ToolCallAccumulator {
    calls: Vec<ToolCall>,
}

impl ToolCallAccumulator {
    pub fn push(&mut self, deltas: Vec<DeltaToolCall>) {
        for d in deltas {
            let idx = d.index.unwrap_or(self.calls.len().saturating_sub(if d.id.is_some() { 0 } else { 1 }));
            while self.calls.len() <= idx {
                self.calls.push(ToolCall { kind: "function".into(), ..Default::default() });
            }
            let call = &mut self.calls[idx];
            if let Some(id) = d.id {
                if !id.is_empty() {
                    call.id = id;
                }
            }
            if let Some(f) = d.function {
                if let Some(n) = f.name {
                    call.function.name.push_str(&n);
                }
                if let Some(a) = f.arguments {
                    call.function.arguments.push_str(&a);
                }
            }
        }
    }

    pub fn finish(self) -> Vec<ToolCall> {
        self.calls
            .into_iter()
            .enumerate()
            .filter(|(_, c)| !c.function.name.is_empty())
            .map(|(i, mut c)| {
                if c.id.is_empty() {
                    c.id = format!("call_{i}_{}", uuid::Uuid::new_v4().simple());
                }
                if c.function.arguments.trim().is_empty() {
                    c.function.arguments = "{}".into();
                }
                c
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_split_events() {
        let mut d = SseDecoder::default();
        assert!(d.push(b"data: {\"a\":").is_empty());
        let ev = d.push(b"1}\n\ndata: [DONE]\n\n");
        assert_eq!(ev, vec!["{\"a\":1}", "[DONE]"]);
    }

    #[test]
    fn decodes_crlf_and_comments() {
        let mut d = SseDecoder::default();
        let ev = d.push(b": keepalive\r\n\r\ndata: x\r\n\r\n");
        assert_eq!(ev, vec!["x"]);
    }

    #[test]
    fn utf8_split_across_chunks() {
        let mut d = SseDecoder::default();
        let bytes = "data: مرحبا\n\n".as_bytes();
        let (a, b) = bytes.split_at(9); // split inside a multibyte char
        assert!(d.push(a).is_empty());
        assert_eq!(d.push(b), vec!["مرحبا"]);
    }

    #[test]
    fn finish_flushes_unterminated() {
        let mut d = SseDecoder::default();
        assert!(d.push(b"data: tail").is_empty());
        assert_eq!(d.finish(), vec!["tail"]);
    }

    #[test]
    fn accumulates_tool_calls() {
        let mut acc = ToolCallAccumulator::default();
        let parse = |s: &str| serde_json::from_str::<Vec<DeltaToolCall>>(s).unwrap();
        acc.push(parse(r#"[{"index":0,"id":"c1","function":{"name":"web_","arguments":""}}]"#));
        acc.push(parse(r#"[{"index":0,"function":{"name":"search","arguments":"{\"q\":"}}]"#));
        acc.push(parse(r#"[{"index":0,"function":{"arguments":"\"php\"}"}}]"#));
        acc.push(parse(r#"[{"index":1,"id":"c2","function":{"name":"fetch"}}]"#));
        let calls = acc.finish();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "c1");
        assert_eq!(calls[0].function.name, "web_search");
        assert_eq!(calls[0].function.arguments, r#"{"q":"php"}"#);
        assert_eq!(calls[1].function.arguments, "{}");
    }
}
