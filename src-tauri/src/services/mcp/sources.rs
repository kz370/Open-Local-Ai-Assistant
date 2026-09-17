//! Extracts web sources (URLs + titles) that actually appear in tool results.

use crate::services::chat::tools::Source;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static MD_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]\n]{1,200})\]\((https?://[^\s)]+)\)").unwrap());
static TITLE_URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?im)^\s*(?:title|name)\s*:\s*(.+?)\s*$\s*^\s*(?:url|link|href)\s*:\s*(https?://\S+)").unwrap());
static BARE_URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"https?://[^\s<>"'\])},]+"#).unwrap());

const MAX_SOURCES: usize = 12;

fn push(out: &mut Vec<Source>, url: &str, title: Option<&str>) {
    let url = url.trim_end_matches(['.', ',', ';', ':', '!', '?', ')']).to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) || url.len() > 2048 {
        return;
    }
    if let Some(existing) = out.iter_mut().find(|s| s.url == url) {
        if existing.title.is_none() {
            existing.title = title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        }
        return;
    }
    if out.len() < MAX_SOURCES {
        out.push(Source { url, title: title.map(|t| t.trim().chars().take(200).collect()).filter(|t: &String| !t.is_empty()) });
    }
}

fn walk_json(v: &Value, out: &mut Vec<Source>) {
    match v {
        Value::Object(map) => {
            let url = ["url", "link", "href", "uri"].iter().find_map(|k| map.get(*k).and_then(Value::as_str));
            if let Some(url) = url {
                let title = ["title", "name", "headline"].iter().find_map(|k| map.get(*k).and_then(Value::as_str));
                push(out, url, title);
            }
            for child in map.values() {
                walk_json(child, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| walk_json(i, out)),
        Value::String(s) => {
            // Tool servers often return JSON encoded inside a text block.
            let t = s.trim_start();
            if t.starts_with('{') || t.starts_with('[') {
                if let Ok(inner) = serde_json::from_str::<Value>(t) {
                    walk_json(&inner, out);
                }
            }
        }
        _ => {}
    }
}

pub fn extract_sources(text: &str, structured: Option<&Value>) -> Vec<Source> {
    let mut out = Vec::new();
    if let Some(v) = structured {
        walk_json(v, &mut out);
    }
    walk_json(&Value::String(text.to_string()), &mut out);
    for c in TITLE_URL.captures_iter(text) {
        push(&mut out, &c[2], Some(&c[1]));
    }
    for c in MD_LINK.captures_iter(text) {
        push(&mut out, &c[2], Some(&c[1]));
    }
    for m in BARE_URL.find_iter(text) {
        push(&mut out, m.as_str(), None);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_from_various_formats() {
        let text = "Title: PHP 8.5 Released\nURL: https://www.php.net/releases/8.5/\n\nSee also [RFC list](https://wiki.php.net/rfc). And https://example.com/a.";
        let s = extract_sources(text, None);
        assert_eq!(s[0].url, "https://www.php.net/releases/8.5/");
        assert_eq!(s[0].title.as_deref(), Some("PHP 8.5 Released"));
        assert!(s.iter().any(|x| x.url == "https://wiki.php.net/rfc" && x.title.as_deref() == Some("RFC list")));
        assert!(s.iter().any(|x| x.url == "https://example.com/a"));
    }

    #[test]
    fn json_results() {
        let json = r#"{"results":[{"title":"NVIDIA news","url":"https://nvidianews.nvidia.com/"},{"title":"Dup","url":"https://nvidianews.nvidia.com/"}]}"#;
        let s = extract_sources(json, None);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].title.as_deref(), Some("NVIDIA news"));
    }

    #[test]
    fn ignores_non_http() {
        assert!(extract_sources("file:///etc/passwd javascript:alert(1)", None).is_empty());
    }
}
