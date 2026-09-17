//! Tool classification and permission policy.
//!
//! Policy: only read-only categories (search, fetch, read) may run without
//! confirmation. Write/unknown tools always require confirmation. Command
//! execution tools are denied by default and can at most be set to "ask".

use crate::services::chat::tools::{Permission, ToolCategory};

fn has_word(hay: &str, words: &[&str]) -> bool {
    let tokens: Vec<&str> = hay.split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty()).collect();
    words.iter().any(|w| tokens.iter().any(|t| t == w || (w.len() > 4 && t.starts_with(w))))
}

pub fn classify(name: &str, description: &str, read_only_hint: Option<bool>, destructive_hint: Option<bool>) -> ToolCategory {
    // camelCase -> snake for tokenizing
    let mut n = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            n.push('_');
        }
        n.push(c.to_ascii_lowercase());
    }
    let d = description.to_lowercase();

    const EXEC: &[&str] = &["exec", "execute", "run", "shell", "command", "cmd", "terminal", "powershell", "bash", "script", "eval", "spawn", "kill", "process", "sudo", "install"];
    const WRITE: &[&str] = &["write", "create", "delete", "remove", "update", "edit", "move", "rename", "upload", "send", "post", "push", "commit", "set", "modify", "insert", "drop", "patch", "append", "save", "merge", "close", "archive", "publish", "mkdir", "rm", "put"];
    const SEARCH: &[&str] = &["search", "query", "lookup", "google", "bing", "duckduckgo", "searxng", "brave", "tavily", "websearch"];
    const FETCH: &[&str] = &["fetch", "scrape", "crawl", "browse", "navigate", "url", "webpage", "page", "extract", "download"];
    const READ: &[&str] = &["read", "get", "list", "find", "view", "show", "describe", "stat", "info", "status", "open", "load", "tree", "cat"];

    if has_word(&n, EXEC) {
        return ToolCategory::Execute;
    }
    if destructive_hint == Some(true) || has_word(&n, WRITE) {
        return ToolCategory::Write;
    }
    if has_word(&n, SEARCH) {
        return ToolCategory::Search;
    }
    if has_word(&n, FETCH) {
        return ToolCategory::Fetch;
    }
    if has_word(&n, &["web", "internet", "news"]) {
        return ToolCategory::Search;
    }
    if read_only_hint == Some(true) || has_word(&n, READ) {
        return ToolCategory::Read;
    }
    // Fall back to the description for generic names like "brave".
    if d.contains("search the web") || d.contains("web search") || d.contains("search engine") {
        return ToolCategory::Search;
    }
    if d.contains("execute") || d.contains("shell command") || d.contains("run a command") {
        return ToolCategory::Execute;
    }
    ToolCategory::Other
}

pub fn default_permission(category: ToolCategory) -> Permission {
    match category {
        ToolCategory::Search | ToolCategory::Fetch | ToolCategory::Read => Permission::Allow,
        ToolCategory::Write | ToolCategory::Other => Permission::Ask,
        ToolCategory::Execute => Permission::Deny,
    }
}

/// Applies the policy ceiling to a user-requested permission.
pub fn clamp_permission(category: ToolCategory, requested: Permission) -> Permission {
    if requested == Permission::Allow && category.is_sensitive() {
        Permission::Ask
    } else {
        requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        assert_eq!(classify("searxng_web_search", "", None, None), ToolCategory::Search);
        assert_eq!(classify("web_url_read", "", None, None), ToolCategory::Fetch);
        assert_eq!(classify("brave_web_search", "", None, None), ToolCategory::Search);
        assert_eq!(classify("read_file", "", None, None), ToolCategory::Read);
        assert_eq!(classify("list_directory", "", Some(true), None), ToolCategory::Read);
        assert_eq!(classify("write_file", "", None, None), ToolCategory::Write);
        assert_eq!(classify("createIssue", "", None, None), ToolCategory::Write);
        assert_eq!(classify("move_file", "", None, None), ToolCategory::Write);
        assert_eq!(classify("run_command", "", None, None), ToolCategory::Execute);
        assert_eq!(classify("execute_powershell", "", None, None), ToolCategory::Execute);
        assert_eq!(classify("docker", "Execute a docker command", None, None), ToolCategory::Execute);
        assert_eq!(classify("frobnicate", "does things", None, None), ToolCategory::Other);
        assert_eq!(classify("frobnicate", "", Some(true), Some(true)), ToolCategory::Write);
        assert_eq!(classify("brave", "Search the web with Brave", None, None), ToolCategory::Search);
    }

    #[test]
    fn policy() {
        assert_eq!(default_permission(ToolCategory::Search), Permission::Allow);
        assert_eq!(default_permission(ToolCategory::Write), Permission::Ask);
        assert_eq!(default_permission(ToolCategory::Execute), Permission::Deny);
        assert_eq!(clamp_permission(ToolCategory::Write, Permission::Allow), Permission::Ask);
        assert_eq!(clamp_permission(ToolCategory::Execute, Permission::Allow), Permission::Ask);
        assert_eq!(clamp_permission(ToolCategory::Read, Permission::Allow), Permission::Allow);
        assert_eq!(clamp_permission(ToolCategory::Read, Permission::Deny), Permission::Deny);
    }
}
