//! Spawns the fixture MCP server over stdio and exercises the real manager.

use local_ai_assistant_lib::database::Db;
use local_ai_assistant_lib::services::chat::tools::{Permission, ToolCategory, ToolProvider};
use local_ai_assistant_lib::services::mcp::config::McpServerConfig;
use local_ai_assistant_lib::services::mcp::McpManager;
use std::sync::Arc;

fn fixture_config(enabled: bool) -> McpServerConfig {
    McpServerConfig {
        id: String::new(),
        name: "Fixture".into(),
        description: String::new(),
        transport: "stdio".into(),
        command: Some(env!("CARGO_BIN_EXE_mcp_fixture_server").into()),
        args: vec![],
        env: Default::default(),
        url: None,
        headers: Default::default(),
        enabled,
        source: "user".into(),
        created_at: String::new(),
    }
}

#[tokio::test]
async fn discovery_invocation_permissions() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let cfg = db.save_mcp_server(&fixture_config(false)).unwrap();
    let mgr = Arc::new(McpManager::new(db.clone(), Arc::new(|| {})));

    // Disabled servers are never connected and expose no tools.
    mgr.connect_enabled().await;
    assert!(mgr.available_tools().await.is_empty());
    assert_eq!(mgr.statuses().await.unwrap()[0].state, "disabled");

    mgr.set_enabled(&cfg.id, true).await.unwrap();
    let status = mgr.statuses().await.unwrap().remove(0);
    assert_eq!(status.state, "connected", "{:?}", status.error);
    assert_eq!(status.tools.len(), 4);
    let perm = |n: &str| status.tools.iter().find(|t| t.name == n).unwrap().permission;
    assert_eq!(perm("web_search"), Permission::Allow);
    assert_eq!(perm("write_file"), Permission::Ask);
    assert_eq!(perm("run_command"), Permission::Deny);
    assert!(status.internet);

    // Execution tools are hidden from the model.
    let tools = mgr.available_tools().await;
    assert!(tools.iter().all(|t| t.tool_name != "run_command"));
    let search = tools.iter().find(|t| t.tool_name == "web_search").unwrap().clone();
    assert_eq!(search.category, ToolCategory::Search);

    let out = mgr.call_tool(&search, serde_json::json!({"query": "latest php"})).await.unwrap();
    assert!(out.text.contains("Result for latest php"));
    assert_eq!(out.sources[0].url, "https://www.php.net/releases/");
    assert!(out.sources.iter().any(|s| s.url == "https://www.php.net/ChangeLog-8.php"));

    let fail = tools.iter().find(|t| t.tool_name == "fail").unwrap();
    assert!(mgr.call_tool(fail, serde_json::json!({})).await.unwrap().is_error);

    // Users cannot grant "allow" to sensitive tools.
    assert_eq!(mgr.set_permission(&cfg.id, "write_file", Permission::Allow).await.unwrap(), Permission::Ask);
    assert_eq!(mgr.set_permission(&cfg.id, "run_command", Permission::Allow).await.unwrap(), Permission::Ask);
    mgr.set_permission(&cfg.id, "web_search", Permission::Deny).await.unwrap();
    assert!(mgr.available_tools().await.iter().all(|t| t.tool_name != "web_search"));
    assert!(mgr.call_tool(&search, serde_json::json!({"query": "x"})).await.is_err());

    mgr.set_enabled(&cfg.id, false).await.unwrap();
    assert!(mgr.available_tools().await.is_empty());
}

#[tokio::test]
async fn missing_command_reports_error() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let mut c = fixture_config(true);
    c.command = Some("definitely-not-a-real-mcp-binary-xyz".into());
    let cfg = db.save_mcp_server(&c).unwrap();
    let mgr = McpManager::new(db, Arc::new(|| {}));
    assert!(mgr.connect(&cfg.id).await.is_err());
    let st = mgr.statuses().await.unwrap().remove(0);
    assert!(st.state == "error" || st.state == "offline", "{}", st.state);
    assert!(mgr.available_tools().await.is_empty());
}

#[tokio::test]
async fn unreachable_http_server_is_not_connected() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    let mut c = fixture_config(true);
    c.transport = "http".into();
    c.command = None;
    c.url = Some(format!("http://127.0.0.1:{port}/mcp"));
    let cfg = db.save_mcp_server(&c).unwrap();
    let mgr = McpManager::new(db, Arc::new(|| {}));
    assert!(mgr.connect(&cfg.id).await.is_err());
    assert_ne!(mgr.statuses().await.unwrap()[0].state, "connected");
}
