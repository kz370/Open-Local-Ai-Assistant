//! Text language detection restricted to the supported languages
//! (English, Arabic, German). Works on short chat messages, mixed
//! Arabic/English text, and ignores code blocks and URLs.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Ar,
    De,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ar => "ar",
            Lang::De => "de",
        }
    }

    pub fn english_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ar => "Arabic",
            Lang::De => "German",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        let c = code.trim().to_ascii_lowercase();
        match c.split(['-', '_']).next().unwrap_or("") {
            "en" | "english" => Some(Lang::En),
            "ar" | "arabic" => Some(Lang::Ar),
            "de" | "german" => Some(Lang::De),
            _ => None,
        }
    }

    pub fn is_rtl(self) -> bool {
        self == Lang::Ar
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

fn is_arabic_char(c: char) -> bool {
    matches!(c as u32, 0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF)
        && c.is_alphabetic()
}

/// Removes fenced/inline code and URLs, which are language-neutral noise.
pub fn strip_noise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut in_code = false;
        for word in line.split_inclusive(char::is_whitespace) {
            let w = word.trim();
            if w.starts_with("http://") || w.starts_with("https://") || w.starts_with("www.") {
                out.push(' ');
                continue;
            }
            for c in word.chars() {
                if c == '`' {
                    in_code = !in_code;
                    continue;
                }
                if !in_code {
                    out.push(c);
                }
            }
        }
        out.push('\n');
    }
    out
}

const EN_WORDS: &[&str] = &[
    "the", "and", "is", "are", "you", "what", "how", "can", "do", "does", "this", "that", "with", "for",
    "please", "hello", "hi", "thanks", "thank", "my", "your", "it", "of", "to", "in", "a", "an", "why",
    "where", "when", "which", "who", "i", "me", "we", "be", "have", "has", "not", "about", "tell",
    "explain", "latest", "today", "should", "would", "could", "will", "there", "yes", "no", "or",
];

const DE_WORDS: &[&str] = &[
    "der", "die", "das", "und", "ist", "sind", "du", "ich", "wie", "was", "kannst", "können", "kann",
    "nicht", "mit", "für", "bitte", "danke", "hallo", "ein", "eine", "einen", "mein", "meine", "dein",
    "deine", "es", "geht", "dir", "mir", "heute", "wer", "wo", "warum", "welche", "welcher", "auf",
    "zu", "von", "den", "dem", "des", "ja", "nein", "oder", "aber", "auch", "noch", "sie", "wir",
    "ihr", "erkläre", "erklär", "neueste", "aktuelle", "gibt", "habe", "hast", "bin", "bist", "tun",
    "machen", "guten", "morgen", "tag", "abend", "wann", "wieso", "über", "ob",
];

/// Words that exist in both languages and must not count for either.
const SHARED: &[&str] = &["was", "so", "in", "hand", "bank", "name", "man", "also", "rose", "art"];

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Detection {
    pub lang: Lang,
    /// 0.0..=1.0
    pub confidence: f32,
}

pub fn detect(text: &str) -> Option<Detection> {
    let clean = strip_noise(text);
    let arabic = clean.chars().filter(|c| is_arabic_char(*c)).count();
    let latin = clean.chars().filter(|c| c.is_ascii_alphabetic() || "äöüßÄÖÜ".contains(*c)).count();
    if arabic + latin == 0 {
        return None;
    }
    // Arabic words dominate or are substantial in mixed text, e.g.
    // "اشرح لي dependency injection في Laravel": technical English terms are
    // embedded in an Arabic sentence, so compare word counts, not letters.
    if arabic >= 2 {
        let (mut ar_words, mut other_words) = (0usize, 0usize);
        for w in clean.split_whitespace() {
            if w.chars().any(is_arabic_char) {
                ar_words += 1;
            } else if w.chars().any(|c| c.is_alphabetic()) {
                other_words += 1;
            }
        }
        let ratio = ar_words as f32 / (ar_words + other_words).max(1) as f32;
        if ratio >= 0.34 {
            return Some(Detection { lang: Lang::Ar, confidence: ratio.max(0.6) });
        }
    }
    if latin < 2 {
        return None;
    }

    let lower = clean.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_alphabetic() || c == '\''))
        .filter(|w| !w.is_empty())
        .collect();
    let mut en = 0.0f32;
    let mut de = 0.0f32;
    for w in &words {
        if SHARED.contains(w) {
            continue;
        }
        if EN_WORDS.contains(w) {
            en += 1.0;
        }
        if DE_WORDS.contains(w) {
            de += 1.0;
        }
    }
    de += lower.chars().filter(|c| "äöüß".contains(*c)).count() as f32 * 0.75;

    let total = en + de;
    if total >= 1.0 && (en - de).abs() >= 1.0 {
        let lang = if de > en { Lang::De } else { Lang::En };
        let conf = (en.max(de) / total).clamp(0.5, 1.0);
        return Some(Detection { lang, confidence: conf });
    }

    // Longer texts without clear stop-word signals: statistical detector
    // restricted to our languages.
    if words.len() >= 3 {
        let detector = whatlang::Detector::with_allowlist(vec![whatlang::Lang::Eng, whatlang::Lang::Deu]);
        if let Some(info) = detector.detect(&clean) {
            let lang = if info.lang() == whatlang::Lang::Deu { Lang::De } else { Lang::En };
            return Some(Detection { lang, confidence: info.confidence() as f32 * 0.8 });
        }
    }
    if total > 0.0 {
        return Some(Detection { lang: if de > en { Lang::De } else { Lang::En }, confidence: 0.4 });
    }
    None
}

/// Detects the language, falling back to `fallback` when the text is too
/// short or ambiguous (e.g. "ok", "PHP 8.3?").
pub fn detect_or(text: &str, fallback: Lang) -> Lang {
    detect(text).map(|d| d.lang).unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(t: &str) -> Option<Lang> {
        detect(t).map(|d| d.lang)
    }

    #[test]
    fn spec_examples() {
        assert_eq!(lang("Hello, what can you do?"), Some(Lang::En));
        assert_eq!(lang("ما الذي يمكنك فعله؟"), Some(Lang::Ar));
        assert_eq!(lang("Was kannst du tun?"), Some(Lang::De));
        assert_eq!(lang("How are you today?"), Some(Lang::En));
        assert_eq!(lang("كيف حالك اليوم؟"), Some(Lang::Ar));
        assert_eq!(lang("Wie geht es dir heute?"), Some(Lang::De));
        assert_eq!(lang("Wie kann ich meine Dateien organisieren?"), Some(Lang::De));
        assert_eq!(lang("What's the latest version of PHP?"), Some(Lang::En));
        assert_eq!(lang("Explain dependency injection in PHP."), Some(Lang::En));
    }

    #[test]
    fn mixed_arabic_english() {
        assert_eq!(lang("ما هو آخر إصدار من PHP؟"), Some(Lang::Ar));
        assert_eq!(lang("اشرح لي dependency injection في Laravel"), Some(Lang::Ar));
    }

    #[test]
    fn german_without_stopwords_uses_umlauts_or_statistics() {
        assert_eq!(lang("Größe ändern"), Some(Lang::De));
        assert_eq!(lang("Erzähl mir einen Witz über Katzen"), Some(Lang::De));
    }

    #[test]
    fn ignores_code_and_urls() {
        let t = "Wie funktioniert das?\n```php\n$this->is->the->english->code();\n```\nhttps://www.example.com/the/and/is";
        assert_eq!(lang(t), Some(Lang::De));
    }

    #[test]
    fn ambiguous_inputs() {
        assert_eq!(lang("123 456"), None);
        assert_eq!(lang("   "), None);
        assert_eq!(detect_or("ok", Lang::Ar), Lang::Ar);
    }

    #[test]
    fn from_code() {
        assert_eq!(Lang::from_code("ar-SA"), Some(Lang::Ar));
        assert_eq!(Lang::from_code("de_DE"), Some(Lang::De));
        assert_eq!(Lang::from_code("fr"), None);
    }
}
