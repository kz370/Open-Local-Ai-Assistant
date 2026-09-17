//! Separates in-band `<think>…</think>` reasoning from visible content in a
//! token stream (some models emit reasoning inside `content`).

#[derive(Default)]
pub struct ThinkFilter {
    in_think: bool,
    pending: String,
}

const OPEN: &str = "<think>";
const CLOSE: &str = "</think>";

impl ThinkFilter {
    /// Returns (visible, reasoning) text for this chunk.
    pub fn push(&mut self, chunk: &str) -> (String, String) {
        self.pending.push_str(chunk);
        let mut visible = String::new();
        let mut reasoning = String::new();
        loop {
            let tag = if self.in_think { CLOSE } else { OPEN };
            if let Some(pos) = self.pending.find(tag) {
                let before: String = self.pending.drain(..pos).collect();
                self.pending.drain(..tag.len());
                if self.in_think {
                    reasoning.push_str(&before);
                } else {
                    visible.push_str(&before);
                }
                self.in_think = !self.in_think;
                continue;
            }
            // Keep a possible partial tag at the end for the next chunk.
            let keep = partial_suffix_len(&self.pending, tag);
            let emit_len = self.pending.len() - keep;
            let out: String = self.pending.drain(..emit_len).collect();
            if self.in_think {
                reasoning.push_str(&out);
            } else {
                visible.push_str(&out);
            }
            break;
        }
        (visible, reasoning)
    }

    pub fn finish(&mut self) -> (String, String) {
        let rest = std::mem::take(&mut self.pending);
        if self.in_think {
            (String::new(), rest)
        } else {
            (rest, String::new())
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
}
