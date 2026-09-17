//! Detects requests that need fresh (post-training) information.

const PHRASES: &[&str] = &[
    // English
    "latest", "current version", "current price", "current status", "currently", "today", "tonight",
    "yesterday", "recently", "recent", "this week", "this month", "this year", "right now", "news",
    "latest release", "recent announcement", "up to date", "up-to-date", "as of now", "breaking",
    "stock price", "weather", "exchange rate",
    // German
    "neueste", "neuesten", "neuster", "aktuell", "aktuelle", "aktuellen", "aktueller", "heute",
    "kürzlich", "neulich", "diese woche", "diesen monat", "dieses jahr", "nachrichten", "derzeit",
    "zurzeit", "momentan", "gerade jetzt", "wetter", "kurs",
    // Arabic
    "أحدث", "آخر إصدار", "آخر الأخبار", "الحالي", "الحالية", "حاليا", "حالياً", "اليوم", "مؤخرا",
    "مؤخراً", "هذا الأسبوع", "هذا الشهر", "هذه السنة", "أخبار", "الأخبار", "الآن", "سعر", "الطقس",
];

/// Whole-word-ish match for Latin phrases, substring match for Arabic
/// (Arabic words take attached prefixes such as "ال" / "و").
pub fn needs_fresh_info(text: &str) -> bool {
    let lower = text.to_lowercase();
    PHRASES.iter().any(|p| {
        if p.chars().any(|c| c as u32 >= 0x0600) {
            return lower.contains(p);
        }
        let mut start = 0;
        while let Some(pos) = lower[start..].find(p) {
            let abs = start + pos;
            let before_ok = lower[..abs].chars().last().map_or(true, |c| !c.is_alphanumeric());
            let end = abs + p.len();
            let after_ok = lower[end..].chars().next().map_or(true, |c| !c.is_alphanumeric());
            if before_ok && after_ok {
                return true;
            }
            start = abs + p.len();
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_spec_indicators() {
        assert!(needs_fresh_info("What is the latest PHP version?"));
        assert!(needs_fresh_info("What's the latest news about NVIDIA?"));
        assert!(needs_fresh_info("current price of bitcoin"));
        assert!(needs_fresh_info("Was ist die aktuelle PHP-Version?"));
        assert!(needs_fresh_info("ما هو أحدث إصدار من PHP؟"));
        assert!(needs_fresh_info("ما هي الأخبار اليوم"));
    }

    #[test]
    fn ignores_general_knowledge() {
        assert!(!needs_fresh_info("Explain dependency injection in PHP."));
        assert!(!needs_fresh_info("Erkläre Dependency Injection"));
        assert!(!needs_fresh_info("Tell me about recentering algorithms")); // no partial word match
    }
}
