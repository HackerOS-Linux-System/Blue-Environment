use super::crypto::{self, EncryptedBlob};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

fn accounts_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BLUE_ACCOUNTS_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/Blue-Environment/blue-accounts")
}
fn vault_path() -> PathBuf {
    accounts_dir().join("vault.enc")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntry {
    pub id: String,
    pub title: String,
    pub username: String,
    /// Kept in plaintext only ever inside the decrypted, in-memory
    /// `Vec<VaultEntry>` (see module doc's "Session model") and inside
    /// the whole-vault plaintext that gets encrypted before ever
    /// touching disk — never written or logged unencrypted anywhere.
    pub password: String,
    pub url: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

static VAULT_SESSION: Mutex<Option<Vec<VaultEntry>>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
pub struct AccountsError(pub String);
impl From<crypto::VaultCryptoError> for AccountsError {
    fn from(e: crypto::VaultCryptoError) -> Self {
        AccountsError(e.to_string())
    }
}
impl std::fmt::Display for AccountsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn vault_exists() -> bool {
    vault_path().is_file()
}

pub fn is_unlocked() -> bool {
    VAULT_SESSION.lock().unwrap().is_some()
}

pub fn lock() {
    *VAULT_SESSION.lock().unwrap() = None;
}

/// Creates a brand-new, empty vault encrypted under `master_password`
/// and immediately unlocks it (a freshly-created vault opening locked
/// would just mean unlocking it again with the password the person
/// just typed seconds ago — unnecessary friction).
pub fn create_vault(master_password: &str) -> Result<(), AccountsError> {
    if vault_exists() {
        return Err(AccountsError("A vault already exists — use unlock instead of create".to_string()));
    }
    if master_password.is_empty() {
        return Err(AccountsError("Master password cannot be empty".to_string()));
    }
    let entries: Vec<VaultEntry> = Vec::new();
    save_encrypted(&entries, master_password)?;
    *VAULT_SESSION.lock().unwrap() = Some(entries);
    Ok(())
}

/// Decrypts `vault.enc` under `master_password` and, on success, holds
/// the entries in memory for subsequent `accounts_*` calls — see
/// module doc's "Session model".
pub fn unlock(master_password: &str) -> Result<usize, AccountsError> {
    let raw = fs::read_to_string(vault_path()).map_err(|e| AccountsError(format!("Failed to read vault file: {e}")))?;
    let blob: EncryptedBlob = serde_json::from_str(&raw).map_err(|e| AccountsError(format!("Vault file is corrupted: {e}")))?;
    let plaintext = crypto::decrypt(&blob, master_password)?;
    let entries: Vec<VaultEntry> = serde_json::from_slice(&plaintext).map_err(|e| AccountsError(format!("Vault contents corrupted: {e}")))?;
    let count = entries.len();
    *VAULT_SESSION.lock().unwrap() = Some(entries);
    Ok(count)
}

fn save_encrypted(entries: &[VaultEntry], master_password: &str) -> Result<(), AccountsError> {
    let plaintext = serde_json::to_vec(entries).map_err(|e| AccountsError(e.to_string()))?;
    let blob = crypto::encrypt(&plaintext, master_password)?;
    fs::create_dir_all(accounts_dir()).map_err(|e| AccountsError(e.to_string()))?;
    let json = serde_json::to_string_pretty(&blob).map_err(|e| AccountsError(e.to_string()))?;
    fs::write(vault_path(), json).map_err(|e| AccountsError(e.to_string()))
}

/// Re-encrypts and saves the current in-memory vault under
/// `master_password`. Called after every mutation
/// (add/update/delete/change-password) with the *same* password used
/// to unlock — see `mod.rs`'s commands for where that password comes
/// from on each call (kept only transiently per-call from the
/// frontend, never cached in this module — see module doc).
fn persist(master_password: &str) -> Result<(), AccountsError> {
    let guard = VAULT_SESSION.lock().unwrap();
    let entries = guard.as_ref().ok_or_else(|| AccountsError("Vault is locked".to_string()))?;
    save_encrypted(entries, master_password)
}

pub fn list_entries() -> Result<Vec<VaultEntry>, AccountsError> {
    VAULT_SESSION.lock().unwrap().clone().ok_or_else(|| AccountsError("Vault is locked".to_string()))
}

pub fn add_entry(entry: VaultEntry, master_password: &str) -> Result<(), AccountsError> {
    {
        let mut guard = VAULT_SESSION.lock().unwrap();
        let entries = guard.as_mut().ok_or_else(|| AccountsError("Vault is locked".to_string()))?;
        entries.push(entry);
    }
    persist(master_password)
}

pub fn update_entry(entry: VaultEntry, master_password: &str) -> Result<(), AccountsError> {
    {
        let mut guard = VAULT_SESSION.lock().unwrap();
        let entries = guard.as_mut().ok_or_else(|| AccountsError("Vault is locked".to_string()))?;
        let existing = entries.iter_mut().find(|e| e.id == entry.id).ok_or_else(|| AccountsError("Entry not found".to_string()))?;
        *existing = entry;
    }
    persist(master_password)
}

pub fn delete_entry(id: &str, master_password: &str) -> Result<(), AccountsError> {
    {
        let mut guard = VAULT_SESSION.lock().unwrap();
        let entries = guard.as_mut().ok_or_else(|| AccountsError("Vault is locked".to_string()))?;
        entries.retain(|e| e.id != id);
    }
    persist(master_password)
}

/// Re-encrypts the whole vault under `new_password` — a real master
/// password change, not just relabeling: every entry is decrypted (via
/// the already-unlocked in-memory copy) and re-encrypted with a fresh
/// salt/nonce derived from `new_password`, so `old_password` stops
/// working the moment this returns successfully.
pub fn change_master_password(new_password: &str) -> Result<(), AccountsError> {
    if new_password.is_empty() {
        return Err(AccountsError("New master password cannot be empty".to_string()));
    }
    persist(new_password)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn isolated_test_env() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("blue-accounts-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        std::env::set_var("BLUE_ACCOUNTS_DIR", &dir);
        lock(); // ensure no state leaks between tests sharing the static VAULT_SESSION
        dir
    }

    fn sample_entry(id: &str) -> VaultEntry {
        VaultEntry {
            id: id.to_string(),
            title: "Example".to_string(),
            username: "user@example.com".to_string(),
            password: "hunter2".to_string(),
            url: "https://example.com".to_string(),
            notes: String::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn create_unlock_add_and_relock_round_trips_through_real_disk_encryption() {
        let dir = isolated_test_env();

        create_vault("master-pw").unwrap();
        assert!(is_unlocked());
        add_entry(sample_entry("e1"), "master-pw").unwrap();

        lock();
        assert!(!is_unlocked());
        assert!(list_entries().is_err(), "listing while locked must fail, not return stale data");

        let count = unlock("master-pw").unwrap();
        assert_eq!(count, 1);
        let entries = list_entries().unwrap();
        assert_eq!(entries[0].username, "user@example.com");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn wrong_master_password_refuses_to_unlock() {
        let dir = isolated_test_env();
        create_vault("correct-pw").unwrap();
        add_entry(sample_entry("e1"), "correct-pw").unwrap();
        lock();

        assert!(unlock("wrong-pw").is_err());
        assert!(!is_unlocked(), "a failed unlock attempt must not leave the vault open");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cannot_create_a_second_vault_over_an_existing_one() {
        let dir = isolated_test_env();
        create_vault("pw1").unwrap();
        lock();
        assert!(create_vault("pw2").is_err(), "must not silently overwrite an existing vault");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn update_and_delete_entries_persist_across_lock_unlock() {
        let dir = isolated_test_env();
        create_vault("pw").unwrap();
        add_entry(sample_entry("e1"), "pw").unwrap();

        let mut updated = sample_entry("e1");
        updated.password = "new-password-value".to_string();
        update_entry(updated, "pw").unwrap();

        lock();
        unlock("pw").unwrap();
        assert_eq!(list_entries().unwrap()[0].password, "new-password-value");

        delete_entry("e1", "pw").unwrap();
        lock();
        unlock("pw").unwrap();
        assert!(list_entries().unwrap().is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn changing_master_password_invalidates_the_old_one() {
        let dir = isolated_test_env();
        create_vault("old-pw").unwrap();
        add_entry(sample_entry("e1"), "old-pw").unwrap();
        change_master_password("new-pw").unwrap();
        lock();

        assert!(unlock("old-pw").is_err(), "old master password must stop working after a change");
        let count = unlock("new-pw").unwrap();
        assert_eq!(count, 1, "entries must survive a master-password change");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn vault_file_on_disk_never_contains_plaintext_password() {
        let dir = isolated_test_env();
        create_vault("pw").unwrap();
        add_entry(sample_entry("e1"), "pw").unwrap();

        let raw = fs::read_to_string(vault_path()).unwrap();
        assert!(!raw.contains("hunter2"), "the on-disk vault file must never contain a plaintext password");
        assert!(!raw.contains("user@example.com"), "or a plaintext username");

        let _ = fs::remove_dir_all(dir);
    }
}
