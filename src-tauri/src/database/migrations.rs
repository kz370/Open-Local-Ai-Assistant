use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    // 1: initial schema
    r#"
    CREATE TABLE conversations (
        id          TEXT PRIMARY KEY,
        title       TEXT NOT NULL,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL,
        model       TEXT,
        language    TEXT
    );
    CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC);

    CREATE TABLE messages (
        id                 TEXT PRIMARY KEY,
        conversation_id    TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
        role               TEXT NOT NULL,
        content            TEXT NOT NULL,
        language           TEXT,
        reasoning          TEXT,
        sources_json       TEXT,
        tool_activity_json TEXT,
        tool_calls_json    TEXT,
        tool_call_id       TEXT,
        created_at         TEXT NOT NULL
    );
    CREATE INDEX idx_messages_conversation ON messages(conversation_id, created_at);

    CREATE VIRTUAL TABLE messages_fts USING fts5(
        content, conversation_id UNINDEXED, message_id UNINDEXED,
        tokenize = 'unicode61 remove_diacritics 2'
    );
    CREATE TRIGGER messages_ai AFTER INSERT ON messages
      WHEN new.role IN ('user', 'assistant') BEGIN
        INSERT INTO messages_fts(content, conversation_id, message_id)
        VALUES (new.content, new.conversation_id, new.id);
    END;
    CREATE TRIGGER messages_au AFTER UPDATE OF content ON messages BEGIN
        DELETE FROM messages_fts WHERE message_id = old.id;
        INSERT INTO messages_fts(content, conversation_id, message_id)
        SELECT new.content, new.conversation_id, new.id WHERE new.role IN ('user', 'assistant');
    END;
    CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
        DELETE FROM messages_fts WHERE message_id = old.id;
    END;

    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE mcp_servers (
        id           TEXT PRIMARY KEY,
        name         TEXT NOT NULL,
        description  TEXT,
        transport    TEXT NOT NULL,
        command      TEXT,
        args_json    TEXT,
        env_json     TEXT,
        url          TEXT,
        headers_json TEXT,
        enabled      INTEGER NOT NULL DEFAULT 0,
        source       TEXT NOT NULL DEFAULT 'user',
        created_at   TEXT NOT NULL
    );

    CREATE TABLE mcp_tool_permissions (
        server_id  TEXT NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
        tool_name  TEXT NOT NULL,
        permission TEXT NOT NULL,
        PRIMARY KEY (server_id, tool_name)
    );
    "#,
    // 2: files and images attached to a message
    r#"
    ALTER TABLE messages ADD COLUMN attachments_json TEXT;
    "#,
    // 3: generation stats of assistant replies
    r#"
    ALTER TABLE messages ADD COLUMN stats_json TEXT;
    "#,
    // 4: past dictations
    r#"
    CREATE TABLE dictation_history (
        id         TEXT PRIMARY KEY,
        created_at TEXT NOT NULL,
        raw        TEXT NOT NULL,
        text       TEXT NOT NULL,
        corrected  INTEGER NOT NULL DEFAULT 0,
        inserted   INTEGER NOT NULL DEFAULT 1
    );
    CREATE INDEX idx_dictation_history_created ON dictation_history(created_at DESC);
    "#,
];

pub fn run(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let target = (i + 1) as i64;
        if version < target {
            conn.execute_batch(&format!(
                "BEGIN; {sql}; PRAGMA user_version = {target}; COMMIT;"
            ))?;
        }
    }
    Ok(())
}
