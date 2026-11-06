pub mod audit;
pub mod crypto;
pub mod idle;
pub mod secret_service;
pub mod vault;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use vault::VaultEntry;

/// True once `org.freedesktop.secrets` was claimed (see `start_background`).
static SECRET_SERVICE_RUNNING: AtomicBool = AtomicBool::new(false);
/// Keeps the D-Bus connection (and so the service) alive for the whole process.
static SECRET_SERVICE_CONN: OnceLock<zbus::Connection> = OnceLock::new();

/// Starts the Secret Service D-Bus bridge and the vault idle auto-lock.
///
/// * `emit` delivers events to the shell (`secrets:unlock-request`,
///   `secrets:unlock-hint`, `secrets:prompt-done`, `accounts:auto-locked`).
pub fn start_background(emit: secret_service::Emitter) {
    let lock_emit = emit.clone();
    idle::spawn_idle_locker(move || lock_emit("accounts:auto-locked", serde_json::json!({})));
    tauri::async_runtime::spawn(async move {
        match secret_service::run(emit).await {
            Ok(Some(conn)) => {
                let _ = SECRET_SERVICE_CONN.set(conn);
                SECRET_SERVICE_RUNNING.store(true, Ordering::Relaxed);
            }
            Ok(None) => tracing::info!("Secret Service not started: another keyring owns the bus name"),
            Err(e) => tracing::warn!("Secret Service failed to start: {e}"),
        }
    });
}

#[tauri::command(async)]
pub fn accounts_vault_exists() -> bool {
    vault::vault_exists()
}

#[tauri::command(async)]
pub fn accounts_is_unlocked() -> bool {
    vault::is_unlocked()
}

#[tauri::command(async)]
pub fn accounts_create_vault(master_password: String) -> Result<(), String> {
    vault::create_vault(&master_password).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn accounts_unlock(master_password: String) -> Result<usize, String> {
    vault::unlock(&master_password).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn accounts_lock() {
    vault::lock();
}

#[tauri::command(async)]
pub fn accounts_list_entries() -> Result<Vec<VaultEntry>, String> {
    vault::list_entries().map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn accounts_add_entry(entry: VaultEntry, master_password: String) -> Result<(), String> {
    vault::add_entry(entry, &master_password).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn accounts_update_entry(entry: VaultEntry, master_password: String) -> Result<(), String> {
    vault::update_entry(entry, &master_password).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn accounts_delete_entry(id: String, master_password: String) -> Result<(), String> {
    vault::delete_entry(&id, &master_password).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn accounts_change_master_password(new_password: String) -> Result<(), String> {
    vault::change_master_password(&new_password).map_err(|e| e.to_string())
}

/// Generates a random password of `length` characters from a
/// configurable character pool — real randomness (`rand::rngs::OsRng`,
/// the OS CSPRNG, same source `crypto.rs` uses for nonces/salts), not
/// `rand::thread_rng()`'s weaker-but-faster PRNG, since a generated
/// password is exactly the kind of output where "weaker but faster"
/// isn't an acceptable tradeoff.
#[tauri::command(async)]
pub fn accounts_generate_password(length: usize, use_symbols: bool, use_digits: bool, use_uppercase: bool) -> String {
    let mut pool: Vec<char> = "abcdefghijklmnopqrstuvwxyz".chars().collect();
    if use_uppercase {
        pool.extend("ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars());
    }
    if use_digits {
        pool.extend("0123456789".chars());
    }
    if use_symbols {
        pool.extend("!@#$%^&*()-_=+[]{}".chars());
    }
    if pool.is_empty() {
        pool = "abcdefghijklmnopqrstuvwxyz".chars().collect(); // never return an empty password over an empty pool
    }

    // `gen_range` samples uniformly (rejection sampling) — a plain
    // `next_u32() % len` would slightly favour the first characters of the pool.
    use rand::Rng;
    let mut rng = rand::rngs::OsRng;
    let length = length.clamp(4, 128);
    (0..length).map(|_| pool[rng.gen_range(0..pool.len())]).collect()
}

// ── password health, auto-lock, Secret Service ─────────────────────────────

/// Weak / reused / empty password report for the manager's own entries.
#[tauri::command(async)]
pub fn accounts_audit() -> Result<audit::AuditReport, String> {
    vault::list_entries().map(|e| audit::audit(&e)).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn accounts_get_settings() -> idle::AccountsSettings {
    idle::load_settings()
}

#[tauri::command(async)]
pub fn accounts_set_settings(settings: idle::AccountsSettings) -> Result<idle::AccountsSettings, String> {
    idle::save_settings(settings)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretServiceStatus {
    pub running: bool,
    pub bus_name: &'static str,
}

#[tauri::command(async)]
pub fn secrets_service_status() -> SecretServiceStatus {
    SecretServiceStatus { running: SECRET_SERVICE_RUNNING.load(Ordering::Relaxed), bus_name: secret_service::BUS_NAME }
}

/// The shell's answer to a `secrets:unlock-request` dialog.
/// `password = None` dismisses. A wrong password is an `Err` and the request stays pending.
#[tauri::command(async)]
pub fn secrets_prompt_reply(prompt_id: String, password: Option<String>, create: bool) -> Result<(), String> {
    secret_service::prompt_reply(&prompt_id, password, create)
}

/// Which application read / stored / deleted which secret (newest first, last 100).
#[tauri::command(async)]
pub fn secrets_access_log() -> Vec<secret_service::AccessRecord> {
    secret_service::access_log()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_password_respects_requested_length() {
        let pw = accounts_generate_password(16, true, true, true);
        assert_eq!(pw.chars().count(), 16);
    }

    #[test]
    fn generated_password_respects_character_pool_restrictions() {
        let pw = accounts_generate_password(50, false, false, false);
        assert!(pw.chars().all(|c| c.is_ascii_lowercase()), "with every extra pool disabled, only lowercase letters should appear");
    }

    #[test]
    fn length_is_clamped_to_a_sane_range() {
        assert_eq!(accounts_generate_password(0, true, true, true).chars().count(), 4);
        assert_eq!(accounts_generate_password(9999, true, true, true).chars().count(), 128);
    }

    #[test]
    fn two_generated_passwords_are_not_identical() {
        // Not a rigorous randomness test, just a sanity check that
        // this isn't accidentally deterministic.
        let a = accounts_generate_password(24, true, true, true);
        let b = accounts_generate_password(24, true, true, true);
        assert_ne!(a, b);
    }
}
