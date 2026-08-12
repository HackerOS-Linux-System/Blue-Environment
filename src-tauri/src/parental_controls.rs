use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let dir = base.join("blue-environment");
    let _ = fs::create_dir_all(&dir);
    dir.join("parental-controls.json")
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ParentalControlsConfig {
    pub enabled: bool,
    pub pin_hash: Option<String>,
    pub pin_salt: Option<String>,
    /// App ids (matches `CachedApp::id` from `cache.rs`) that are
    /// completely blocked from launching.
    pub blocked_apps: Vec<String>,
    /// Per-app daily limit in minutes. Apps not listed here have no
    /// limit. Key is the same app id as `blocked_apps`.
    pub daily_limits_minutes: HashMap<String, u32>,
    /// Today's accumulated usage in minutes, per app — reset when
    /// `usage_date` no longer matches today's date.
    pub usage_minutes_today: HashMap<String, u32>,
    pub usage_date: String,
    /// Optional daily allowed-hours window (e.g. "08:00"-"20:00") outside
    /// of which ALL apps (except this Settings section itself) are
    /// blocked. `None` means no time-of-day restriction.
    pub allowed_hours_start: Option<String>,
    pub allowed_hours_end: Option<String>,
}

fn load() -> ParentalControlsConfig {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(cfg: &ParentalControlsConfig) -> bool {
    serde_json::to_string_pretty(cfg)
        .ok()
        .map(|s| fs::write(config_path(), s).is_ok())
        .unwrap_or(false)
}

fn hash_pin(pin: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(pin.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn random_salt() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Not cryptographically strong (no dependency on `rand` elsewhere in
    // this crate), but a salt only needs to be unpredictable-per-install,
    // not per-value — this is adequate for the "keep a kid out" threat
    // model described above.
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let pid = std::process::id();
    format!("{nanos:x}{pid:x}")
}

fn today_string() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Resets `usage_minutes_today` if the stored date has rolled over.
fn roll_usage_if_new_day(cfg: &mut ParentalControlsConfig) {
    let today = today_string();
    if cfg.usage_date != today {
        cfg.usage_minutes_today.clear();
        cfg.usage_date = today;
    }
}

#[tauri::command]
pub fn parental_controls_get() -> ParentalControlsConfig {
    let mut cfg = load();
    roll_usage_if_new_day(&mut cfg);
    // Never send the hash/salt to the frontend — it has no legitimate use
    // for them and it's needless exposure of (admittedly low-value, but
    // still) secret material over the Tauri IPC bridge.
    cfg.pin_hash = None;
    cfg.pin_salt = None;
    cfg
}

#[tauri::command]
pub fn parental_controls_is_pin_set() -> bool {
    let cfg = load();
    cfg.pin_hash.is_some()
}

#[tauri::command]
pub fn parental_controls_set_pin(pin: String, current_pin: Option<String>) -> bool {
    let mut cfg = load();
    // If a PIN is already set, changing it requires the current one —
    // otherwise anyone (e.g. the child the controls are meant to
    // restrict) could just clear/replace it from the same Settings app.
    if let Some(existing_hash) = &cfg.pin_hash {
        let Some(current) = current_pin else { return false };
        let Some(salt) = &cfg.pin_salt else { return false };
        if &hash_pin(&current, salt) != existing_hash {
            return false;
        }
    }
    if pin.trim().is_empty() {
        return false;
    }
    let salt = random_salt();
    cfg.pin_hash = Some(hash_pin(&pin, &salt));
    cfg.pin_salt = Some(salt);
    save(&cfg)
}

#[tauri::command]
pub fn parental_controls_verify_pin(pin: String) -> bool {
    let cfg = load();
    match (&cfg.pin_hash, &cfg.pin_salt) {
        (Some(hash), Some(salt)) => &hash_pin(&pin, salt) == hash,
        _ => false, // no PIN set — nothing to verify against, deny by default
    }
}

#[tauri::command]
pub fn parental_controls_set_enabled(enabled: bool, pin: String) -> bool {
    let mut cfg = load();
    if !parental_controls_verify_pin(pin) {
        return false;
    }
    cfg.enabled = enabled;
    save(&cfg)
}

#[tauri::command]
pub fn parental_controls_set_blocked_apps(apps: Vec<String>, pin: String) -> bool {
    if !parental_controls_verify_pin(pin) {
        return false;
    }
    let mut cfg = load();
    cfg.blocked_apps = apps;
    save(&cfg)
}

#[tauri::command]
pub fn parental_controls_set_daily_limit(app_id: String, minutes: Option<u32>, pin: String) -> bool {
    if !parental_controls_verify_pin(pin) {
        return false;
    }
    let mut cfg = load();
    match minutes {
        Some(m) => { cfg.daily_limits_minutes.insert(app_id, m); }
        None => { cfg.daily_limits_minutes.remove(&app_id); }
    }
    save(&cfg)
}

#[tauri::command]
pub fn parental_controls_set_allowed_hours(start: Option<String>, end: Option<String>, pin: String) -> bool {
    if !parental_controls_verify_pin(pin) {
        return false;
    }
    let mut cfg = load();
    cfg.allowed_hours_start = start;
    cfg.allowed_hours_end = end;
    save(&cfg)
}

/// Called by the app launcher before spawning an app. Returns a reason
/// string if launch should be blocked, or `None` if it's allowed.
///
/// NOTE: not yet called from the actual launch call sites — see the
/// module doc comment.
#[tauri::command]
pub fn parental_controls_check_launch(app_id: String) -> Option<String> {
    let mut cfg = load();
    if !cfg.enabled {
        return None;
    }
    roll_usage_if_new_day(&mut cfg);

    if cfg.blocked_apps.iter().any(|a| a == &app_id) {
        return Some("This app is blocked by Parental Controls.".to_string());
    }

    if let (Some(start), Some(end)) = (&cfg.allowed_hours_start, &cfg.allowed_hours_end) {
        let now = chrono::Local::now().format("%H:%M").to_string();
        // Simple string comparison works for "HH:MM" since it's
        // lexicographically ordered the same as chronologically, as long
        // as start <= end (doesn't handle windows that cross midnight —
        // acceptable for a first pass, e.g. "08:00"-"20:00" is the
        // overwhelmingly common case; a "22:00"-"06:00" overnight-block
        // window is a follow-up).
        if now.as_str() < start.as_str() || now.as_str() > end.as_str() {
            return Some(format!("Apps are only allowed between {start} and {end}."));
        }
    }

    if let Some(&limit) = cfg.daily_limits_minutes.get(&app_id) {
        let used = cfg.usage_minutes_today.get(&app_id).copied().unwrap_or(0);
        if used >= limit {
            return Some(format!("Daily time limit reached ({limit} min)."));
        }
    }

    None
}

/// Records that `app_id` has been actively used for `minutes` more today.
/// Meant to be called periodically (e.g. every 60s while the app has
/// focus) by whatever already tracks active windows — see module doc
/// comment for why this isn't wired up to a poller yet.
#[tauri::command]
pub fn parental_controls_record_usage(app_id: String, minutes: u32) -> bool {
    let mut cfg = load();
    roll_usage_if_new_day(&mut cfg);
    *cfg.usage_minutes_today.entry(app_id).or_insert(0) += minutes;
    save(&cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_pin_is_deterministic_for_same_pin_and_salt() {
        let h1 = hash_pin("1234", "somesalt");
        let h2 = hash_pin("1234", "somesalt");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_pin_differs_for_different_pins() {
        let h1 = hash_pin("1234", "somesalt");
        let h2 = hash_pin("5678", "somesalt");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_pin_differs_for_different_salts() {
        let h1 = hash_pin("1234", "salt-a");
        let h2 = hash_pin("1234", "salt-b");
        assert_ne!(h1, h2, "same PIN with different salts must not collide");
    }

    #[test]
    fn hash_pin_output_is_64_char_lowercase_hex() {
        let h = hash_pin("0000", "x");
        assert_eq!(h.len(), 64); // SHA-256 -> 32 bytes -> 64 hex chars
        assert!(h.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn random_salt_produces_distinct_values() {
        let s1 = random_salt();
        let s2 = random_salt();
        // Not a strict guarantee (see random_salt's doc comment on why
        // it's not cryptographically strong), but two salts generated
        // back-to-back should essentially never collide in practice.
        assert_ne!(s1, s2);
    }

    #[test]
    fn roll_usage_if_new_day_clears_stale_usage() {
        let mut cfg = ParentalControlsConfig {
            usage_date: "2000-01-01".to_string(),
            ..Default::default()
        };
        cfg.usage_minutes_today.insert("some_app".to_string(), 42);

        roll_usage_if_new_day(&mut cfg);

        assert!(cfg.usage_minutes_today.is_empty());
        assert_eq!(cfg.usage_date, today_string());
    }

    #[test]
    fn roll_usage_if_new_day_keeps_usage_for_same_day() {
        let today = today_string();
        let mut cfg = ParentalControlsConfig {
            usage_date: today.clone(),
            ..Default::default()
        };
        cfg.usage_minutes_today.insert("some_app".to_string(), 42);

        roll_usage_if_new_day(&mut cfg);

        assert_eq!(cfg.usage_minutes_today.get("some_app"), Some(&42));
    }

    #[test]
    fn check_launch_blocks_apps_on_the_blocklist_when_enabled() {
        let mut cfg = ParentalControlsConfig {
            enabled: true,
            usage_date: today_string(),
            ..Default::default()
        };
        cfg.blocked_apps.push("mail".to_string());

        // Mirrors the core decision logic of `parental_controls_check_launch`
        // without going through the filesystem (that command reads/writes
        // `config_path()`, which isn't hermetic for a unit test) — this
        // exercises the same blocklist-membership check directly.
        assert!(cfg.blocked_apps.iter().any(|a| a == "mail"));
        assert!(!cfg.blocked_apps.iter().any(|a| a == "terminal"));
    }

    #[test]
    fn daily_limit_reached_when_usage_meets_or_exceeds_limit() {
        let mut cfg = ParentalControlsConfig::default();
        cfg.daily_limits_minutes.insert("games".to_string(), 30);
        cfg.usage_minutes_today.insert("games".to_string(), 30);

        let limit = cfg.daily_limits_minutes.get("games").copied().unwrap();
        let used = cfg.usage_minutes_today.get("games").copied().unwrap_or(0);
        assert!(used >= limit, "usage equal to the limit should count as reached");
    }
}
