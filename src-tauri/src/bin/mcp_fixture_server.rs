//! Minimal stdio MCP server used only by integration tests.
//! Tools: web_search (fake results with URLs), write_file (write category),
//! run_command (execution category), fail (returns an MCP tool error).

use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn reply(out: &mut impl Write, v: Value) {
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

fn main() {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(id) = msg.get("id").cloned() else { continue }; // notification
        let method = msg["method"].as_str().unwrap_or("");
        let result = match method {
            "initialize" => json!({
                "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "fixture", "version": "1.0.0"}
            }),
            "tools/list" => json!({"tools": [
                {"name": "web_search", "description": "Search the web", "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}, "annotations": {"readOnlyHint": true}},
                {"name": "write_file", "description": "Write a file", "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}}},
                {"name": "run_command", "description": "Run a shell command", "inputSchema": {"type": "object"}},
                {"name": "fail", "description": "Always fails", "inputSchema": {"type": "object"}}
            ]}),
            "tools/call" => {
                let name = msg["params"]["name"].as_str().unwrap_or("");
                let args = &msg["params"]["arguments"];
                match name {
                    "web_search" => json!({"content": [{"type": "text", "text": format!(
                        "Title: Result for {}\nURL: https://www.php.net/releases/\n\n[Changelog](https://www.php.net/ChangeLog-8.php)",
                        args["query"].as_str().unwrap_or(""))}]}),
                    "write_file" => json!({"content": [{"type": "text", "text": "written"}]}),
                    "fail" => json!({"content": [{"type": "text", "text": "boom"}], "isError": true}),
                    _ => {
                        reply(&mut out, json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32602, "message": "unknown tool"}}));
                        continue;
                    }
                }
            }
            "ping" => json!({}),
            _ => {
                reply(&mut out, json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "method not found"}}));
                continue;
            }
        };
        reply(&mut out, json!({"jsonrpc": "2.0", "id": id, "result": result}));
    }
}
