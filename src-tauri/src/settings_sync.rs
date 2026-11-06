#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use crate::lock_ext::LockExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_ENTRIES: usize = 200;
pub const MAX_VALUE_BYTES: usize = 256 * 1024;
pub const MAX_TOTAL_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_FUTURE_SKEW_MS: i64 = 24 * 60 * 60 * 1000;

/// `UserConfig` fields that travel, plus `store:*` pseudo-fields for settings kept outside it.
pub const SYNC_FIELDS: &[&str] = &[
    "theme", "themeName", "accentColor", "shellThemeId", "customThemes", "windowControlsOrderByTheme",
    "iconTheme", "iconSize", "cursorTheme",
    "panelEnabled", "panelPosition", "panelSize", "panelOpacity", "topBar", "startButtonLabelMode", "startButtonIcon",
    "language", "nightLightEnabled", "nightLightTemperature", "nightLightSchedule", "nightLightStartHour", "nightLightEndHour",
    "hotCorners", "superDoubleTapMs", "osdEnabled", "onscreenKeyboardEnabled", "showTransferProgress",
    "clipboardHoverPreviewEnabled", "networkHoverInfoEnabled", "weatherEnabled", "weatherCity", "weatherUnit",
    "pinnedApps", "defaultTextEditor", "customBookmarks",
    "store:a11y", "store:focus",
];

/// Substrings that must never appear in a synchronised key, whatever the allow-list says (belt and braces).
const NEVER: &[&str] = &["password", "token", "secret", "apikey", "api_key", "credential", "vault", "parental", "pin_", "cert", "pairing", "accounts", "aiconfig"];

pub fn is_syncable(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SYNC_FIELDS.contains(&key) && !NEVER.iter().any(|n| lower.contains(n))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub value: Value,
    /// Epoch milliseconds of the edit.
    pub updated_at: i64,
    /// Device that made the edit.
    pub origin: String,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema: u32,
    pub device_id: String,
    pub generated_at: i64,
    pub entries: BTreeMap<String, Entry>,
}

/// `a` beats `b`: later edit wins, the larger device id breaks exact ties.
pub fn newer(a: &Entry, b: &Entry) -> bool {
    (a.updated_at, &a.origin) > (b.updated_at, &b.origin)
}

/// Validates a snapshot from a peer: right schema, allow-listed keys only,
/// bounded size, timestamps clamped. Never panics on hostile input.
pub fn sanitize(mut s: Snapshot, now: i64) -> Result<Snapshot, String> {
    if s.schema != SCHEMA_VERSION {
        return Err(format!("unsupported sync schema {} (this version speaks {SCHEMA_VERSION})", s.schema));
    }
    if s.entries.len() > MAX_ENTRIES * 2 {
        return Err("too many entries".into());
    }
    let mut total = 0usize;
    s.entries.retain(|k, e| {
        if !is_syncable(k) {
            return false;
        }
        let size = serde_json::to_vec(&e.value).map(|v| v.len()).unwrap_or(usize::MAX);
        if size > MAX_VALUE_BYTES {
            return false;
        }
        total = total.saturating_add(size);
        e.updated_at = e.updated_at.min(now + MAX_FUTURE_SKEW_MS).max(0);
        true
    });
    if total > MAX_TOTAL_BYTES {
        return Err("snapshot too large".into());
    }
    Ok(s)
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub entries: BTreeMap<String, Entry>,
}

/// Changes a merge decided to apply locally.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct Applied(pub BTreeMap<String, Entry>);

impl Store {
    /// Records a local edit. Returns true when it was a real change.
    pub fn put_local(&mut self, key: &str, value: Option<Value>, now: i64, device_id: &str) -> bool {
        if !is_syncable(key) {
            return false;
        }
        let (value, deleted) = match value {
            Some(v) => (v, false),
            None => (Value::Null, true),
        };
        if let Some(cur) = self.entries.get(key) {
            if cur.deleted == deleted && cur.value == value {
                return false; // already what the store says: no new timestamp, no echo
            }
        } else if deleted {
            return false;
        }
        // strictly newer than whatever is there, even if the clock stepped back
        let ts = self.entries.get(key).map(|c| c.updated_at.max(now - 1) + 1).unwrap_or(now).max(now);
        self.entries.insert(key.to_string(), Entry { value, updated_at: ts, origin: device_id.to_string(), deleted });
        true
    }

    pub fn snapshot(&self, device_id: &str, now: i64) -> Snapshot {
        Snapshot { schema: SCHEMA_VERSION, device_id: device_id.to_string(), generated_at: now, entries: self.entries.clone() }
    }

    /// Merges a (sanitised) remote snapshot. Returns what changed here.
    pub fn merge(&mut self, remote: &Snapshot) -> Applied {
        let mut applied = Applied::default();
        for (k, theirs) in &remote.entries {
            let take = match self.entries.get(k) {
                None => true,
                Some(ours) => newer(theirs, ours),
            };
            if take {
                let same_value = self.entries.get(k).map(|o| o.value == theirs.value && o.deleted == theirs.deleted).unwrap_or(false);
                self.entries.insert(k.clone(), theirs.clone());
                if !same_value {
                    applied.0.insert(k.clone(), theirs.clone());
                }
            }
        }
        applied
    }
}

// ── persistent state ───────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerState {
    pub enabled: bool,
    #[serde(default)]
    pub last_sync: Option<i64>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub store: Store,
    pub peers: BTreeMap<String, PeerState>,
}

/// Remote changes waiting for the shell to apply them to its live config.
static PENDING: Mutex<BTreeMap<String, Entry>> = Mutex::new(BTreeMap::new());

fn dir() -> PathBuf {
    if let Ok(d) = std::env::var("BLUE_SYNC_DIR") {
        return PathBuf::from(d);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Blue-Environment")
}

fn state_path() -> PathBuf {
    dir().join("sync-state.json")
}

pub fn load() -> State {
    std::fs::read_to_string(state_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save(s: &State) -> Result<(), String> {
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    let tmp = state_path().with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, state_path()).map_err(|e| e.to_string())
}

static STATE_LOCK: Mutex<()> = Mutex::new(());

/// Runs `f` on the persisted state under a lock and saves it afterwards.
pub fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> Result<T, String> {
    let _g = STATE_LOCK.lock_safe();
    let mut s = load();
    let out = f(&mut s);
    save(&s)?;
    Ok(out)
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn is_enabled(device_id: &str) -> bool {
    load().peers.get(device_id).map(|p| p.enabled).unwrap_or(false)
}

pub fn any_enabled() -> bool {
    load().peers.values().any(|p| p.enabled)
}

pub fn report_local(local_device: &str, changes: BTreeMap<String, Option<Value>>) -> Result<usize, String> {
    let now = now_ms();
    with_state(|s| changes.into_iter().filter(|(k, v)| s.store.put_local(k, v.clone(), now, local_device)).count())
}

/// A peer pushed its snapshot (server side) or answered ours (client side):
/// merge, remember what the shell has to apply, return what changed.
pub fn apply_remote(peer: &str, remote: Snapshot, local_device: &str) -> Result<Applied, String> {
    let now = now_ms();
    let remote = sanitize(remote, now)?;
    let _ = local_device;
    let applied = with_state(|s| {
        let a = s.store.merge(&remote);
        let p = s.peers.entry(peer.to_string()).or_default();
        p.last_sync = Some(now);
        p.last_error = None;
        a
    })?;
    PENDING.lock_safe().extend(applied.0.clone());
    Ok(applied)
}

/// The shell drains this and applies the values to its live configuration.
pub fn take_pending() -> BTreeMap<String, Entry> {
    std::mem::take(&mut *PENDING.lock_safe())
}

pub fn note_error(peer: &str, err: &str) {
    let _ = with_state(|s| s.peers.entry(peer.to_string()).or_default().last_error = Some(err.to_string()));
}

pub fn set_enabled(peer: &str, enabled: bool) -> Result<(), String> {
    with_state(|s| s.peers.entry(peer.to_string()).or_default().enabled = enabled)
}

/// What a server answers to an exchange: merge theirs, reply with the merged whole.
pub fn handle_exchange(peer: &str, theirs: Snapshot, local_device: &str) -> Result<Snapshot, String> {
    if !is_enabled(peer) {
        return Err("sync is not enabled for this device".into());
    }
    apply_remote(peer, theirs, local_device)?;
    Ok(load().store.snapshot(local_device, now_ms()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn e(v: Value, at: i64, origin: &str) -> Entry {
        Entry { value: v, updated_at: at, origin: origin.into(), deleted: false }
    }
    fn snap(device: &str, entries: &[(&str, Entry)]) -> Snapshot {
        Snapshot { schema: SCHEMA_VERSION, device_id: device.into(), generated_at: 0, entries: entries.iter().map(|(k, v)| (k.to_string(), v.clone())).collect() }
    }

    #[test]
    fn secrets_and_machine_specific_settings_never_sync() {
        for no in ["accounts", "aiConfig", "wallpaper", "displayScale", "desktopPath", "clonedApps", "installedPlugins", "parentalControls", "filePickerLastPaths", "blueGames", "appsEnabled"] {
            assert!(!is_syncable(no), "{no}");
        }
        for yes in ["theme", "customThemes", "shellThemeId", "language", "hotCorners", "store:a11y", "store:focus", "accentColor"] {
            assert!(is_syncable(yes), "{yes}");
        }
        // the deny-list holds even if someone later adds a bad name to the allow-list
        assert!(NEVER.iter().all(|n| !SYNC_FIELDS.iter().any(|f| f.to_ascii_lowercase().contains(n))));
    }

    #[test]
    fn last_writer_wins_with_a_deterministic_tie_break() {
        let a = e(json!("dark"), 100, "A");
        let b = e(json!("light"), 200, "B");
        assert!(newer(&b, &a) && !newer(&a, &b));
        let tie_a = e(json!(1), 100, "A");
        let tie_b = e(json!(2), 100, "B");
        assert!(newer(&tie_b, &tie_a) && !newer(&tie_a, &tie_b), "both sides must pick the same winner");
    }

    #[test]
    fn two_devices_converge_whatever_the_merge_order() {
        let mut x = Store::default();
        let mut y = Store::default();
        x.put_local("theme", Some(json!("dark")), 1000, "X");
        x.put_local("language", Some(json!("pl")), 1000, "X");
        y.put_local("theme", Some(json!("light")), 2000, "Y");
        y.put_local("accentColor", Some(json!("red")), 1500, "Y");
        let sx = x.snapshot("X", 3000);
        let sy = y.snapshot("Y", 3000);
        x.merge(&sy);
        y.merge(&sx);
        assert_eq!(x.entries, y.entries, "both stores must end up identical");
        assert_eq!(x.entries["theme"].value, json!("light"), "the later edit won");
        assert_eq!(x.entries["language"].value, json!("pl"));
        assert_eq!(x.entries["accentColor"].value, json!("red"));
    }

    #[test]
    fn applying_a_remote_value_and_reporting_it_back_does_not_echo() {
        let mut s = Store::default();
        s.merge(&snap("B", &[("theme", e(json!("light"), 500, "B"))]));
        // the shell applied it and now reports it as "local":
        assert!(!s.put_local("theme", Some(json!("light")), 900, "A"), "same value: no change, no new timestamp");
        assert_eq!(s.entries["theme"].origin, "B");
        assert!(s.put_local("theme", Some(json!("dark")), 901, "A"), "a real change does count");
    }

    #[test]
    fn a_local_edit_is_always_newer_than_what_it_replaces_even_if_the_clock_stepped_back() {
        let mut s = Store::default();
        s.merge(&snap("B", &[("theme", e(json!("light"), 5000, "B"))]));
        assert!(s.put_local("theme", Some(json!("dark")), 100, "A"));
        assert!(s.entries["theme"].updated_at > 5000);
    }

    #[test]
    fn deletions_propagate() {
        let mut a = Store::default();
        a.put_local("customBookmarks", Some(json!(["x"])), 100, "A");
        let mut b = Store::default();
        b.merge(&a.snapshot("A", 100));
        a.put_local("customBookmarks", None, 200, "A");
        let applied = b.merge(&a.snapshot("A", 200));
        assert!(applied.0["customBookmarks"].deleted);
        assert!(!a.put_local("theme", None, 300, "A"), "deleting something that never existed is not a change");
    }

    #[test]
    fn hostile_snapshots_are_filtered_clamped_and_bounded() {
        let now = 1_000_000;
        let big = json!("x".repeat(MAX_VALUE_BYTES + 1));
        let s = sanitize(
            snap("evil", &[("theme", e(json!("dark"), now + 10 * MAX_FUTURE_SKEW_MS, "evil")), ("accounts", e(json!({"k": "v"}), 1, "evil")), ("language", e(big, 1, "evil")), ("wallpaper", e(json!("/x"), 1, "evil"))]),
            now,
        )
        .unwrap();
        assert_eq!(s.entries.keys().collect::<Vec<_>>(), vec!["theme"], "only small, allow-listed keys survive");
        assert_eq!(s.entries["theme"].updated_at, now + MAX_FUTURE_SKEW_MS, "a far-future timestamp is clamped");
        let mut wrong = snap("x", &[]);
        wrong.schema = 99;
        assert!(sanitize(wrong, now).is_err());
        let many: Vec<(String, Entry)> = (0..MAX_ENTRIES * 2 + 1).map(|i| (format!("k{i}"), e(json!(1), 1, "x"))).collect();
        let too_many = Snapshot { schema: SCHEMA_VERSION, device_id: "x".into(), generated_at: 0, entries: many.into_iter().collect() };
        assert!(sanitize(too_many, now).is_err());
    }

    #[test]
    fn a_clock_running_far_ahead_cannot_win_forever() {
        let now = 1_000_000;
        let mut s = Store::default();
        s.put_local("theme", Some(json!("dark")), now, "A");
        let hostile = sanitize(snap("B", &[("theme", e(json!("light"), i64::MAX / 2, "B"))]), now).unwrap();
        s.merge(&hostile);
        assert_eq!(s.entries["theme"].updated_at, now + MAX_FUTURE_SKEW_MS);
        // a genuine edit made a day and a bit later beats the clamped value
        assert!(s.put_local("theme", Some(json!("dark")), now + MAX_FUTURE_SKEW_MS + 5, "A"));
    }

    #[test]
    fn exchange_requires_opt_in_and_returns_the_merged_whole() {
        let tmp = std::env::temp_dir().join(format!("blue-sync-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("BLUE_SYNC_DIR", &tmp);
        let theirs = snap("peer", &[("language", e(json!("de"), now_ms(), "peer"))]);
        assert!(handle_exchange("peer", theirs.clone(), "me").is_err(), "unknown device is refused");
        set_enabled("peer", true).unwrap();
        report_local("me", [("theme".to_string(), Some(json!("dark")))].into_iter().collect()).unwrap();
        let reply = handle_exchange("peer", theirs, "me").unwrap();
        assert_eq!(reply.entries["language"].value, json!("de"));
        assert_eq!(reply.entries["theme"].value, json!("dark"));
        let pending = take_pending();
        assert!(pending.contains_key("language") && !pending.contains_key("theme"));
        assert!(take_pending().is_empty(), "drained once");
        assert!(load().peers["peer"].last_sync.is_some());
        let _ = std::fs::remove_dir_all(tmp);
    }
}
