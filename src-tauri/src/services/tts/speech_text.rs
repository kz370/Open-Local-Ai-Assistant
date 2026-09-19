//! Converts Markdown-ish assistant text into plain speakable text.

use regex::Regex;
use std::sync::LazyLock;

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!?\[([^\]]*)\]\([^)]*\)").unwrap());
static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:https?://|www\.)\S+").unwrap());
static LIST_MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*(?:[-*+•]|\d+[.)])\s+").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*#{1,6}\s*").unwrap());
static QUOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*>\s?").unwrap());
static EMPH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\*\*|__|\*|~~|`)").unwrap());
static TABLE_RULE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*\|?\s*:?-{3,}.*$").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
/// Quote marks that some voices pronounce ("quote", "Anführungszeichen").
/// The apostrophe is left alone: it belongs inside words like "don't".
const QUOTE_MARKS: &[char] = &['"', '“', '”', '„', '‟', '«', '»', '‹', '›', '＂', '「', '」', '『', '』'];

pub fn clean_for_speech(text: &str) -> String {
    let s = LINK.replace_all(text, "$1");
    let s = URL.replace_all(&s, "");
    let s = TABLE_RULE.replace_all(&s, "");
    let s = HEADING.replace_all(&s, "");
    let s = QUOTE.replace_all(&s, "");
    let s = LIST_MARK.replace_all(&s, "");
    let s = EMPH.replace_all(&s, "");
    let s = s.replace('|', " ");
    let s = s.replace(QUOTE_MARKS, "");
    // Underscore emphasis only when it wraps words (keep snake_case identifiers).
    let s = SPACES.replace_all(&s, " ");
    let trimmed = s.trim();
    if trimmed.chars().any(char::is_alphanumeric) {
        trimmed.to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markdown() {
        assert_eq!(clean_for_speech("**Bold** and `code` see [docs](https://a.b)."), "Bold and code see docs.");
        assert_eq!(clean_for_speech("- item one"), "item one");
        assert_eq!(clean_for_speech("Visit https://php.net/downloads now"), "Visit now");
        assert_eq!(clean_for_speech("| a | b |\n|---|---|"), "a b");
        assert_eq!(clean_for_speech("---"), "");
        assert_eq!(clean_for_speech("### مرحبا **بك**"), "مرحبا بك");
    }

    #[test]
    fn drops_quote_marks_but_keeps_apostrophes() {
        assert_eq!(clean_for_speech(r#"He said "hello" twice"#), "He said hello twice");
        assert_eq!(clean_for_speech("She said “hi” and left"), "She said hi and left");
        assert_eq!(clean_for_speech("Er sagte „hallo“ zu mir"), "Er sagte hallo zu mir");
        assert_eq!(clean_for_speech("don't stop"), "don't stop");
        assert_eq!(clean_for_speech("قال «مرحبا» لي"), "قال مرحبا لي");
    }
}
