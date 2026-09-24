//! "Explain" for text the user selected in a reply, answered in a popup next
//! to the selection instead of as a message in the conversation.

use crate::services::ai::{ChatMessage, ChatRequest};
use serde::Serialize;

/// Selections are short; the surrounding reply is capped so a long answer
/// does not turn a quick explanation into a slow one.
const MAX_SELECTION_CHARS: usize = 4_000;
const MAX_PASSAGE_CHARS: usize = 12_000;

const EXPLAIN_PROMPT: &str = "You explain a piece of text the user selected in an assistant's reply. \
Say what it means in plain words, briefly: a short paragraph, or a few bullet points when it lists several things. \
Use the surrounding reply only as context. Answer in the language of the selected text. \
Do not repeat the selected text and do not start with a preamble.";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ExplainEvent {
    Delta { text: String },
    Done,
    Error { code: String, detail: String },
}

fn clip(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

pub fn request(model: &str, selection: &str, passage: &str, temperature: f32) -> ChatRequest {
    let selection = clip(selection.trim(), MAX_SELECTION_CHARS);
    let passage = clip(passage.trim(), MAX_PASSAGE_CHARS);
    let user = if passage.is_empty() || passage == selection {
        format!("<selection>\n{selection}\n</selection>")
    } else {
        format!("<reply>\n{passage}\n</reply>\n\n<selection>\n{selection}\n</selection>")
    };
    ChatRequest {
        model: model.to_string(),
        messages: vec![ChatMessage::text("system", EXPLAIN_PROMPT), ChatMessage::text("user", user)],
        tools: vec![],
        temperature,
        max_tokens: Some(700),
        stream: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_bounded_request() {
        let req = request("m", "  Photosynthese  ", "Pflanzen nutzen Photosynthese.", 0.7);
        let user = req.messages[1].content_text();
        assert!(user.contains("<selection>\nPhotosynthese\n</selection>"));
        assert!(user.contains("<reply>\nPflanzen nutzen Photosynthese.\n</reply>"));
        assert!(req.tools.is_empty());

        // The passage is left out when it is the selection itself.
        let same = request("m", "hello", "hello", 0.7);
        assert!(!same.messages[1].content_text().contains("<reply>"));

        // Long input is clipped on a character boundary.
        let long = "ع".repeat(MAX_PASSAGE_CHARS + 50);
        let req = request("m", "x", &long, 0.7);
        assert_eq!(req.messages[1].content_text().matches('ع').count(), MAX_PASSAGE_CHARS);
    }
}
