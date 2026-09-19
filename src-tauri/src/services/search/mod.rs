//! Built-in web search.
//!
//! Search has to work out of the box, so this is a plain HTTPS call to
//! DuckDuckGo's HTML endpoint rather than an MCP server: no API key, no
//! account, and no Node/Python runtime the user would have to install first.
//! It is exposed to the model as an ordinary tool (`web_search`), so the
//! orchestrator, permissions and citations treat it like any MCP search tool.

use crate::errors::{AppError, AppResult};
use crate::services::chat::tools::{Permission, Source, ToolCategory, ToolOutput, ToolProvider, ToolSpec};
use crate::settings::SettingsStore;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const SERVER_ID: &str = "builtin-web-search";
pub const SERVER_NAME: &str = "Web Search";
pub const TOOL_NAME: &str = "web_search";

/// The no-JavaScript result page, and the classic one as a second try. Both
/// answer with a bot challenge when they see too many requests at once, so a
/// search falls back from one to the other before giving up.
const ENDPOINTS: [&str; 2] = ["https://lite.duckduckgo.com/lite/", "https://html.duckduckgo.com/html/"];
/// Repeat searches inside this window are answered from memory instead of
/// hitting the engine again (models like to retry the same query).
const CACHE_TTL: Duration = Duration::from_secs(600);
/// A plain desktop browser agent; the HTML endpoint serves an empty page to
/// clients that do not look like a browser.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";
const MAX_SNIPPET: usize = 320;

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebSearch {
    http: reqwest::Client,
    settings: Arc<SettingsStore>,
    cache: Mutex<HashMap<String, (Instant, Vec<SearchResult>)>>,
}

impl WebSearch {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        Self { http, settings, cache: Mutex::new(HashMap::new()) }
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            llm_name: TOOL_NAME.into(),
            server_id: SERVER_ID.into(),
            server_name: SERVER_NAME.into(),
            tool_name: TOOL_NAME.into(),
            description: "Search the web (DuckDuckGo) and return titles, URLs and snippets of the top results. \
                          Use it for current, recent or unfamiliar information, then cite the URLs you used."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What to search for." },
                    "maxResults": { "type": "integer", "description": "How many results to return (1-10).", "minimum": 1, "maximum": 10 }
                },
                "required": ["query"]
            }),
            category: ToolCategory::Search,
            permission: Permission::Allow,
        }
    }

    /// The raw result page, exposed for diagnostics.
    pub async fn raw_html(&self, query: &str) -> AppResult<String> {
        self.fetch(query, ENDPOINTS[0]).await
    }

    async fn fetch(&self, query: &str, endpoint: &str) -> AppResult<String> {
        let mut endpoint = url::Url::parse(endpoint).expect("valid endpoint");
        endpoint.query_pairs_mut().append_pair("q", query).append_pair("kl", "wt-wt");
        // Browser-shaped headers: the endpoint answers bare HTTP clients with a
        // captcha page instead of results.
        let body = self
            .http
            .get(endpoint)
            .header("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("accept-language", "en-US,en;q=0.9")
            .header("referer", "https://lite.duckduckgo.com/")
            .header("upgrade-insecure-requests", "1")
            .header("sec-fetch-dest", "document")
            .header("sec-fetch-mode", "navigate")
            .header("sec-fetch-site", "same-origin")
            .header("sec-fetch-user", "?1")
            .send()
            .await
            .map_err(|e| AppError::Other(format!("web search failed: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Other(format!("web search failed: {e}")))?
            .text()
            .await
            .map_err(|e| AppError::Other(format!("web search failed: {e}")))?;
        if body.contains("anomaly-modal") || body.contains("challenge-form") {
            return Err(AppError::Other("the search engine asked for a captcha, so this search could not run".into()));
        }
        Ok(body)
    }

    pub async fn search(&self, query: &str, max_results: usize) -> AppResult<Vec<SearchResult>> {
        if let Some(hit) = self.cached(query) {
            return Ok(hit.into_iter().take(max_results).collect());
        }
        // A SearXNG instance the user runs or trusts always wins: it answers
        // JSON and never shows a captcha.
        let search = self.settings.get().search;
        let instance = search.searxng_url.trim().to_string();
        if search.searxng_enabled && !instance.is_empty() {
            match self.search_searxng(&instance, query).await {
                Ok(results) if !results.is_empty() => {
                    self.remember(query, &results);
                    return Ok(results.into_iter().take(max_results).collect());
                }
                Ok(_) => tracing::info!("searxng returned no results, falling back to duckduckgo"),
                Err(e) => tracing::warn!(error = %e, "searxng search failed, falling back to duckduckgo"),
            }
        }
        // A challenge on one endpoint does not mean the next one refuses too,
        // and a short pause is usually enough for the engine to answer again.
        let mut last = AppError::Other("web search produced no results".into());
        for (attempt, endpoint) in ENDPOINTS.iter().enumerate() {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(1200)).await;
            }
            match self.fetch(query, endpoint).await {
                Ok(body) => {
                    let results = parse_results(&body, 10);
                    if !results.is_empty() {
                        self.remember(query, &results);
                        return Ok(results.into_iter().take(max_results).collect());
                    }
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Queries a SearXNG instance through its JSON API.
    async fn search_searxng(&self, instance: &str, query: &str) -> AppResult<Vec<SearchResult>> {
        let base = instance.trim_end_matches('/');
        let mut endpoint = url::Url::parse(&format!("{base}/search")).map_err(|_| AppError::Invalid("the SearXNG address is not a valid URL".into()))?;
        endpoint
            .query_pairs_mut()
            .append_pair("q", query)
            .append_pair("format", "json")
            .append_pair("safesearch", "0");
        let body = self
            .http
            .get(endpoint)
            .header("accept", "application/json")
            .send()
            .await
            .map_err(|e| AppError::Other(format!("web search failed: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Other(format!("web search failed: {e}")))?
            .text()
            .await
            .map_err(|e| AppError::Other(format!("web search failed: {e}")))?;
        Ok(parse_searxng(&body))
    }

    fn cached(&self, query: &str) -> Option<Vec<SearchResult>> {
        let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        cache.retain(|_, (at, _)| at.elapsed() < CACHE_TTL);
        cache.get(query).map(|(_, r)| r.clone())
    }

    fn remember(&self, query: &str, results: &[SearchResult]) {
        let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        cache.insert(query.to_string(), (Instant::now(), results.to_vec()));
    }
}

#[async_trait]
impl ToolProvider for WebSearch {
    async fn available_tools(&self) -> Vec<ToolSpec> {
        if self.settings.get().search.enabled {
            vec![self.spec()]
        } else {
            Vec::new()
        }
    }

    async fn call_tool(&self, spec: &ToolSpec, args: serde_json::Value) -> AppResult<ToolOutput> {
        if spec.server_id != SERVER_ID {
            return Err(AppError::Mcp(format!("tool {} unavailable", spec.llm_name)));
        }
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        if query.is_empty() {
            return Err(AppError::Invalid("query is required".into()));
        }
        let configured = self.settings.get().search.max_results as usize;
        let max = args
            .get("maxResults")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(configured)
            .clamp(1, 10);

        let results = self.search(&query, max).await?;
        if results.is_empty() {
            return Ok(ToolOutput { text: format!("No results for \"{query}\"."), is_error: false, sources: Vec::new() });
        }
        let mut text = format!("Results for \"{query}\":\n");
        for (i, r) in results.iter().enumerate() {
            text.push_str(&format!("\n{}. {}\n   {}\n   {}\n", i + 1, r.title, r.url, r.snippet));
        }
        let sources = results
            .iter()
            .map(|r| Source { url: r.url.clone(), title: Some(r.title.clone()) })
            .collect();
        Ok(ToolOutput { text, is_error: false, sources })
    }
}

/// Pulls the result list out of a DuckDuckGo result page. Handles both the
/// lite page (`result-link` anchors, snippets in table cells) and the classic
/// HTML page (`result__a`), so a change of endpoint does not break search.
fn parse_results(html: &str, max: usize) -> Vec<SearchResult> {
    let link = regex::Regex::new(r#"(?s)<a[^>]*href=['"]([^'"]+)['"][^>]*class=['"][^'"]*result(?:-link|__a)[^'"]*['"][^>]*>(.*?)</a>"#).expect("valid regex");
    let snippet = regex::Regex::new(r#"(?s)<(?:td|a)[^>]*class=['"][^'"]*result(?:-snippet|__snippet)[^'"]*['"][^>]*>(.*?)</(?:td|a)>"#).expect("valid regex");
    let snippets: Vec<String> = snippet.captures_iter(html).map(|c| clean_text(&c[1])).collect();

    link.captures_iter(html)
        .enumerate()
        .filter_map(|(i, c)| {
            let url = real_url(&c[1]);
            let title = clean_text(&c[2]);
            if url.is_empty() || title.is_empty() {
                return None;
            }
            Some(SearchResult { title, url, snippet: snippets.get(i).cloned().unwrap_or_default() })
        })
        .take(max)
        .collect()
}

/// DuckDuckGo wraps every hit in a redirect (`/l/?uddg=<encoded target>`).
fn real_url(href: &str) -> String {
    let absolute = if href.starts_with("//") { format!("https:{href}") } else { href.to_string() };
    let Ok(parsed) = url::Url::parse(&absolute) else { return String::new() };
    if let Some((_, target)) = parsed.query_pairs().find(|(k, _)| k == "uddg") {
        return target.to_string();
    }
    match parsed.scheme() {
        "http" | "https" => absolute,
        _ => String::new(),
    }
}

/// Strips tags and decodes the few entities DuckDuckGo emits.
fn clean_text(raw: &str) -> String {
    let tags = regex::Regex::new(r"<[^>]*>").expect("valid regex");
    let text = tags.replace_all(raw, "");
    let text = text
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > MAX_SNIPPET {
        text.chars().take(MAX_SNIPPET).collect::<String>() + "…"
    } else {
        text
    }
}

/// Reads the result list of a SearXNG JSON response.
fn parse_searxng(body: &str) -> Vec<SearchResult> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else { return Vec::new() };
    let Some(items) = json.get("results").and_then(|r| r.as_array()) else { return Vec::new() };
    items
        .iter()
        .filter_map(|r| {
            let url = r.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if url.is_empty() || title.is_empty() {
                return None;
            }
            let snippet = clean_text(r.get("content").and_then(|v| v.as_str()).unwrap_or(""));
            Some(SearchResult { title, url, snippet })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped like the lite page: href first, single-quoted classes, snippets
    /// in their own table cell.
    const PAGE: &str = r##"
    <tr><td><a rel="nofollow" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust%2Dlang.org%2F&amp;rut=x" class='result-link'>Rust <b>lang</b></a></td></tr>
    <tr><td class='result-snippet'>A language empowering&nbsp;everyone.</td></tr>
    <tr><td><a rel="nofollow" href="https://doc.rust-lang.org/book/" class='result-link'>The Book</a></td></tr>
    <tr><td class='result-snippet'>Learn Rust&#x27;s basics.</td></tr>"##;

    /// The classic page, still parsed so a fallback endpoint keeps working.
    const CLASSIC_PAGE: &str = r##"
    <div class="result results_links">
      <a rel="nofollow" href="https://doc.rust-lang.org/book/" class="result__a">The Book</a>
      <a class="result__snippet" href="https://doc.rust-lang.org/book/">Learn Rust basics.</a>
    </div>"##;

    #[test]
    fn parses_titles_urls_and_snippets() {
        let r = parse_results(PAGE, 10);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].title, "Rust lang");
        assert_eq!(r[0].url, "https://www.rust-lang.org/", "the redirect wrapper must be unwrapped");
        assert_eq!(r[0].snippet, "A language empowering everyone.");
        assert_eq!(r[1].url, "https://doc.rust-lang.org/book/");
        assert_eq!(r[1].snippet, "Learn Rust's basics.");
    }

    #[test]
    fn parses_the_classic_page_too() {
        let r = parse_results(CLASSIC_PAGE, 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].url, "https://doc.rust-lang.org/book/");
        assert_eq!(r[0].snippet, "Learn Rust basics.");
    }

    #[test]
    fn honours_the_result_limit() {
        assert_eq!(parse_results(PAGE, 1).len(), 1);
        assert!(parse_results("<html>nothing here</html>", 5).is_empty());
    }

    #[test]
    fn reads_searxng_json() {
        let body = r#"{"results":[{"url":"https://example.com","title":"Example","content":"An <b>example</b> page."},{"url":"","title":"broken"}]}"#;
        let r = parse_searxng(body);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].url, "https://example.com");
        assert_eq!(r[0].snippet, "An example page.");
        assert!(parse_searxng("not json").is_empty());
    }

    #[test]
    fn rejects_non_http_links() {
        assert_eq!(real_url("javascript:alert(1)"), "");
        assert_eq!(real_url("https://example.com/a?b=c"), "https://example.com/a?b=c");
    }
}
