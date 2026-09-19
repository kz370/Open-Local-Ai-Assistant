//! Separates in-band `<think>…</think>` reasoning from visible content in a
//! token stream (some models emit reasoning inside `content`). Also drops
//! `<tool_call>…</tool_call>` blocks a model writes as plain text when the
//! server did not parse them into real tool calls.

#[derive(Default)]
pub struct ThinkFilter {
    /// Index into `SPANS` of the span currently open.
    open: Option<usize>,
    pending: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Sink {
    Reasoning,
    Drop,
}

const SPANS: &[(&str, &str, Sink)] = &[("<think>", "</think>", Sink::Reasoning), ("<tool_call>", "</tool_call>", Sink::Drop)];

impl ThinkFilter {
    /// Returns (visible, reasoning) text for this chunk.
    pub fn push(&mut self, chunk: &str) -> (String, String) {
        self.pending.push_str(chunk);
        let mut visible = String::new();
        let mut reasoning = String::new();
        loop {
            let found = match self.open {
                Some(i) => self.pending.find(SPANS[i].1).map(|pos| (pos, SPANS[i].1.len(), None)),
                None => SPANS
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| self.pending.find(s.0).map(|pos| (pos, s.0.len(), Some(i))))
                    .min_by_key(|x| x.0),
            };
            if let Some((pos, tag_len, next)) = found {
                let before: String = self.pending.drain(..pos).collect();
                self.pending.drain(..tag_len);
                self.route(before, &mut visible, &mut reasoning);
                self.open = next;
                continue;
            }
            // Keep a possible partial tag at the end for the next chunk.
            let keep = match self.open {
                Some(i) => partial_suffix_len(&self.pending, SPANS[i].1),
                None => SPANS.iter().map(|s| partial_suffix_len(&self.pending, s.0)).max().unwrap_or(0),
            };
            let emit_len = self.pending.len() - keep;
            let out: String = self.pending.drain(..emit_len).collect();
            self.route(out, &mut visible, &mut reasoning);
            break;
        }
        (visible, reasoning)
    }

    pub fn finish(&mut self) -> (String, String) {
        let rest = std::mem::take(&mut self.pending);
        let (mut visible, mut reasoning) = (String::new(), String::new());
        self.route(rest, &mut visible, &mut reasoning);
        (visible, reasoning)
    }

    fn route(&self, text: String, visible: &mut String, reasoning: &mut String) {
        match self.open.map(|i| SPANS[i].2) {
            None => visible.push_str(&text),
            Some(Sink::Reasoning) => reasoning.push_str(&text),
            Some(Sink::Drop) => {}
        }
    }
}

fn partial_suffix_len(s: &str, tag: &str) -> usize {
    (1..tag.len())
        .rev()
        .find(|&n| s.len() >= n && s.is_char_boundary(s.len() - n) && tag.starts_with(&s[s.len() - n..]))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_across_chunks() {
        let mut f = ThinkFilter::default();
        let mut vis = String::new();
        let mut rea = String::new();
        for c in ["<thi", "nk>plan", " it</th", "ink>Hello", " <b>x</b>"] {
            let (v, r) = f.push(c);
            vis.push_str(&v);
            rea.push_str(&r);
        }
        let (v, r) = f.finish();
        vis.push_str(&v);
        rea.push_str(&r);
        assert_eq!(vis, "Hello <b>x</b>");
        assert_eq!(rea, "plan it");
    }

    #[test]
    fn passthrough_and_unicode() {
        let mut f = ThinkFilter::default();
        assert_eq!(f.push("مرحبا <").0, "مرحبا ");
        assert_eq!(f.push("3").0, "<3");
    }

    #[test]
    fn drops_leaked_tool_calls() {
        let mut f = ThinkFilter::default();
        let mut vis = String::new();
        for c in ["Opening it. <tool_", "call> <function=run> <parameter=command> x", " </tool_call> Done", " <tool_call> unclosed"] {
            vis.push_str(&f.push(c).0);
        }
        vis.push_str(&f.finish().0);
        assert_eq!(vis, "Opening it.  Done ");
    }
}
