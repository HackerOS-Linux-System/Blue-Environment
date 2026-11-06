#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use super::vault;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const DEFAULT_AUTO_LOCK_MINUTES: u32 = 15;
pub const MAX_AUTO_LOCK_MINUTES: u32 = 24 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountsSettings {
    #[serde(default = "default_minutes")]
    pub auto_lock_minutes: u32,
}

fn default_minutes() -> u32 {
    DEFAULT_AUTO_LOCK_MINUTES
}

impl Default for AccountsSettings {
    fn default() -> Self {
        Self { auto_lock_minutes: DEFAULT_AUTO_LOCK_MINUTES }
    }
}

fn settings_path() -> PathBuf {
    vault::accounts_dir().join("settings.json")
}

pub fn load_settings() -> AccountsSettings {
    fs::read_to_string(settings_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_settings(mut s: AccountsSettings) -> Result<AccountsSettings, String> {
    s.auto_lock_minutes = s.auto_lock_minutes.min(MAX_AUTO_LOCK_MINUTES);
    fs::create_dir_all(vault::accounts_dir()).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?;
    fs::write(settings_path(), json).map_err(|e| e.to_string())?;
    Ok(s)
}

/// Pure decision: should a vault idle for `idle_secs` be locked?
pub fn should_lock(idle_secs: u64, timeout_minutes: u32) -> bool {
    timeout_minutes > 0 && idle_secs >= u64::from(timeout_minutes) * 60
}

/// One tick of the watcher. Returns true when it locked the vault.
pub fn tick() -> bool {
    match vault::idle_seconds() {
        Some(idle) if should_lock(idle, load_settings().auto_lock_minutes) => {
            vault::lock();
            true
        }
        _ => false,
    }
}

/// Background thread: checks every 20 s and calls `on_lock` after locking so
/// the UI can show the lock screen.
pub fn spawn_idle_locker(on_lock: impl Fn() + Send + 'static) {
    let _ = std::thread::Builder::new().name("vault-idle-lock".into()).spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(20));
        if tick() {
            on_lock();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_disables_the_timer() {
        assert!(!should_lock(10_000_000, 0));
    }

    #[test]
    fn locks_exactly_at_the_threshold() {
        assert!(!should_lock(14 * 60 + 59, 15));
        assert!(should_lock(15 * 60, 15));
        assert!(should_lock(99_999, 15));
    }

    #[test]
    fn missing_settings_fall_back_to_the_default() {
        let s: AccountsSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.auto_lock_minutes, DEFAULT_AUTO_LOCK_MINUTES);
    }
}
