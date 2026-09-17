//! Reads a real database file (LA_DB) the way the History panel does.
use local_ai_assistant_lib::database::Db;

#[test]
fn search_lists_conversations() {
    let Ok(path) = std::env::var("LA_DB") else {
        eprintln!("LA_DB not set; skipping");
        return;
    };
    let db = Db::open(std::path::Path::new(&path)).expect("open db");
    let all = db.list_conversations(100, 0).unwrap();
    eprintln!("list_conversations -> {}", all.len());
    let hits = db.search_conversations("", 100).unwrap();
    eprintln!("search('') -> {}", hits.len());
    for h in hits.iter().take(5) {
        eprintln!("  {} | {}", h.conversation.title, h.conversation.updated_at);
    }
    let php = db.search_conversations("dependency", 100).unwrap();
    eprintln!("search('dependency') -> {}", php.len());
    assert_eq!(all.len(), hits.len());
    assert!(!hits.is_empty());
}
