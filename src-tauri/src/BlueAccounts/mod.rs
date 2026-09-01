pub mod crypto;
pub mod vault;

use vault::VaultEntry;

#[tauri::command]
pub fn accounts_vault_exists() -> bool {
    vault::vault_exists()
}

#[tauri::command]
pub fn accounts_is_unlocked() -> bool {
    vault::is_unlocked()
}

#[tauri::command]
pub fn accounts_create_vault(master_password: String) -> Result<(), String> {
    vault::create_vault(&master_password).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn accounts_unlock(master_password: String) -> Result<usize, String> {
    vault::unlock(&master_password).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn accounts_lock() {
    vault::lock();
}

#[tauri::command]
pub fn accounts_list_entries() -> Result<Vec<VaultEntry>, String> {
    vault::list_entries().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn accounts_add_entry(entry: VaultEntry, master_password: String) -> Result<(), String> {
    vault::add_entry(entry, &master_password).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn accounts_update_entry(entry: VaultEntry, master_password: String) -> Result<(), String> {
    vault::update_entry(entry, &master_password).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn accounts_delete_entry(id: String, master_password: String) -> Result<(), String> {
    vault::delete_entry(&id, &master_password).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn accounts_change_master_password(new_password: String) -> Result<(), String> {
    vault::change_master_password(&new_password).map_err(|e| e.to_string())
}

/// Generates a random password of `length` characters from a
/// configurable character pool — real randomness (`rand::rngs::OsRng`,
/// the OS CSPRNG, same source `crypto.rs` uses for nonces/salts), not
/// `rand::thread_rng()`'s weaker-but-faster PRNG, since a generated
/// password is exactly the kind of output where "weaker but faster"
/// isn't an acceptable tradeoff.
#[tauri::command]
pub fn accounts_generate_password(length: usize, use_symbols: bool, use_digits: bool, use_uppercase: bool) -> String {
    use rand::RngCore;

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

    let mut rng = rand::rngs::OsRng;
    let length = length.clamp(4, 128);
    (0..length)
        .map(|_| {
            let idx = (rng.next_u32() as usize) % pool.len();
            pool[idx]
        })
        .collect()
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
