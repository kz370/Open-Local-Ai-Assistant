//! Turns a token stream into speakable sentences as soon as they complete.
//! Code fences are skipped entirely; decimals, abbreviations and URLs do not
//! end sentences; very long run-on text is split at a comma or space.

const TERMINATORS: &[char] = &['.', '!', '?', '…', '؟', '。'];
const CLOSERS: &[char] = &['"', '\'', ')', ']', '»', '”', '’'];
const ABBREVIATIONS: &[&str] = &[
    "e.g", "i.e", "etc", "vs", "mr", "mrs", "ms", "dr", "prof", "st", "no", "fig", "approx", "z.b", "bzw",
    "usw", "ca", "nr", "dh", "d.h", "u.a", "inkl", "evtl", "ggf", "vgl",
];
const MIN_CHARS: usize = 16;
const MAX_CHARS: usize = 280;

#[derive(Default)]
pub struct SentenceBuffer {
    raw: String,
    in_code: bool,
    carry: String,
}

impl SentenceBuffer {
    pub fn push(&mut self, text: &str) -> Vec<String> {
        self.raw.push_str(text);
        self.extract(false)
    }

    pub fn flush(&mut self) -> Vec<String> {
        let mut out = self.extract(true);
        if !self.in_code {
            let rest = std::mem::take(&mut self.raw);
            self.emit(&rest, true, &mut out);
        }
        self.raw.clear();
        self.in_code = false;
        let carry = std::mem::take(&mut self.carry);
        if carry.chars().any(char::is_alphanumeric) {
            out.push(carry);
        }
        out
    }

    fn emit(&mut self, raw_sentence: &str, force: bool, out: &mut Vec<String>) {
        let cleaned = super::speech_text::clean_for_speech(raw_sentence);
        if cleaned.is_empty() {
            return;
        }
        let combined = if self.carry.is_empty() { cleaned } else { format!("{} {}", std::mem::take(&mut self.carry), cleaned) };
        if combined.chars().count() < MIN_CHARS && !force {
            self.carry = combined;
        } else if combined.chars().any(char::is_alphanumeric) {
            out.push(combined);
        }
    }

    fn extract(&mut self, at_end: bool) -> Vec<String> {
        let mut out = Vec::new();
        loop {
            if self.in_code {
                match self.raw.find("```") {
                    Some(p) => match self.raw[p..].find('\n') {
                        Some(nl) => {
                            self.raw.drain(..p + nl + 1);
                            self.in_code = false;
                            continue;
                        }
                        None if at_end => {
                            self.raw.clear();
                            self.in_code = false;
                            break;
                        }
                        None => break,
                    },
                    None => {
                        // Keep a possible partial closing fence at the end.
                        let keep_from = self.raw.rfind('\n').map(|i| i + 1).unwrap_or(0);
                        self.raw.drain(..keep_from);
                        break;
                    }
                }
            }

            let fence = find_fence(&self.raw);
            let region_end = fence.unwrap_or(self.raw.len());
            if let Some(b) = next_boundary(&self.raw[..region_end], at_end || fence.is_some()) {
                let sentence: String = self.raw.drain(..b).collect();
                self.emit(&sentence, false, &mut out);
                continue;
            }
            if let Some(f) = fence {
                let before: String = self.raw.drain(..f).collect();
                self.emit(&before, false, &mut out);
                self.raw.drain(..3.min(self.raw.len()));
                self.in_code = true;
                continue;
            }
            if self.raw.chars().count() > MAX_CHARS {
                let cut = soft_cut(&self.raw);
                let sentence: String = self.raw.drain(..cut).collect();
                self.emit(&sentence, true, &mut out);
                continue;
            }
            break;
        }
        out
    }
}

/// Byte index of a ``` fence at the start of a line.
fn find_fence(s: &str) -> Option<usize> {
    let mut start = 0;
    while let Some(p) = s[start..].find("```") {
        let abs = start + p;
        let line_start = s[..abs].rfind('\n').map(|i| i + 1).unwrap_or(0);
        if s[line_start..abs].trim().is_empty() {
            return Some(line_start);
        }
        start = abs + 3;
    }
    None
}

/// Byte index just past the first complete sentence (including trailing
/// closers and whitespace), or None if more text is needed.
fn next_boundary(s: &str, at_end: bool) -> Option<usize> {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    for (i, &(pos, c)) in chars.iter().enumerate() {
        if c == '\n' {
            if s[..pos].trim().is_empty() {
                continue;
            }
            return Some(pos + 1);
        }
        if !TERMINATORS.contains(&c) {
            continue;
        }
        // Collapse runs like "?!" or "..."
        if chars.get(i + 1).is_some_and(|(_, n)| TERMINATORS.contains(n)) {
            continue;
        }
        let mut j = i + 1;
        while chars.get(j).is_some_and(|(_, n)| CLOSERS.contains(n)) {
            j += 1;
        }
        let Some(&(_, next)) = chars.get(j) else {
            if at_end {
                return Some(s.len());
            }
            return None; // need to see what follows
        };
        if !next.is_whitespace() {
            continue; // "3.14", "example.com", "v2.1"
        }
        if c == '.' && is_abbreviation(&s[..pos]) {
            continue;
        }
        if c == '.' && chars[..i].last().is_some_and(|(_, p)| p.is_ascii_digit()) && s[..pos].split_whitespace().last().is_some_and(|w| w.chars().all(|ch| ch.is_ascii_digit())) && s[..pos].split_whitespace().count() <= 1 {
            continue; // "1. item" list numbering at line start
        }
        let mut end = chars.get(j).map(|(p, _)| *p).unwrap_or(s.len());
        while let Some((p, ch)) = s[end..].char_indices().next().map(|(o, ch)| (end + o, ch)) {
            if ch.is_whitespace() && ch != '\n' {
                end = p + ch.len_utf8();
            } else {
                break;
            }
        }
        return Some(end);
    }
    None
}

fn is_abbreviation(before: &str) -> bool {
    let word = before
        .rsplit(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("")
        .to_lowercase();
    // Single letters ("J. Smith") are initials.
    (word.chars().count() == 1 && word.chars().all(char::is_alphabetic)) || ABBREVIATIONS.contains(&word.as_str())
}

fn soft_cut(s: &str) -> usize {
    let limit = s.char_indices().nth(MAX_CHARS - 40).map(|(i, _)| i).unwrap_or(s.len());
    let region = &s[..limit];
    region
        .rfind([',', '،', ';', ':'])
        .map(|i| i + region[i..].chars().next().map(char::len_utf8).unwrap_or(1))
        .or_else(|| region.rfind(' ').map(|i| i + 1))
        .filter(|&i| i > 0)
        .unwrap_or(limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(chunks: &[&str]) -> Vec<String> {
        let mut b = SentenceBuffer::default();
        let mut out = Vec::new();
        for c in chunks {
            out.extend(b.push(c));
        }
        out.extend(b.flush());
        out
    }

    #[test]
    fn spec_example_streams_first_sentence_early() {
        let mut b = SentenceBuffer::default();
        assert!(b.push("Hello! I can help").is_empty());
        let first = b.push(" you with that. Let me");
        assert_eq!(first, vec!["Hello! I can help you with that."]);
        assert!(b.push(" explain...").is_empty());
        assert_eq!(b.flush(), vec!["Let me explain..."]);
    }

    #[test]
    fn decimals_urls_abbreviations() {
        let out = feed(&["PHP 8.4 was released on php.net today. ", "It adds e.g. property hooks, z.B. neue Features. Done now, really."]);
        assert_eq!(out, vec!["PHP 8.4 was released on php.net today.", "It adds e.g. property hooks, z.B. neue Features.", "Done now, really."]);
    }

    #[test]
    fn skips_code_blocks() {
        let out = feed(&["Here is the code:\n```php\necho 'Hi. There!';\n", "```\nThat prints a greeting."]);
        assert_eq!(out, vec!["Here is the code:", "That prints a greeting."]);
    }

    #[test]
    fn arabic_and_german_terminators() {
        let out = feed(&["كيف يمكنني مساعدتك اليوم؟ ", "أنا هنا للمساعدة."]);
        assert_eq!(out, vec!["كيف يمكنني مساعدتك اليوم؟", "أنا هنا للمساعدة."]);
        let de = feed(&["Wie geht es dir heute? Mir geht es gut, danke."]);
        assert_eq!(de.len(), 2);
    }

    #[test]
    fn short_fragments_are_merged() {
        let out = feed(&["Yes. Sure. That works perfectly for your case."]);
        assert_eq!(out, vec!["Yes. Sure. That works perfectly for your case."]);
    }

    #[test]
    fn long_runon_is_split() {
        let long = "word, ".repeat(80);
        let out = feed(&[&long]);
        assert!(out.len() >= 2);
        assert!(out.iter().all(|s| s.chars().count() <= MAX_CHARS));
    }

    #[test]
    fn markdown_is_cleaned_and_lists_split() {
        let out = feed(&["## Steps\n", "1. **Open** the [settings](https://x.y/z) page\n2. Click save\n"]);
        assert_eq!(out, vec!["Steps Open the settings page", "Click save"]);
    }
}
