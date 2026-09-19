//! Live check that the built-in DuckDuckGo search still parses real results.
//! Run with: cargo test --test web_search_live -- --ignored --nocapture

use local_ai_assistant_lib::database::Db;
use local_ai_assistant_lib::services::search::WebSearch;
use local_ai_assistant_lib::settings::SettingsStore;
use std::sync::Arc;

#[tokio::test]
#[ignore]
async fn finds_results_for_a_real_query() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let settings = Arc::new(SettingsStore::load(db).unwrap());
    let search = WebSearch::new(settings);
    let results = search.search("rust programming language", 5).await.expect("search failed");
    for r in &results {
        println!("{} -> {}\n   {}", r.title, r.url, r.snippet);
    }
    assert!(!results.is_empty(), "DuckDuckGo returned no parsable results");
    assert!(results.iter().all(|r| r.url.starts_with("http")));
}

/// A SearXNG instance under a sub path (searxng.site serves /searxng/) that
/// only answers browser-like HTML requests.
#[tokio::test]
#[ignore]
async fn searches_a_searxng_instance() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let settings = Arc::new(SettingsStore::load(db).unwrap());
    settings
        .update(|s| {
            s.search.searxng_source = "local".into();
            s.search.searxng_url = "https://searxng.site".into();
        })
        .unwrap();
    let search = WebSearch::new(settings);
    let (results, engine) = search.search_uncached("rust programming language").await.expect("search failed");
    println!("{engine}: {} results", results.len());
    assert!(engine.starts_with("SearXNG"), "fell back to {engine}");
    assert!(!results.is_empty());
}

/// The searx.space list loads and an automatic pick finds results.
#[tokio::test]
#[ignore]
async fn searches_a_public_instance_from_searx_space() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let settings = Arc::new(SettingsStore::load(db).unwrap());
    settings.update(|s| s.search.searxng_source = "public".into()).unwrap();
    let search = WebSearch::new(settings);
    let list = search.public_instances(true).await.expect("list failed");
    println!("{} public instances, first {:?}", list.len(), list.first());
    let (results, engine) = search.search_uncached("rust programming language").await.expect("search failed");
    println!("{engine}: {} results", results.len());
    assert!(!results.is_empty());
}

/// With DuckDuckGo picked as primary it answers first; SearXNG is only the
/// fallback.
#[tokio::test]
#[ignore]
async fn primary_engine_goes_first() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let settings = Arc::new(SettingsStore::load(db).unwrap());
    settings
        .update(|s| {
            s.search.primary = "duckduckgo".into();
            s.search.searxng_url = "https://searxng.site".into();
        })
        .unwrap();
    let search = WebSearch::new(settings);
    let (results, engine) = search.search_uncached("rust programming language").await.expect("search failed");
    println!("{engine}: {} results", results.len());
    assert!(engine == "Built-in search (DuckDuckGo)", "answered by {engine}");
}
