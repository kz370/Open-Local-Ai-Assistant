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
