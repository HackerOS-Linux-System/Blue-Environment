use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn notif_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/Blue-Environment/blue-notifications")
}
fn rules_path() -> PathBuf { notif_dir().join("rules.json") }

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRule {
    pub id: String,
    pub name: String,
    /// Only `"rss"` has a real checker right now — see module doc.
    pub kind: String,
    pub url: String,
    pub interval_minutes: u32,
    pub enabled: bool,
    /// GUIDs (or, absent a `<guid>`, the item link) already seen — used
    /// to compute the "new since last check" diff. Bounded to the most
    /// recent 200 to keep the file from growing forever on a
    /// high-volume feed; that's more than enough to never re-notify
    /// about something already seen under any realistic polling
    /// interval.
    pub last_seen_guids: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct FeedItem {
    pub guid: String,
    pub title: String,
    pub link: String,
}

impl From<crate::feed_parser::FeedItem> for FeedItem {
    fn from(f: crate::feed_parser::FeedItem) -> Self {
        // This app's own `FeedItem` predates the shared `feed_parser`
        // module and only ever needed guid/title/link (the "did
        // something new get published" check doesn't care about a
        // description or date) — kept as its own smaller type rather
        // than switching every call site to the richer shared one, so
        // existing frontend code (`notif_check_feed`'s response shape)
        // doesn't change.
        Self { guid: f.guid, title: f.title, link: f.link }
    }
}

fn read_rules() -> Vec<NotificationRule> {
    fs::read_to_string(rules_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn write_rules(rules: &[NotificationRule]) -> Result<(), String> {
    fs::create_dir_all(notif_dir()).map_err(|e| e.to_string())?;
    fs::write(rules_path(), serde_json::to_string_pretty(rules).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn notif_rules_load() -> Vec<NotificationRule> {
    read_rules()
}

#[tauri::command]
pub fn notif_rules_save(rule: NotificationRule) -> Result<(), String> {
    let mut rules = read_rules();
    if let Some(existing) = rules.iter_mut().find(|r| r.id == rule.id) {
        *existing = rule;
    } else {
        rules.push(rule);
    }
    write_rules(&rules)
}

#[tauri::command]
pub fn notif_rules_delete(id: String) -> Result<(), String> {
    let mut rules = read_rules();
    rules.retain(|r| r.id != id);
    write_rules(&rules)
}

/// Splits `xml` into item/entry blocks and pulls guid/title/link out of
/// each — delegates entirely to the shared `feed_parser` module (see
/// that module's doc comment for why it was extracted from here) and
/// just narrows its richer `FeedItem` down to the guid/title/link shape
/// this app has always used.
fn parse_feed_items(xml: &str) -> Vec<FeedItem> {
    crate::feed_parser::parse_feed_items(xml).into_iter().map(FeedItem::from).collect()
}

#[derive(Serialize)]
pub struct FeedCheckResult {
    pub new_items: Vec<FeedItem>,
}

/// Fetches `rule.url`, diffs against `rule.last_seen_guids`, persists
/// the updated seen-set (capped at the most recent 200 — see
/// `NotificationRule::last_seen_guids` doc), and returns whatever's new.
/// The frontend is responsible for turning `new_items` into actual
/// desktop/Notification-Center alerts — see this module's doc comment
/// on why that split exists (real shell integration via the existing
/// `notificationManager`, not a backend-owned notification path).
#[tauri::command]
pub async fn notif_check_feed(rule: NotificationRule) -> Result<FeedCheckResult, String> {
    if rule.kind != "rss" {
        return Err(format!("rule kind \"{}\" has no checker implemented (only \"rss\" does)", rule.kind));
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("BlueNotifications/1.0")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&rule.url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("feed returned HTTP {}", resp.status()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let items = parse_feed_items(&body);

    let seen: std::collections::HashSet<&str> = rule.last_seen_guids.iter().map(|s| s.as_str()).collect();
    let new_items: Vec<FeedItem> = items.iter().filter(|i| !seen.contains(i.guid.as_str())).cloned().collect();

    // Persist the updated seen-set regardless of whether this was the
    // very first check (an empty `last_seen_guids` — that first check
    // deliberately does NOT surface every existing item in the feed as
    // "new"; it just seeds the seen-set silently, matching what anyone
    // subscribing to a feed for the first time actually wants — nobody
    // wants 200 backlog notifications the moment they add a feed).
    let first_check = rule.last_seen_guids.is_empty();
    let mut updated = rule.clone();
    let mut all_guids: Vec<String> = items.iter().map(|i| i.guid.clone()).collect();
    all_guids.extend(rule.last_seen_guids.iter().cloned());
    all_guids.dedup();
    all_guids.truncate(200);
    updated.last_seen_guids = all_guids;
    let mut rules = read_rules();
    if let Some(existing) = rules.iter_mut().find(|r| r.id == rule.id) {
        *existing = updated;
    } else {
        rules.push(updated);
    }
    write_rules(&rules)?;

    Ok(FeedCheckResult { new_items: if first_check { Vec::new() } else { new_items } })
}
