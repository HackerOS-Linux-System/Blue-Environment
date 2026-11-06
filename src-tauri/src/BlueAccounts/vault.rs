#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use super::crypto::{self, EncryptedBlob};
use crate::lock_ext::LockExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use zeroize::Zeroizing;

pub(super) fn accounts_dir() -> PathBuf {
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
fn backup_path() -> PathBuf {
    accounts_dir().join("vault.enc.bak")
}

/// Where an entry came from. Entries created through the password manager UI
/// have an empty / `manager` source; entries stored by other applications
/// through the Secret Service D-Bus API are `secret-service`.
pub const SOURCE_SECRET_SERVICE: &str = "secret-service";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntry {
    pub id: String,
    pub title: String,
    pub username: String,
    /// Kept in plaintext only ever inside the decrypted, in-memory
    /// `Vec<VaultEntry>` and inside the whole-vault plaintext that gets
    /// encrypted before ever touching disk — never written or logged
    /// unencrypted anywhere.
    pub password: String,
    pub url: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
    /// Secret Service attributes (`xdg:schema`, `service`, `account`, …). Empty
    /// for ordinary password-manager entries.
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
    /// MIME type of the secret (Secret Service `content_type`).
    #[serde(default)]
    pub content_type: String,
    /// `manager` (default) or `secret-service`.
    #[serde(default)]
    pub source: String,
    /// Name of the application that created a `secret-service` entry, when
    /// known (D-Bus sender → process name). Shown in the UI.
    #[serde(default)]
    pub created_by: String,
}

impl VaultEntry {
    pub fn is_secret_service(&self) -> bool {
        self.source == SOURCE_SECRET_SERVICE
    }
}

struct Session {
    entries: Vec<VaultEntry>,
    /// Cached master password (see module doc). `None` when only the
    /// per-call flow of the UI is in use is *not* possible: unlock always sets it.
    password: Zeroizing<String>,
    last_activity: Instant,
}

static VAULT_SESSION: Mutex<Option<Session>> = Mutex::new(None);

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

fn locked() -> AccountsError {
    AccountsError("Vault is locked".to_string())
}

pub fn vault_exists() -> bool {
    vault_path().is_file()
}

pub fn is_unlocked() -> bool {
    VAULT_SESSION.lock_safe().is_some()
}

/// Drops the decrypted entries *and* the cached master password (the
/// `Zeroizing` buffer overwrites itself on drop).
pub fn lock() {
    *VAULT_SESSION.lock_safe() = None;
}

/// Marks activity (any list/read/write) so the idle auto-lock does not fire
/// while the vault is actually in use.
fn touch(session: &mut Session) {
    session.last_activity = Instant::now();
}

/// Seconds since the last activity, or `None` when locked. Used by `idle.rs`.
pub fn idle_seconds() -> Option<u64> {
    VAULT_SESSION.lock_safe().as_ref().map(|s| s.last_activity.elapsed().as_secs())
}

/// Creates a brand-new, empty vault encrypted under `master_password`
/// and immediately unlocks it.
pub fn create_vault(master_password: &str) -> Result<(), AccountsError> {
    if vault_exists() {
        return Err(AccountsError("A vault already exists — use unlock instead of create".to_string()));
    }
    if master_password.is_empty() {
        return Err(AccountsError("Master password cannot be empty".to_string()));
    }
    let entries: Vec<VaultEntry> = Vec::new();
    save_encrypted(&entries, master_password)?;
    *VAULT_SESSION.lock_safe() = Some(Session {
        entries,
        password: Zeroizing::new(master_password.to_string()),
        last_activity: Instant::now(),
    });
    Ok(())
}

/// Decrypts `vault.enc` under `master_password`. On success the entries (and
/// the password, see module doc) are held in memory.
pub fn unlock(master_password: &str) -> Result<usize, AccountsError> {
    let raw = fs::read_to_string(vault_path()).map_err(|e| AccountsError(format!("Failed to read vault file: {e}")))?;
    let blob: EncryptedBlob = serde_json::from_str(&raw).map_err(|e| AccountsError(format!("Vault file is corrupted: {e}")))?;
    let plaintext = crypto::decrypt(&blob, master_password)?;
    let entries: Vec<VaultEntry> = serde_json::from_slice(&plaintext).map_err(|e| AccountsError(format!("Vault contents corrupted: {e}")))?;
    let count = entries.len();
    *VAULT_SESSION.lock_safe() = Some(Session {
        entries,
        password: Zeroizing::new(master_password.to_string()),
        last_activity: Instant::now(),
    });
    Ok(count)
}

/// Atomic, private write: tmp file (0600) → fsync → keep the old file as
/// `.bak` → rename over `vault.enc`.
fn write_atomic(json: &str) -> Result<(), AccountsError> {
    let io = |e: std::io::Error| AccountsError(e.to_string());
    let dir = accounts_dir();
    fs::create_dir_all(&dir).map_err(io)?;
    let tmp = dir.join("vault.enc.tmp");
    {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp).map_err(io)?;
        f.write_all(json.as_bytes()).map_err(io)?;
        f.sync_all().map_err(io)?;
    }
    if vault_path().is_file() {
        // Best effort: a failed backup must not block saving the new version.
        let _ = fs::copy(vault_path(), backup_path());
    }
    fs::rename(&tmp, vault_path()).map_err(io)
}

fn save_encrypted(entries: &[VaultEntry], master_password: &str) -> Result<(), AccountsError> {
    let plaintext = Zeroizing::new(serde_json::to_vec(entries).map_err(|e| AccountsError(e.to_string()))?);
    let blob = crypto::encrypt(&plaintext, master_password)?;
    let json = serde_json::to_string_pretty(&blob).map_err(|e| AccountsError(e.to_string()))?;
    write_atomic(&json)
}

/// Applies `f` to a copy of the entries, saves the copy, and only then makes
/// it the live state — so a rejected password or a failed write can never leave
/// the in-memory vault ahead of what is on disk.
///
/// `provided` is what the UI sent. An **empty** one means "use the session's
/// password": the Blue Accounts window can be (re)opened while the vault was
/// already unlocked elsewhere (e.g. via a Secret Service prompt), in which case
/// it never learned the password and must not re-encrypt the vault under "".
/// A non-empty one must match the session's, so a stale or wrong password can
/// never silently re-key the vault.
fn mutate_persist<T>(provided: &str, f: impl FnOnce(&mut Vec<VaultEntry>) -> Result<T, AccountsError>) -> Result<T, AccountsError> {
    let mut guard = VAULT_SESSION.lock_safe();
    let s = guard.as_mut().ok_or_else(locked)?;
    touch(s);
    if !provided.is_empty() && provided != s.password.as_str() {
        return Err(AccountsError("Incorrect master password".to_string()));
    }
    let mut next = s.entries.clone();
    let out = f(&mut next)?;
    save_encrypted(&next, &s.password)?;
    s.entries = next;
    Ok(out)
}

fn with_session<T>(f: impl FnOnce(&mut Session) -> Result<T, AccountsError>) -> Result<T, AccountsError> {
    let mut guard = VAULT_SESSION.lock_safe();
    let s = guard.as_mut().ok_or_else(locked)?;
    touch(s);
    f(s)
}

pub fn list_entries() -> Result<Vec<VaultEntry>, AccountsError> {
    with_session(|s| Ok(s.entries.clone()))
}

pub fn add_entry(entry: VaultEntry, master_password: &str) -> Result<(), AccountsError> {
    mutate_persist(master_password, |v| {
        v.push(entry);
        Ok(())
    })
}

pub fn update_entry(entry: VaultEntry, master_password: &str) -> Result<(), AccountsError> {
    mutate_persist(master_password, |v| {
        let existing = v.iter_mut().find(|e| e.id == entry.id).ok_or_else(|| AccountsError("Entry not found".to_string()))?;
        *existing = entry;
        Ok(())
    })
}

pub fn delete_entry(id: &str, master_password: &str) -> Result<(), AccountsError> {
    mutate_persist(master_password, |v| {
        v.retain(|e| e.id != id);
        Ok(())
    })
}

/// Re-encrypts the whole vault under `new_password` (fresh salt and nonce) and
/// updates the cached password, so the old one stops working immediately.
pub fn change_master_password(new_password: &str) -> Result<(), AccountsError> {
    if new_password.is_empty() {
        return Err(AccountsError("New master password cannot be empty".to_string()));
    }
    with_session(|s| {
        save_encrypted(&s.entries, new_password)?;
        s.password = Zeroizing::new(new_password.to_string());
        Ok(())
    })
}

// ── Secret Service helpers (no password argument: they use the cached one) ──

/// True when every `(k, v)` of `query` is present in `attrs` with the same value.
pub fn attributes_match(attrs: &BTreeMap<String, String>, query: &BTreeMap<String, String>) -> bool {
    query.iter().all(|(k, v)| attrs.get(k) == Some(v))
}

/// Entries whose attributes contain all of `query`. An empty query matches
/// every Secret Service entry (spec: SearchItems with no attributes).
pub fn find_by_attributes(query: &BTreeMap<String, String>) -> Result<Vec<VaultEntry>, AccountsError> {
    with_session(|s| Ok(s.entries.iter().filter(|e| e.is_secret_service() && attributes_match(&e.attributes, query)).cloned().collect()))
}

pub fn get_entry(id: &str) -> Result<Option<VaultEntry>, AccountsError> {
    with_session(|s| Ok(s.entries.iter().find(|e| e.id == id).cloned()))
}

/// Creates or (when `replace` and an entry with identical attributes exists)
/// updates a Secret Service item. Returns the entry id.
pub fn upsert_secret_item(mut entry: VaultEntry, replace: bool) -> Result<String, AccountsError> {
    entry.source = SOURCE_SECRET_SERVICE.to_string();
    mutate_persist("", |v| {
        if replace && !entry.attributes.is_empty() {
            if let Some(existing) = v.iter_mut().find(|e| e.is_secret_service() && e.attributes == entry.attributes) {
                existing.title = entry.title.clone();
                existing.password = entry.password.clone();
                existing.content_type = entry.content_type.clone();
                existing.updated_at = entry.updated_at.clone();
                return Ok(existing.id.clone());
            }
        }
        let id = entry.id.clone();
        v.push(entry);
        Ok(id)
    })
}

pub fn set_secret(id: &str, secret: &str, content_type: &str, now: &str) -> Result<(), AccountsError> {
    mutate_persist("", |v| {
        let e = v.iter_mut().find(|e| e.id == id).ok_or_else(|| AccountsError("Entry not found".to_string()))?;
        e.password = secret.to_string();
        if !content_type.is_empty() {
            e.content_type = content_type.to_string();
        }
        e.updated_at = now.to_string();
        Ok(())
    })
}

pub fn set_title_and_attributes(id: &str, title: Option<&str>, attrs: Option<BTreeMap<String, String>>, now: &str) -> Result<(), AccountsError> {
    mutate_persist("", |v| {
        let e = v.iter_mut().find(|e| e.id == id).ok_or_else(|| AccountsError("Entry not found".to_string()))?;
        if let Some(t) = title {
            e.title = t.to_string();
        }
        if let Some(a) = attrs {
            e.attributes = a;
        }
        e.updated_at = now.to_string();
        Ok(())
    })
}

pub fn delete_cached(id: &str) -> Result<(), AccountsError> {
    mutate_persist("", |v| {
        v.retain(|e| e.id != id);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    /// The vault session and `BLUE_ACCOUNTS_DIR` are process-wide, so tests that
    /// touch them must not run concurrently (they used to race).
    static TEST_GUARD: Mutex<()> = Mutex::new(());

    fn isolated_test_env() -> (std::sync::MutexGuard<'static, ()>, PathBuf) {
        let guard = TEST_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("blue-accounts-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        std::env::set_var("BLUE_ACCOUNTS_DIR", &dir);
        lock();
        (guard, dir)
    }

    fn sample_entry(id: &str) -> VaultEntry {
        VaultEntry {
            id: id.to_string(),
            title: "Example".to_string(),
            username: "user@example.com".to_string(),
            password: "hunter2".to_string(),
            url: "https://example.com".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            ..Default::default()
        }
    }

    fn attrs(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn create_unlock_add_and_relock_round_trips_through_real_disk_encryption() {
        let (_g, dir) = isolated_test_env();
        create_vault("master-pw").unwrap();
        assert!(is_unlocked());
        add_entry(sample_entry("e1"), "master-pw").unwrap();

        lock();
        assert!(!is_unlocked());
        assert!(list_entries().is_err(), "listing while locked must fail, not return stale data");

        assert_eq!(unlock("master-pw").unwrap(), 1);
        assert_eq!(list_entries().unwrap()[0].username, "user@example.com");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn wrong_master_password_refuses_to_unlock() {
        let (_g, dir) = isolated_test_env();
        create_vault("correct-pw").unwrap();
        add_entry(sample_entry("e1"), "correct-pw").unwrap();
        lock();
        assert!(unlock("wrong-pw").is_err());
        assert!(!is_unlocked(), "a failed unlock attempt must not leave the vault open");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cannot_create_a_second_vault_over_an_existing_one() {
        let (_g, dir) = isolated_test_env();
        create_vault("pw1").unwrap();
        lock();
        assert!(create_vault("pw2").is_err(), "must not silently overwrite an existing vault");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn update_and_delete_entries_persist_across_lock_unlock() {
        let (_g, dir) = isolated_test_env();
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
        let (_g, dir) = isolated_test_env();
        create_vault("old-pw").unwrap();
        add_entry(sample_entry("e1"), "old-pw").unwrap();
        change_master_password("new-pw").unwrap();
        lock();
        assert!(unlock("old-pw").is_err(), "old master password must stop working after a change");
        assert_eq!(unlock("new-pw").unwrap(), 1, "entries must survive a master-password change");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn vault_file_on_disk_never_contains_plaintext_password() {
        let (_g, dir) = isolated_test_env();
        create_vault("pw").unwrap();
        add_entry(sample_entry("e1"), "pw").unwrap();
        let raw = fs::read_to_string(vault_path()).unwrap();
        assert!(!raw.contains("hunter2"));
        assert!(!raw.contains("user@example.com"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_vault_json_without_new_fields_still_loads() {
        let json = r#"[{"id":"a","title":"T","username":"u","password":"p","url":"","notes":"","createdAt":"x","updatedAt":"y"}]"#;
        let entries: Vec<VaultEntry> = serde_json::from_str(json).unwrap();
        assert!(entries[0].attributes.is_empty());
        assert!(!entries[0].is_secret_service());
    }

    #[test]
    fn writes_are_atomic_private_and_keep_a_backup() {
        let (_g, dir) = isolated_test_env();
        create_vault("pw").unwrap();
        add_entry(sample_entry("e1"), "pw").unwrap();
        add_entry(sample_entry("e2"), "pw").unwrap();
        assert!(backup_path().is_file(), "the previous version must be kept as vault.enc.bak");
        assert!(!dir.join("vault.enc.tmp").exists(), "no tmp file may be left behind");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(vault_path()).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "vault must be readable only by its owner");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn secret_service_items_are_found_by_attributes_and_persist() {
        let (_g, dir) = isolated_test_env();
        create_vault("pw").unwrap();
        let mut e = sample_entry("s1");
        e.attributes = attrs(&[("service", "git"), ("account", "me")]);
        let id = upsert_secret_item(e, false).unwrap();
        assert_eq!(id, "s1");

        assert_eq!(find_by_attributes(&attrs(&[("service", "git")])).unwrap().len(), 1);
        assert!(find_by_attributes(&attrs(&[("service", "ssh")])).unwrap().is_empty());
        // manager entries must never leak through the Secret Service search
        add_entry(sample_entry("m1"), "pw").unwrap();
        assert_eq!(find_by_attributes(&BTreeMap::new()).unwrap().len(), 1);

        lock();
        unlock("pw").unwrap();
        let again = get_entry("s1").unwrap().unwrap();
        assert_eq!(again.source, SOURCE_SECRET_SERVICE);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn replace_updates_the_item_with_identical_attributes_instead_of_duplicating() {
        let (_g, dir) = isolated_test_env();
        create_vault("pw").unwrap();
        let mut a = sample_entry("s1");
        a.attributes = attrs(&[("service", "git")]);
        upsert_secret_item(a, false).unwrap();
        let mut b = sample_entry("s2");
        b.attributes = attrs(&[("service", "git")]);
        b.password = "rotated".into();
        let id = upsert_secret_item(b, true).unwrap();
        assert_eq!(id, "s1", "replace must reuse the existing item");
        assert_eq!(list_entries().unwrap().len(), 1);
        assert_eq!(get_entry("s1").unwrap().unwrap().password, "rotated");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn locking_wipes_the_session_and_blocks_cached_writes() {
        let (_g, dir) = isolated_test_env();
        create_vault("pw").unwrap();
        lock();
        assert!(upsert_secret_item(sample_entry("x"), false).is_err());
        assert!(idle_seconds().is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_password_means_session_password_and_a_wrong_one_is_rejected() {
        let (_g, dir) = isolated_test_env();
        create_vault("right").unwrap();
        // UI that never learned the password (vault unlocked via a Secret Service prompt):
        add_entry(sample_entry("e1"), "").unwrap();
        assert!(add_entry(sample_entry("e2"), "wrong").is_err(), "a wrong password must not re-key the vault");
        assert_eq!(list_entries().unwrap().len(), 1, "the rejected add must not have been applied to the saved vault");
        lock();
        assert!(unlock("").is_err(), "the vault must NOT have been re-encrypted under an empty password");
        assert_eq!(unlock("right").unwrap(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn changing_the_master_password_updates_the_cached_one() {
        let (_g, dir) = isolated_test_env();
        create_vault("old").unwrap();
        change_master_password("new").unwrap();
        // a cached write after the change must be decryptable with the NEW password
        let mut e = sample_entry("s1");
        e.attributes = attrs(&[("a", "b")]);
        upsert_secret_item(e, false).unwrap();
        lock();
        assert!(unlock("old").is_err());
        assert_eq!(unlock("new").unwrap(), 1);
        let _ = fs::remove_dir_all(dir);
    }
}
