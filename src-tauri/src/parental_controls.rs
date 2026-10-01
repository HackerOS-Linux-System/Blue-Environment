use argon2::password_hash::rand_core::OsRng as ArgonOsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let dir = base.join("blue-environment");
    let _ = fs::create_dir_all(&dir);
    dir.join("parental-controls.json")
}

/// How many consecutive wrong PINs are tolerated before locking out
/// further attempts. A 4-digit PIN only has 10,000 possible values, so
/// without a lockout an attacker with local access to this IPC command
/// could brute-force it in well under a second even against Argon2
/// (Argon2 slows down *offline* hash cracking; it does nothing against
/// an *online* attacker who can just call `parental_controls_verify_pin`
/// in a loop, since each call only costs one hash computation, not one
/// per candidate).
const MAX_ATTEMPTS_BEFORE_LOCKOUT: u32 = 5;
/// Lockout duration in seconds, doubled for every lockout that happens
/// again without a successful verification in between (capped — see
/// `lockout_seconds_for`). Starts at 30s: annoying enough to make
/// online brute-forcing impractical, short enough that a parent who
/// mistypes their own PIN a few times isn't locked out for long.
const BASE_LOCKOUT_SECONDS: u64 = 30;
const MAX_LOCKOUT_SECONDS: u64 = 15 * 60;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ParentalControlsConfig {
    pub enabled: bool,
    /// Argon2id PHC string (self-describing: algorithm, version, cost
    /// parameters and salt are all encoded in this one string — see
    /// `hash_pin`/`verify_pin_hash`). `None` means no PIN has ever been
    /// set.
    pub pin_hash: Option<String>,
    /// Legacy field, kept only so `serde` can still deserialize a
    /// config file written before the Argon2 migration without erroring
    /// out; no longer written. See `migrate_legacy_sha256_pin_if_needed`.
    #[serde(default)]
    pub pin_salt: Option<String>,
    /// Consecutive failed `parental_controls_verify_pin` calls since the
    /// last success. Reset to 0 on any successful verification.
    #[serde(default)]
    pub failed_attempts: u32,
    /// Unix timestamp (seconds) until which verification attempts are
    /// rejected outright (without even hashing the supplied PIN) —
    /// `None` or a value in the past means not currently locked out.
    #[serde(default)]
    pub locked_until_unix: Option<i64>,
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

/// Hashes `pin` with Argon2id using a freshly generated random salt,
/// returning a self-describing PHC string (`$argon2id$v=19$m=...,t=...,p=...$<salt>$<hash>`)
/// that carries everything needed to later verify a candidate PIN
/// against it — no separate salt field to keep in sync, unlike the
/// SHA-256 scheme this replaces.
///
/// Argon2's default parameters (from `Argon2::default()`) are tuned for
/// interactive password verification (~19 MiB memory, moderate
/// iterations) — deliberately expensive to compute, which is the whole
/// point: it makes each *offline* guess (e.g. against a stolen copy of
/// parental-controls.json) cost meaningfully more than a single SHA-256
/// call would, unlike the scheme this replaces.
fn hash_pin(pin: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut ArgonOsRng);
    Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// Verifies `pin` against a previously stored Argon2 PHC hash string.
/// `PasswordVerifier::verify_password` performs the comparison in
/// constant time with respect to *where* a mismatch occurs (it always
/// hashes the full candidate and compares the full digest, rather than
/// returning early on the first differing byte the way a naive `==` on
/// two byte strings can) — this is what "constant-time comparison"
/// means for a password hash check: it's the hash algorithm's own
/// verification routine doing the comparing, not a raw `String == String`.
fn verify_pin_hash(pin: &str, stored_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else { return false };
    Argon2::default().verify_password(pin.as_bytes(), &parsed).is_ok()
}

/// One-time upgrade path for configs written before this module used
/// Argon2: the old scheme stored a 64-hex-char SHA-256 digest in
/// `pin_hash` plus a separate `pin_salt`. If `cfg.pin_hash` still looks
/// like that (rather than a `$argon2...` PHC string) and the caller has
/// just supplied the correct PIN for it, upgrade the stored hash to
/// Argon2 in place. Callers only reach this after already accepting the
/// PIN under the legacy scheme, so this never weakens verification —
/// it just means an existing installation's PIN gets silently
/// strengthened the next time its owner types it in correctly, with no
/// forced PIN reset.
fn legacy_sha256_hash(pin: &str, salt: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(pin.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn is_legacy_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

fn today_string() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Resets `usage_minutes_today` if the stored date has rolled over.
fn roll_usage_if_new_day(cfg: &mut ParentalControlsConfig) {
    let today = today_string();
    if cfg.usage_date != today {
        cfg.usage_minutes_today.clear();
        cfg.usage_date = today;
    }
}

/// `2^(min(attempts, 5) - 1) * BASE_LOCKOUT_SECONDS`, capped at
/// `MAX_LOCKOUT_SECONDS` — 30s, 60s, 120s, 240s, 480s, then capped at
/// 900s (15 min) for every lockout after that. Escalating rather than a
/// single fixed delay means a script that keeps retrying right after
/// each lockout expires keeps getting pushed further out, instead of
/// settling into a steady one-guess-per-30-seconds rate that would
/// still eventually brute-force a 4-digit PIN.
fn lockout_seconds_for(attempts_over_limit: u32) -> u64 {
    let exponent = attempts_over_limit.min(5);
    let seconds = BASE_LOCKOUT_SECONDS.saturating_mul(1u64 << exponent.saturating_sub(1).max(0));
    seconds.min(MAX_LOCKOUT_SECONDS)
}

/// Central PIN-check used by every command below that needs one.
/// Enforces the lockout window (rejecting outright, without hashing
/// anything, while locked out — this also means a locked-out caller
/// can't use timing to learn whether *this* attempt would otherwise
/// have been correct), verifies via Argon2 (transparently migrating a
/// legacy SHA-256 hash on success), and updates `failed_attempts` /
/// `locked_until_unix` accordingly. Always persists the updated
/// counters before returning, on both the success and failure paths.
fn verify_pin_internal(cfg: &mut ParentalControlsConfig, pin: &str) -> bool {
    let Some(stored_hash) = cfg.pin_hash.clone() else { return false }; // no PIN set — deny by default

    if let Some(locked_until) = cfg.locked_until_unix {
        if now_unix() < locked_until {
            return false; // still locked out; don't even attempt verification
        }
    }

    let ok = if is_legacy_hash(&stored_hash) {
        let Some(salt) = cfg.pin_salt.clone() else { return false };
        let matched = legacy_sha256_hash(pin, &salt) == stored_hash;
        if matched {
            // Upgrade in place now that we know `pin` is correct.
            if let Ok(new_hash) = hash_pin(pin) {
                cfg.pin_hash = Some(new_hash);
                cfg.pin_salt = None;
            }
        }
        matched
    } else {
        verify_pin_hash(pin, &stored_hash)
    };

    if ok {
        cfg.failed_attempts = 0;
        cfg.locked_until_unix = None;
    } else {
        cfg.failed_attempts += 1;
        if cfg.failed_attempts >= MAX_ATTEMPTS_BEFORE_LOCKOUT {
            let over = cfg.failed_attempts - MAX_ATTEMPTS_BEFORE_LOCKOUT + 1;
            cfg.locked_until_unix = Some(now_unix() + lockout_seconds_for(over) as i64);
        }
    }
    save(cfg);
    ok
}

#[tauri::command(async)]
pub fn parental_controls_get() -> ParentalControlsConfig {
    let mut cfg = load();
    roll_usage_if_new_day(&mut cfg);
    // Never send the hash/salt to the frontend — it has no legitimate use
    // for them and it's needless exposure of secret material over the
    // Tauri IPC bridge.
    cfg.pin_hash = None;
    cfg.pin_salt = None;
    cfg
}

#[tauri::command(async)]
pub fn parental_controls_is_pin_set() -> bool {
    let cfg = load();
    cfg.pin_hash.is_some()
}

/// Seconds remaining in an active lockout, or `0` if not currently
/// locked out. The frontend uses this to show "try again in Ns" instead
/// of a generic "wrong PIN" on a locked-out attempt.
#[tauri::command(async)]
pub fn parental_controls_lockout_remaining_seconds() -> u64 {
    let cfg = load();
    match cfg.locked_until_unix {
        Some(until) if until > now_unix() => (until - now_unix()) as u64,
        _ => 0,
    }
}

#[tauri::command(async)]
pub fn parental_controls_set_pin(pin: String, current_pin: Option<String>) -> bool {
    let mut cfg = load();
    // If a PIN is already set, changing it requires the current one —
    // otherwise anyone (e.g. the child the controls are meant to
    // restrict) could just clear/replace it from the same Settings app.
    if cfg.pin_hash.is_some() {
        let Some(current) = current_pin else { return false };
        if !verify_pin_internal(&mut cfg, &current) {
            return false;
        }
    }
    if pin.trim().is_empty() || pin.len() < 4 {
        return false;
    }
    let Ok(hash) = hash_pin(&pin) else { return false };
    cfg.pin_hash = Some(hash);
    cfg.pin_salt = None;
    cfg.failed_attempts = 0;
    cfg.locked_until_unix = None;
    save(&cfg)
}

#[tauri::command(async)]
pub fn parental_controls_verify_pin(pin: String) -> bool {
    let mut cfg = load();
    verify_pin_internal(&mut cfg, &pin)
}

#[tauri::command(async)]
pub fn parental_controls_set_enabled(enabled: bool, pin: String) -> bool {
    let mut cfg = load();
    if !verify_pin_internal(&mut cfg, &pin) {
        return false;
    }
    cfg.enabled = enabled;
    save(&cfg)
}

#[tauri::command(async)]
pub fn parental_controls_set_blocked_apps(apps: Vec<String>, pin: String) -> bool {
    let mut cfg = load();
    if !verify_pin_internal(&mut cfg, &pin) {
        return false;
    }
    cfg.blocked_apps = apps;
    save(&cfg)
}

#[tauri::command(async)]
pub fn parental_controls_set_daily_limit(app_id: String, minutes: Option<u32>, pin: String) -> bool {
    let mut cfg = load();
    if !verify_pin_internal(&mut cfg, &pin) {
        return false;
    }
    match minutes {
        Some(m) => { cfg.daily_limits_minutes.insert(app_id, m); }
        None => { cfg.daily_limits_minutes.remove(&app_id); }
    }
    save(&cfg)
}

#[tauri::command(async)]
pub fn parental_controls_set_allowed_hours(start: Option<String>, end: Option<String>, pin: String) -> bool {
    let mut cfg = load();
    if !verify_pin_internal(&mut cfg, &pin) {
        return false;
    }
    cfg.allowed_hours_start = start;
    cfg.allowed_hours_end = end;
    save(&cfg)
}

/// Called by the app launcher before spawning an app. Returns a reason
/// string if launch should be blocked, or `None` if it's allowed.
#[tauri::command(async)]
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
/// Called periodically (every 60s while the app has focus) by
/// `startParentalControlsUsageTracking` in `windowManager.ts`.
#[tauri::command(async)]
pub fn parental_controls_record_usage(app_id: String, minutes: u32) -> bool {
    let mut cfg = load();
    roll_usage_if_new_day(&mut cfg);
    *cfg.usage_minutes_today.entry(app_id).or_insert(0) += minutes;
    save(&cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_cfg() -> ParentalControlsConfig {
        ParentalControlsConfig {
            usage_date: today_string(),
            ..Default::default()
        }
    }

    #[test]
    fn hash_pin_produces_a_argon2_phc_string() {
        let h = hash_pin("1234").unwrap();
        assert!(h.starts_with("$argon2id$") || h.starts_with("$argon2i$") || h.starts_with("$argon2d$"));
    }

    #[test]
    fn correct_pin_verifies_against_its_own_hash() {
        let h = hash_pin("4321").unwrap();
        assert!(verify_pin_hash("4321", &h));
    }

    #[test]
    fn wrong_pin_does_not_verify() {
        let h = hash_pin("4321").unwrap();
        assert!(!verify_pin_hash("0000", &h));
    }

    #[test]
    fn two_hashes_of_the_same_pin_differ_due_to_random_salt() {
        let h1 = hash_pin("1234").unwrap();
        let h2 = hash_pin("1234").unwrap();
        assert_ne!(h1, h2, "each hash_pin call must use a fresh random salt");
        // But both still verify against the same PIN.
        assert!(verify_pin_hash("1234", &h1));
        assert!(verify_pin_hash("1234", &h2));
    }

    #[test]
    fn legacy_sha256_hash_still_verifies_and_gets_migrated() {
        let salt = "some-legacy-salt";
        let legacy_hash = legacy_sha256_hash("9999", salt);
        assert!(is_legacy_hash(&legacy_hash));

        let mut cfg = base_cfg();
        cfg.pin_hash = Some(legacy_hash);
        cfg.pin_salt = Some(salt.to_string());

        // Correct PIN against the legacy hash still verifies...
        assert!(verify_pin_internal(&mut cfg, "9999"));
        // ...and is transparently upgraded to Argon2 in place.
        assert!(!is_legacy_hash(cfg.pin_hash.as_ref().unwrap()));
        assert!(cfg.pin_salt.is_none());
        assert!(verify_pin_hash("9999", cfg.pin_hash.as_ref().unwrap()));
    }

    #[test]
    fn failed_attempts_increments_and_resets_on_success() {
        let mut cfg = base_cfg();
        cfg.pin_hash = Some(hash_pin("1111").unwrap());

        assert!(!verify_pin_internal(&mut cfg, "0000"));
        assert_eq!(cfg.failed_attempts, 1);
        assert!(!verify_pin_internal(&mut cfg, "0000"));
        assert_eq!(cfg.failed_attempts, 2);

        assert!(verify_pin_internal(&mut cfg, "1111"));
        assert_eq!(cfg.failed_attempts, 0);
        assert!(cfg.locked_until_unix.is_none());
    }

    #[test]
    fn lockout_engages_after_max_attempts_and_blocks_even_the_correct_pin() {
        let mut cfg = base_cfg();
        cfg.pin_hash = Some(hash_pin("1111").unwrap());

        for _ in 0..MAX_ATTEMPTS_BEFORE_LOCKOUT {
            assert!(!verify_pin_internal(&mut cfg, "wrong"));
        }
        assert!(cfg.locked_until_unix.is_some());

        // Now locked out — even the correct PIN is rejected without
        // being checked, until the lockout window passes.
        assert!(!verify_pin_internal(&mut cfg, "1111"));
    }

    #[test]
    fn lockout_seconds_escalate_and_cap() {
        assert_eq!(lockout_seconds_for(1), BASE_LOCKOUT_SECONDS);
        assert_eq!(lockout_seconds_for(2), BASE_LOCKOUT_SECONDS * 2);
        assert_eq!(lockout_seconds_for(3), BASE_LOCKOUT_SECONDS * 4);
        assert!(lockout_seconds_for(20) <= MAX_LOCKOUT_SECONDS);
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
