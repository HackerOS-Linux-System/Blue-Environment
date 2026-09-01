use super::{Message, MessageDirection};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

fn db_path() -> PathBuf {
    super::messages_dir().join("messages.db")
}
fn legacy_json_path() -> PathBuf {
    super::messages_dir().join("messages.json")
}

// One shared, mutex-guarded connection per process rather than opening
// a fresh connection per call — SQLite handles concurrent connections
// fine, but a single connection avoids any question of file-locking
// contention between rapid-fire calls (e.g. `matrix_refresh_thread`
// inserting a batch right after `messages_send` inserted one), and this
// app's own message volume never justifies a connection pool.
static CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);

fn with_connection<T>(f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, String> {
    let mut guard = CONNECTION.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(open_and_migrate().map_err(|e| e.to_string())?);
    }
    let conn = guard.as_ref().unwrap();
    f(conn).map_err(|e| e.to_string())
}

fn open_and_migrate() -> rusqlite::Result<Connection> {
    std::fs::create_dir_all(super::messages_dir()).ok();
    let conn = Connection::open(db_path())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id              TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            body            TEXT NOT NULL,
            direction       TEXT NOT NULL,
            sent_at         TEXT NOT NULL,
            read            INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id, sent_at)",
        [],
    )?;

    migrate_legacy_json(&conn)?;
    // Age-based retention runs once per process start (not per-insert,
    // unlike the per-conversation cap — see `enforce_retention_for`'s
    // own doc for why that one runs on every send instead) — cheap
    // enough that "once at startup" is plenty responsive for a setting
    // a person would change rarely, and avoids a per-message `julianday`
    // computation on every single insert.
    enforce_age_retention(&conn, retention_max_age_days())?;
    Ok(conn)
}

/// One-time import of the pre-SQLite `messages.json` format, if
/// present. Safe to call every startup — it only does anything the
/// first time (`legacy_json_path()` is renamed away once migrated, so
/// every subsequent call finds nothing to do).
fn migrate_legacy_json(conn: &Connection) -> rusqlite::Result<()> {
    let legacy_path = legacy_json_path();
    let Ok(raw) = std::fs::read_to_string(&legacy_path) else { return Ok(()) };
    let Ok(items) = serde_json::from_str::<Vec<Message>>(&raw) else { return Ok(()) };

    for m in &items {
        insert_message(conn, m)?;
    }
    tracing::info!("Blue Messages: migrated {} messages from messages.json to SQLite", items.len());

    // Keep the old file around under a new name rather than deleting it
    // — see module doc's "Migration" section for why.
    let _ = std::fs::rename(&legacy_path, legacy_path.with_extension("json.migrated"));
    Ok(())
}

fn insert_message(conn: &Connection, m: &Message) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO messages (id, conversation_id, body, direction, sent_at, read) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![m.id, m.conversation_id, m.body, direction_to_str(&m.direction), m.sent_at, m.read as i32],
    )?;
    Ok(())
}

fn direction_to_str(d: &MessageDirection) -> &'static str {
    match d {
        MessageDirection::Outgoing => "outgoing",
        MessageDirection::Incoming => "incoming",
    }
}
fn direction_from_str(s: &str) -> MessageDirection {
    if s == "outgoing" { MessageDirection::Outgoing } else { MessageDirection::Incoming }
}

pub fn add(message: &Message) -> Result<(), String> {
    with_connection(|conn| insert_message(conn, message))?;
    // Enforce retention after every insert rather than on a timer —
    // this app has no background scheduler today, and a person sending
    // messages is exactly when history is growing, so checking here
    // costs nothing extra beyond what already needed a DB round-trip.
    enforce_retention_for(message.conversation_id.clone());
    Ok(())
}

pub fn add_many(messages: &[Message]) -> Result<(), String> {
    with_connection(|conn| {
        for m in messages {
            insert_message(conn, m)?;
        }
        Ok(())
    })?;
    // One retention pass per distinct conversation touched, not per
    // message — `matrix_refresh_thread` can insert dozens of messages
    // in one call, and there's no value in re-checking the same
    // conversation's row count that many times in a row.
    let mut seen = std::collections::HashSet::new();
    for m in messages {
        if seen.insert(m.conversation_id.clone()) {
            enforce_retention_for(m.conversation_id.clone());
        }
    }
    Ok(())
}

// ── Retention ─────────────────────────────────────────────────────────
// Two independent policies, both real defaults rather than "unlimited
// unless someone configures otherwise":
//   1. Per-conversation cap (`retention_max_messages_per_conversation`) —
//      keeps at most N messages *per conversation*, oldest deleted
//      first. Enforced after every insert (see `add`/`add_many` above).
//   2. Global age cap (`retention_max_age_days`) — deletes any message
//      older than N days, regardless of conversation. Enforced once at
//      startup (see `open_and_migrate`).
// Both are overridable via env vars (`BLUE_MESSAGES_MAX_PER_CONVERSATION`,
// `BLUE_MESSAGES_MAX_AGE_DAYS`) — the same override mechanism
// `BLUE_MESSAGES_DIR` already uses for tests, and doubles as a real,
// if unpolished, way for a person to change the policy without a
// dedicated settings UI existing yet (see `messages_get_retention_settings`/
// `messages_set_retention_settings` below for the real UI-facing path).

const DEFAULT_MAX_MESSAGES_PER_CONVERSATION: i64 = 10_000;
const DEFAULT_MAX_AGE_DAYS: i64 = 365;

fn retention_max_messages_per_conversation() -> i64 {
    std::env::var("BLUE_MESSAGES_MAX_PER_CONVERSATION")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_MESSAGES_PER_CONVERSATION)
}

fn retention_max_age_days() -> i64 {
    std::env::var("BLUE_MESSAGES_MAX_AGE_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_AGE_DAYS)
}

/// Deletes the oldest messages in `conversation_id` beyond
/// [`retention_max_messages_per_conversation`], keeping the most
/// recent N. A cap of `<= 0` disables this policy entirely (treated as
/// "unlimited" — a person who explicitly sets this to `0` clearly
/// doesn't want messages silently deleted, not "keep zero messages").
fn enforce_retention_for(conversation_id: String) {
    let max = retention_max_messages_per_conversation();
    if max <= 0 {
        return;
    }
    let _ = with_connection(|conn| {
        conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1 AND id NOT IN (
                SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY sent_at DESC LIMIT ?2
            )",
            params![conversation_id, max],
        )?;
        Ok(())
    });
}

/// Deletes every message older than `max_age_days`, across every
/// conversation. `sent_at` is stored as an RFC 3339 string (see
/// `Message.sent_at`'s own doc in `mod.rs`) — SQLite's `julianday()`
/// parses ISO 8601 timestamps directly, so this comparison doesn't need
/// a separate stored "age in days" column kept in sync on every read.
/// A cap of `<= 0` disables this policy, same convention as
/// [`enforce_retention_for`].
fn enforce_age_retention(conn: &Connection, max_age_days: i64) -> rusqlite::Result<usize> {
    if max_age_days <= 0 {
        return Ok(0);
    }
    conn.execute(
        "DELETE FROM messages WHERE julianday('now') - julianday(sent_at) > ?1",
        params![max_age_days],
    )
}

/// The two retention numbers currently in effect, for a future
/// Settings UI to display/edit (see `messages_set_retention_settings`
/// for the write side) — reading from the same env-var overrides
/// `enforce_retention_for`/`enforce_age_retention` themselves consult,
/// so this always reflects what's actually being enforced, not a
/// separately-tracked "intended" value that could drift from reality.
#[tauri::command]
pub fn messages_get_retention_settings() -> (i64, i64) {
    (retention_max_messages_per_conversation(), retention_max_age_days())
}

/// Sets both retention numbers for the remainder of this process's
/// lifetime (via the same env vars the getters read — see this
/// module's own "Retention" doc) and immediately runs an age-based
/// sweep with the new value, so tightening the policy has a visible
/// effect right away rather than only applying to messages received
/// after the change. Does **not** persist across restarts on its own —
/// wiring this into `Config`/`config.hk` so it survives a restart is
/// real, separate follow-up work (this project's small-`.hk` tier is
/// exactly where a "keep messages for N days" setting belongs).
#[tauri::command]
pub fn messages_set_retention_settings(max_per_conversation: i64, max_age_days: i64) -> Result<(), String> {
    std::env::set_var("BLUE_MESSAGES_MAX_PER_CONVERSATION", max_per_conversation.to_string());
    std::env::set_var("BLUE_MESSAGES_MAX_AGE_DAYS", max_age_days.to_string());
    with_connection(|conn| enforce_age_retention(conn, max_age_days).map(|_| ()))
}

pub fn thread(conversation_id: &str) -> Vec<Message> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, body, direction, sent_at, read FROM messages WHERE conversation_id = ?1 ORDER BY sent_at ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                body: row.get(2)?,
                direction: direction_from_str(&row.get::<_, String>(3)?),
                sent_at: row.get(4)?,
                read: row.get::<_, i32>(5)? != 0,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    })
    .unwrap_or_default()
}

pub fn message_ids_for(conversation_id: &str) -> std::collections::HashSet<String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare("SELECT id FROM messages WHERE conversation_id = ?1")?;
        let rows = stmt.query_map(params![conversation_id], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<std::collections::HashSet<_>>>()
    })
    .unwrap_or_default()
}

pub fn mark_read(conversation_id: &str) -> Result<(), String> {
    with_connection(|conn| {
        conn.execute("UPDATE messages SET read = 1 WHERE conversation_id = ?1", params![conversation_id])?;
        Ok(())
    })
}

pub fn delete_for_conversation(conversation_id: &str) -> Result<(), String> {
    with_connection(|conn| {
        conn.execute("DELETE FROM messages WHERE conversation_id = ?1", params![conversation_id])?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Points `super::messages_dir()`'s effective root at a fresh temp
    /// dir per test (via the same `BLUE_MESSAGES_DIR`-style override
    /// pattern `themes.rs`'s `BLUE_THEMES_DIR` already establishes for
    /// exactly this reason — tests can't share `~/.config`, and can't
    /// share a single global `CONNECTION` cache with each other
    /// either), returning the path for cleanup.
    fn isolated_test_env() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("blue-messages-storage-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("BLUE_MESSAGES_DIR", &dir);
        *CONNECTION.lock().unwrap() = None; // force a fresh connection against the new dir
        dir
    }

    fn sample_message(id: &str, conversation_id: &str) -> Message {
        Message {
            id: id.to_string(),
            conversation_id: conversation_id.to_string(),
            body: "hello".to_string(),
            direction: MessageDirection::Outgoing,
            sent_at: "2026-01-01T00:00:00Z".to_string(),
            read: false,
        }
    }

    #[test]
    fn inserts_and_reads_back_a_thread() {
        let dir = isolated_test_env();
        add(&sample_message("m1", "c1")).unwrap();
        add(&sample_message("m2", "c1")).unwrap();
        add(&sample_message("m3", "c2")).unwrap();

        let t = thread("c1");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].id, "m1");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mark_read_only_touches_its_own_conversation() {
        let dir = isolated_test_env();
        add(&sample_message("m1", "c1")).unwrap();
        add(&sample_message("m2", "c2")).unwrap();
        mark_read("c1").unwrap();

        assert!(thread("c1")[0].read);
        assert!(!thread("c2")[0].read);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migrates_legacy_json_on_first_open() {
        let dir = isolated_test_env();
        let legacy = vec![sample_message("legacy-1", "c1")];
        std::fs::write(dir.join("messages.json"), serde_json::to_string(&legacy).unwrap()).unwrap();
        *CONNECTION.lock().unwrap() = None; // force open_and_migrate to run again against the file we just wrote

        let t = thread("c1");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].id, "legacy-1");
        assert!(dir.join("messages.json.migrated").exists());
        assert!(!dir.join("messages.json").exists());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn per_conversation_cap_keeps_only_the_newest_n() {
        let dir = isolated_test_env();
        std::env::set_var("BLUE_MESSAGES_MAX_PER_CONVERSATION", "3");

        for i in 0..10 {
            let mut m = sample_message(&format!("m{i}"), "c1");
            m.sent_at = format!("2026-01-01T00:00:{:02}Z", i);
            add(&m).unwrap();
        }

        let t = thread("c1");
        assert_eq!(t.len(), 3, "should keep exactly the cap, not all 10 inserted");
        let ids: Vec<&str> = t.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["m7", "m8", "m9"]);

        std::env::remove_var("BLUE_MESSAGES_MAX_PER_CONVERSATION");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn cap_of_zero_disables_the_policy() {
        let dir = isolated_test_env();
        std::env::set_var("BLUE_MESSAGES_MAX_PER_CONVERSATION", "0");

        for i in 0..20 {
            add(&sample_message(&format!("m{i}"), "c1")).unwrap();
        }
        assert_eq!(thread("c1").len(), 20, "a cap of 0 must mean unlimited, not 'keep zero'");

        std::env::remove_var("BLUE_MESSAGES_MAX_PER_CONVERSATION");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn age_retention_deletes_old_messages_but_keeps_recent_ones() {
        let dir = isolated_test_env();
        std::env::set_var("BLUE_MESSAGES_MAX_AGE_DAYS", "30");

        let mut old = sample_message("old", "c1");
        old.sent_at = "2020-01-01T00:00:00Z".to_string();
        add(&old).unwrap();
        let mut recent = sample_message("recent", "c1");
        recent.sent_at = chrono::Utc::now().to_rfc3339();
        add(&recent).unwrap();

        *CONNECTION.lock().unwrap() = None; // simulate a fresh process start, re-running the startup age sweep

        let t = thread("c1");
        let ids: Vec<&str> = t.iter().map(|m| m.id.as_str()).collect();
        assert!(!ids.contains(&"old"), "message from 2020 should be pruned under a 30-day policy");
        assert!(ids.contains(&"recent"), "a message from just now must survive the same sweep");

        std::env::remove_var("BLUE_MESSAGES_MAX_AGE_DAYS");
        let _ = std::fs::remove_dir_all(dir);
    }
}
