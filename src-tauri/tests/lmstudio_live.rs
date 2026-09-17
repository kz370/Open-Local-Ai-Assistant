//! Live test against a running LM Studio server (skipped unless LA_LIVE_LMSTUDIO=1).
//!
//! Verifies with the real, automatically selected local model:
//! * streaming chat in English / Arabic / German with matching response language
//! * a "latest version" question triggers the MCP web-search tool and keeps sources
//! * a general-knowledge question does not use tools

use local_ai_assistant_lib::database::Db;
use local_ai_assistant_lib::services::ai::lmstudio::LmStudioService;
use local_ai_assistant_lib::services::chat::orchestrator::Emit;
use local_ai_assistant_lib::services::chat::tools::NoSpeech;
use local_ai_assistant_lib::services::chat::{ChatEngine, ChatEvent, ModelResolver, SendInput};
use local_ai_assistant_lib::services::hardware;
use local_ai_assistant_lib::services::language::detect;
use local_ai_assistant_lib::services::mcp::config::McpServerConfig;
use local_ai_assistant_lib::services::mcp::McpManager;
use local_ai_assistant_lib::settings::SettingsStore;
use std::sync::{Arc, Mutex};

struct Harness {
    engine: ChatEngine,
}

async fn harness() -> Option<Harness> {
    if std::env::var("LA_LIVE_LMSTUDIO").ok().as_deref() != Some("1") {
        eprintln!("LA_LIVE_LMSTUDIO not set; skipping live LM Studio test");
        return None;
    }
    let db = Arc::new(Db::open_in_memory().unwrap());
    let settings = Arc::new(SettingsStore::load(db.clone()).unwrap());
    settings.update(|s| s.ai.temperature = 0.2).unwrap();
    let ai = Arc::new(LmStudioService::new(&settings.get().ai.server_url, 300));
    let resolver = Arc::new(ModelResolver::with_hardware(ai.clone(), hardware::detect()));

    let cfg = db
        .save_mcp_server(&McpServerConfig {
            id: String::new(),
            name: "Web".into(),
            description: String::new(),
            transport: "stdio".into(),
            command: Some(env!("CARGO_BIN_EXE_mcp_fixture_server").into()),
            args: vec![],
            env: Default::default(),
            url: None,
            headers: Default::default(),
            enabled: false,
            source: "user".into(),
            created_at: String::new(),
        })
        .unwrap();
    let mcp = Arc::new(McpManager::new(db.clone(), Arc::new(|| {})));
    mcp.set_enabled(&cfg.id, true).await.unwrap();
    // Only expose the read-only search tool for this test.
    for tool in ["write_file", "fail"] {
        mcp.set_permission(&cfg.id, tool, local_ai_assistant_lib::services::chat::tools::Permission::Deny).await.unwrap();
    }
    Some(Harness { engine: ChatEngine::new(db, settings, ai, resolver, mcp, Arc::new(NoSpeech)) })
}

async fn ask(h: &Harness, text: &str) -> Vec<ChatEvent> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let e2 = events.clone();
    let emit: Emit = Arc::new(move |ev| e2.lock().unwrap().push(ev));
    let input = SendInput { turn_id: uuid::Uuid::new_v4().to_string(), conversation_id: None, text: text.into(), spoken_language: None, voice: false };
    let started = std::time::Instant::now();
    h.engine.send(input, emit).await.expect("turn");
    let ev = events.lock().unwrap().clone();
    let deltas = ev.iter().filter(|e| matches!(e, ChatEvent::Delta { .. })).count();
    eprintln!("[{text}] {} ms, {deltas} deltas", started.elapsed().as_millis());
    ev
}

fn final_message(ev: &[ChatEvent]) -> &local_ai_assistant_lib::database::conversations::Message {
    ev.iter()
        .find_map(|e| match e {
            ChatEvent::Done { message, .. } => Some(message),
            _ => None,
        })
        .expect("done event")
}

#[tokio::test]
async fn live_languages_and_web_search() {
    let Some(h) = harness().await else { return };

    for (q, lang) in [("In one short sentence: what is dependency injection?", "en"), ("اشرح باختصار ما هو حقن التبعية في جملة واحدة.", "ar"), ("Erkläre kurz in einem Satz, was Dependency Injection ist.", "de")] {
        let ev = ask(&h, q).await;
        let m = final_message(&ev);
        eprintln!("  -> {}", m.content);
        assert!(ev.iter().filter(|e| matches!(e, ChatEvent::Delta { .. })).count() > 1, "response should stream");
        assert_eq!(detect(&m.content).map(|d| d.lang.code()), Some(lang), "response language");
        assert!(!ev.iter().any(|e| matches!(e, ChatEvent::ToolStarted { .. })), "general knowledge should not use tools");
    }

    let ev = ask(&h, "What is the latest PHP version?").await;
    let m = final_message(&ev);
    eprintln!("  -> {}", m.content);
    assert!(ev.iter().any(|e| matches!(e, ChatEvent::ToolStarted { tool_name, .. } if tool_name == "web_search")), "latest-info question must use web search");
    let sources = m.sources.as_ref().expect("sources preserved");
    assert!(sources.to_string().contains("php.net"));
}
