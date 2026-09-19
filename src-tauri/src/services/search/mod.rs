//! Built-in web search.
//!
//! Search has to work out of the box, so this is a plain HTTPS call to
//! DuckDuckGo's HTML endpoint rather than an MCP server: no API key, no
//! account, and no Node/Python runtime the user would have to install first.
//! A SearXNG instance (the user's own, or a public one from searx.space) is
//! asked first when configured.
//! It is exposed to the model as an ordinary tool (`web_search`), so the
//! orchestrator, permissions and citations treat it like any MCP search tool.

use crate::errors::{AppError, AppResult};
use crate::services::chat::tools::{Permission, Source, ToolCategory, ToolOutput, ToolProvider, ToolSpec};
use crate::settings::{SearchSettings, SettingsStore};
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
/// searx.space's machine-readable list of public SearXNG instances.
const INSTANCES_URL: &str = "https://searx.space/data/instances.json";
/// searx.space re-checks instances every few hours, so the list keeps a while.
const INSTANCES_TTL: Duration = Duration::from_secs(3 * 3600);
/// How many public instances an automatic pick tries before DuckDuckGo.
const AUTO_INSTANCES: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// One entry of the searx.space list, as shown in settings.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicInstance {
    pub url: String,
    /// Share of searx.space's test searches that returned results (0-100).
    pub search_success: f64,
    /// Seconds a test search took, when known.
    pub search_time: Option<f64>,
    pub version: Option<String>,
}

pub struct WebSearch {
    http: reqwest::Client,
    settings: Arc<SettingsStore>,
    cache: Mutex<HashMap<String, (Instant, Vec<SearchResult>)>>,
    instances: Mutex<Option<(Instant, Vec<PublicInstance>)>>,
    /// Search endpoint found for each SearXNG base URL (some live under a
    /// sub path), so the discovery round trip happens once.
    endpoints: Mutex<HashMap<String, url::Url>>,
}

impl WebSearch {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        Self {
            http,
            settings,
            cache: Mutex::new(HashMap::new()),
            instances: Mutex::new(None),
            endpoints: Mutex::new(HashMap::new()),
        }
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            llm_name: TOOL_NAME.into(),
            server_id: SERVER_ID.into(),
            server_name: SERVER_NAME.into(),
            tool_name: TOOL_NAME.into(),
            description: "Search the web and return titles, URLs and snippets of the top results. \
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
        let (results, _) = self.search_uncached(query).await?;
        self.remember(query, &results);
        Ok(results.into_iter().take(max_results).collect())
    }

    /// Runs one search without the cache and names the engine that answered,
    /// so the settings page can test the current setup.
    pub async fn search_uncached(&self, query: &str) -> AppResult<(Vec<SearchResult>, String)> {
        // DuckDuckGo and SearXNG are switched on independently; with both on,
        // the one picked as primary goes first and the other is the fallback.
        let search = self.settings.get().search;
        let mut errors = Vec::new();
        for engine in engine_order(&search) {
            let outcome = match engine {
                Engine::Searxng => self.search_searxng_any(&search, query).await,
                Engine::DuckDuckGo => self.search_duckduckgo(query).await.map(|r| (r, "DuckDuckGo".to_string())),
            };
            match outcome {
                Ok(found) => return Ok(found),
                Err(e) => errors.push(format!("{}: {e}", engine.name())),
            }
        }
        if errors.is_empty() {
            return Err(AppError::Invalid("no search engine is switched on".into()));
        }
        Err(AppError::Other(format!("web search failed ({})", errors.join("; "))))
    }

    /// Tries the configured SearXNG instances in turn.
    async fn search_searxng_any(&self, search: &SearchSettings, query: &str) -> AppResult<(Vec<SearchResult>, String)> {
        let candidates = self.searxng_candidates(search).await;
        if candidates.is_empty() {
            return Err(AppError::Invalid(if search.searxng_source == "public" {
                "the searx.space list could not be loaded".into()
            } else {
                "no instance address is set".into()
            }));
        }
        let mut last = String::new();
        for instance in candidates {
            match self.search_searxng(&instance, query).await {
                Ok(results) if !results.is_empty() => return Ok((results, format!("SearXNG ({instance})"))),
                Ok(_) => {
                    tracing::info!(%instance, "searxng returned no results");
                    last = format!("{instance} returned no results");
                }
                Err(e) => {
                    tracing::warn!(%instance, error = %e, "searxng search failed");
                    last = format!("{instance}: {e}");
                }
            }
        }
        Err(AppError::Other(last))
    }

    async fn search_duckduckgo(&self, query: &str) -> AppResult<Vec<SearchResult>> {
        // A challenge on one endpoint does not mean the next one refuses too,
        // and a short pause is usually enough for the engine to answer again.
        let mut last = AppError::Other("no results".into());
        for (attempt, endpoint) in ENDPOINTS.iter().enumerate() {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(1200)).await;
            }
            match self.fetch(query, endpoint).await {
                Ok(body) => {
                    let results = parse_results(&body, 10);
                    if !results.is_empty() {
                        return Ok(results);
                    }
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// The SearXNG instances to try, in order, for the current settings.
    async fn searxng_candidates(&self, search: &SearchSettings) -> Vec<String> {
        if search.searxng_source == "public" {
            let chosen = search.searxng_public_url.trim();
            if !chosen.is_empty() {
                return vec![chosen.to_string()];
            }
            // Automatic: the fastest few from the list, so one instance that is
            // down or rate limiting does not end the search.
            return match self.public_instances(false).await {
                Ok(list) => list.into_iter().take(AUTO_INSTANCES).map(|i| i.url).collect(),
                Err(e) => {
                    tracing::warn!(error = %e, "could not load the searx.space instance list");
                    Vec::new()
                }
            };
        }
        let local = search.searxng_url.trim();
        if local.is_empty() {
            Vec::new()
        } else {
            vec![local.to_string()]
        }
    }

    /// Public SearXNG instances from searx.space that currently answer
    /// searches, fastest first. Kept for a while; `refresh` fetches anew.
    pub async fn public_instances(&self, refresh: bool) -> AppResult<Vec<PublicInstance>> {
        if !refresh {
            let cached = self.instances.lock().unwrap_or_else(|p| p.into_inner());
            if let Some((at, list)) = cached.as_ref() {
                if at.elapsed() < INSTANCES_TTL {
                    return Ok(list.clone());
                }
            }
        }
        let body = self
            .http
            .get(INSTANCES_URL)
            .header("accept", "application/json")
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| AppError::Other(format!("could not load the searx.space list: {e}")))?
            .text()
            .await
            .map_err(|e| AppError::Other(format!("could not load the searx.space list: {e}")))?;
        let list = parse_instances(&body);
        if list.is_empty() {
            return Err(AppError::Other("the searx.space list has no working instances right now".into()));
        }
        *self.instances.lock().unwrap_or_else(|p| p.into_inner()) = Some((Instant::now(), list.clone()));
        Ok(list)
    }

    /// Searches one SearXNG instance through its normal result page. The JSON
    /// API is off on almost every instance (public ones and a fresh local
    /// install alike), while the HTML page always answers.
    async fn search_searxng(&self, instance: &str, query: &str) -> AppResult<Vec<SearchResult>> {
        let base = instance_base(instance)?;
        let known = self.endpoints.lock().unwrap_or_else(|p| p.into_inner()).get(base.as_str()).cloned();
        let mut endpoint = known.unwrap_or_else(|| base.join("search").expect("relative path"));
        // One retry: an instance under a sub path (searxng.site serves from
        // /searxng/) bounces /search to a home page whose search form names
        // the real endpoint.
        for _ in 0..2 {
            let (status, final_url, body) = self.fetch_searxng(&endpoint, &base, query).await?;
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(AppError::Other("the instance is rate limiting requests (HTTP 429), try another one".into()));
            }
            if !status.is_success() {
                return Err(AppError::Other(format!("the instance answered HTTP {}", status.as_u16())));
            }
            let results = if body.trim_start().starts_with('{') { parse_searxng(&body) } else { parse_searxng_html(&body) };
            if !results.is_empty() || is_result_page(&body) {
                self.endpoints.lock().unwrap_or_else(|p| p.into_inner()).insert(base.to_string(), endpoint);
                return Ok(results);
            }
            match form_action(&body).and_then(|a| final_url.join(&a).ok()) {
                Some(real) if real != endpoint => endpoint = real,
                _ => break,
            }
        }
        Err(AppError::Other("this address did not return a SearXNG result page, check the URL".into()))
    }

    async fn fetch_searxng(&self, endpoint: &url::Url, base: &url::Url, query: &str) -> AppResult<(reqwest::StatusCode, url::Url, String)> {
        let mut url = endpoint.clone();
        url.query_pairs_mut().append_pair("q", query).append_pair("safesearch", "0");
        // SearXNG's bot detection turns away clients without browser headers
        // (Accept-Language, Accept-Encoding and the Sec-Fetch set).
        let resp = self
            .http
            .get(url)
            .header("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("accept-language", "en-US,en;q=0.9")
            .header("referer", base.as_str())
            .header("sec-fetch-dest", "document")
            .header("sec-fetch-mode", "navigate")
            .header("sec-fetch-site", "same-origin")
            .header("sec-fetch-user", "?1")
            .header("upgrade-insecure-requests", "1")
            .send()
            .await
            .map_err(|e| AppError::Other(format!("could not reach the instance: {e}")))?;
        let status = resp.status();
        let final_url = resp.url().clone();
        let body = resp.text().await.map_err(|e| AppError::Other(format!("could not read the answer: {e}")))?;
        Ok((status, final_url, body))
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
        if !engine_order(&self.settings.get().search).is_empty() {
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum Engine {
    Searxng,
    DuckDuckGo,
}

impl Engine {
    fn name(self) -> &'static str {
        match self {
            Engine::Searxng => "SearXNG",
            Engine::DuckDuckGo => "DuckDuckGo",
        }
    }
}

/// The switched-on engines, primary first.
fn engine_order(search: &SearchSettings) -> Vec<Engine> {
    let mut order = Vec::new();
    if search.searxng_enabled {
        order.push(Engine::Searxng);
    }
    if search.enabled {
        order.push(Engine::DuckDuckGo);
    }
    if search.primary == "duckduckgo" {
        order.reverse();
    }
    order
}

/// Reads the result list of a SearXNG HTML page (the "simple" theme wraps
/// each hit in `<article class="result ...">`, older themes in a div).
fn parse_searxng_html(html: &str) -> Vec<SearchResult> {
    let title = regex::Regex::new(r#"(?s)<h3[^>]*>\s*<a[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#).expect("valid regex");
    let content = regex::Regex::new(r#"(?s)<p class="content">(.*?)</p>"#).expect("valid regex");
    html.split(r#"class="result result-"#)
        .skip(1)
        .filter_map(|block| {
            let block = block.split("</article>").next().unwrap_or(block);
            let c = title.captures(block)?;
            let url = c[1].replace("&amp;", "&");
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return None;
            }
            let title = clean_text(&c[2]);
            if title.is_empty() {
                return None;
            }
            let snippet = content.captures(block).map(|s| clean_text(&s[1])).unwrap_or_default();
            Some(SearchResult { title, url, snippet })
        })
        .collect()
}

/// True for a SearXNG result page, even one with no hits (so "nothing found"
/// is not mistaken for a wrong address).
fn is_result_page(html: &str) -> bool {
    html.contains(r#"id="results""#) || html.contains(r#"id="urls""#)
}

/// The action of the page's search form, e.g. "/searxng/search".
fn form_action(html: &str) -> Option<String> {
    let form = regex::Regex::new(r#"<form[^>]*id="search"[^>]*>"#).expect("valid regex");
    let action = regex::Regex::new(r#"action="([^"]+)""#).expect("valid regex");
    let tag = form.find(html)?.as_str();
    Some(action.captures(tag)?[1].replace("&amp;", "&"))
}

/// Normalises what the user typed into a base URL ending in "/": a missing
/// scheme means http for local addresses and https for everything else.
fn instance_base(instance: &str) -> AppResult<url::Url> {
    let raw = instance.trim();
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        let host = raw.split(['/', ':']).next().unwrap_or("");
        let local = host == "localhost" || host.starts_with("127.") || host.starts_with("192.168.") || host.starts_with("10.") || host.ends_with(".local");
        format!("{}://{raw}", if local { "http" } else { "https" })
    };
    let mut url = url::Url::parse(&with_scheme).map_err(|_| AppError::Invalid("the SearXNG address is not a valid URL".into()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::Invalid("the SearXNG address must start with http:// or https://".into()));
    }
    url.set_query(None);
    url.set_fragment(None);
    // "…/search" pasted from the address bar still means the instance itself.
    let path = url.path().trim_end_matches('/').trim_end_matches("/search").to_string();
    url.set_path(&format!("{path}/"));
    Ok(url)
}

/// Picks the usable public instances out of searx.space's instances.json:
/// plain web (no Tor), answering, and passing most test searches. Fastest
/// first.
fn parse_instances(body: &str) -> Vec<PublicInstance> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else { return Vec::new() };
    let Some(map) = json.get("instances").and_then(|i| i.as_object()) else { return Vec::new() };
    let mut list: Vec<PublicInstance> = map
        .iter()
        .filter_map(|(url, v)| {
            if v.get("network_type").and_then(|n| n.as_str()) != Some("normal") {
                return None;
            }
            if v.pointer("/http/status_code").and_then(|s| s.as_u64()) != Some(200) {
                return None;
            }
            let success = v.pointer("/timing/search/success_percentage").and_then(|s| s.as_f64()).unwrap_or(0.0);
            if success < 50.0 {
                return None;
            }
            Some(PublicInstance {
                url: url.clone(),
                search_success: success,
                search_time: v.pointer("/timing/search/all/value").and_then(|s| s.as_f64()),
                version: v.get("version").and_then(|s| s.as_str()).map(str::to_string),
            })
        })
        .collect();
    list.sort_by(|a, b| {
        b.search_success
            .partial_cmp(&a.search_success)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.search_time.unwrap_or(f64::MAX).partial_cmp(&b.search_time.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal))
    });
    list
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

    /// Trimmed from a real searxng.site result page.
    const SEARXNG_PAGE: &str = r##"
    <form id="search" method="GET" action="/searxng/search" role="search"></form>
    <div id="results"><div id="urls" role="main">
    <article class="result result-default category-general"><a href="https://www.python.org/" class="url_header" rel="noreferrer"><div class="url_wrapper">x</div></a><h3><a href="https://www.python.org/?a=1&amp;b=2" rel="noreferrer">Welcome to <span class="highlight">Python</span>.org</a></h3><time class="published_date" datetime="" ></time>  <p class="content">
        The official home of the <span class="highlight">Python</span> Programming Language
      </p><div class="engines"><span>bing</span></div></article>
    <article class="result result-images category-images"><h3><a href="javascript:void(0)">bad</a></h3></article>
    <article class="result result-default category-general"><h3><a href="https://docs.python.org/3/" rel="noreferrer">Python docs</a></h3></article>
    </div></div>"##;

    #[test]
    fn reads_searxng_html() {
        let r = parse_searxng_html(SEARXNG_PAGE);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].url, "https://www.python.org/?a=1&b=2");
        assert_eq!(r[0].title, "Welcome to Python.org");
        assert_eq!(r[0].snippet, "The official home of the Python Programming Language");
        assert_eq!(r[1].snippet, "");
        assert!(is_result_page(SEARXNG_PAGE));
        assert!(!is_result_page("<form id=\"search\" action=\"/searxng/search\"></form>"));
    }

    #[test]
    fn finds_the_search_form_action() {
        assert_eq!(form_action(SEARXNG_PAGE).as_deref(), Some("/searxng/search"));
        assert_eq!(form_action("<html></html>"), None);
    }

    #[test]
    fn normalises_instance_addresses() {
        assert_eq!(instance_base("https://searxng.site").unwrap().as_str(), "https://searxng.site/");
        assert_eq!(instance_base("https://searxng.site/searxng").unwrap().as_str(), "https://searxng.site/searxng/");
        assert_eq!(instance_base("https://searxng.site/searxng/search?q=x").unwrap().as_str(), "https://searxng.site/searxng/");
        assert_eq!(instance_base("localhost:8080").unwrap().as_str(), "http://localhost:8080/");
        assert_eq!(instance_base("searx.be").unwrap().as_str(), "https://searx.be/");
        assert!(instance_base("ftp://x").is_err());
    }

    #[test]
    fn keeps_only_working_public_instances() {
        let body = r#"{"instances":{
            "https://slow.example/":{"network_type":"normal","http":{"status_code":200},"timing":{"search":{"success_percentage":100.0,"all":{"value":1.5}}},"version":"2026.1"},
            "https://fast.example/":{"network_type":"normal","http":{"status_code":200},"timing":{"search":{"success_percentage":100.0,"all":{"value":0.4}}}},
            "https://broken.example/":{"network_type":"normal","http":{"status_code":200},"timing":{"search":{"success_percentage":0.0,"all":null}}},
            "http://x.onion/":{"network_type":"tor","http":{"status_code":200},"timing":{"search":{"success_percentage":100.0}}},
            "https://down.example/":{"network_type":"normal","http":{"status_code":502},"timing":{}}
        }}"#;
        let list = parse_instances(body);
        let urls: Vec<_> = list.iter().map(|i| i.url.as_str()).collect();
        assert_eq!(urls, ["https://fast.example/", "https://slow.example/"]);
        assert_eq!(list[1].version.as_deref(), Some("2026.1"));
        assert!(parse_instances("nope").is_empty());
    }

    #[test]
    fn engines_are_independent() {
        let mut s = SearchSettings { enabled: false, searxng_enabled: true, ..Default::default() };
        assert_eq!(engine_order(&s), [Engine::Searxng], "SearXNG must work with DuckDuckGo off");
        s.enabled = true;
        assert_eq!(engine_order(&s), [Engine::Searxng, Engine::DuckDuckGo]);
        s.primary = "duckduckgo".into();
        assert_eq!(engine_order(&s), [Engine::DuckDuckGo, Engine::Searxng]);
        s.searxng_enabled = false;
        assert_eq!(engine_order(&s), [Engine::DuckDuckGo]);
        s.enabled = false;
        assert!(engine_order(&s).is_empty());
    }

    #[test]
    fn rejects_non_http_links() {
        assert_eq!(real_url("javascript:alert(1)"), "");
        assert_eq!(real_url("https://example.com/a?b=c"), "https://example.com/a?b=c");
    }
}
