use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a user's pattern-lock hash is stored, relative to their home directory.
pub const PATTERN_HASH_RELATIVE_PATH: &str = ".config/Blue-Environment/pattern.hash";

/// Minimum number of cells a pattern must visit to be accepted.
/// (Android-style 3x3 grid, cells encoded as indices 0-8.)
pub const MIN_PATTERN_LEN: usize = 4;

/// Absolute path to the pattern-lock hash file for a given home directory.
pub fn pattern_hash_path(home: impl AsRef<Path>) -> PathBuf {
    home.as_ref().join(PATTERN_HASH_RELATIVE_PATH)
}

/// Validate pattern length before hashing/storing/comparing. Kept as a free
/// function so all three call sites reject the same "too short" patterns.
pub fn validate_pattern_len(pattern: &[u8]) -> Result<(), String> {
    if pattern.len() < MIN_PATTERN_LEN {
        Err(format!("Pattern too short (minimum {MIN_PATTERN_LEN} points)"))
    } else {
        Ok(())
    }
}

/// Combines username + raw pattern bytes into the single byte string that
/// gets fed to the KDF. Salting with the username (in addition to Argon2's
/// own per-hash random salt) means one user's stored hash can never be
/// replayed against another user's account even in the pathological case
/// of two users somehow sharing a salt.
fn pattern_material(username: &str, pattern: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(username.len() + 1 + pattern.len());
    buf.extend_from_slice(username.as_bytes());
    buf.push(b':');
    buf.extend_from_slice(pattern);
    buf
}

/// Hash a pattern with Argon2id, using a fresh random salt each time (the
/// result is a self-describing PHC string — algorithm, params, salt and
/// hash all travel together, same as Blue-Accounts' vault crypto).
///
/// This is the *current* format. See [`verify_pattern`] for how the old
/// bare-SHA-256 format (this project's format up through v0.7) is still
/// accepted and transparently upgraded, since a lock-screen change can't
/// force every already-installed user to re-draw their pattern.
pub fn hash_pattern(username: &str, pattern: &[u8]) -> Result<String, String> {
    let material = pattern_material(username, pattern);
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(&material, &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("failed to hash pattern: {e}"))
}

/// Legacy (pre-v0.8) hash: unsalted-cost SHA-256 of `"{username}:{pattern}"`,
/// stored as a bare 64-character lowercase hex digest with no algorithm tag.
/// Kept only so [`verify_pattern`] can recognise and migrate hashes written
/// by older builds — never used for new hashes.
fn legacy_sha256_hash(username: &str, pattern: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pattern_material(username, pattern));
    format!("{:x}", hasher.finalize())
}

/// A stored hash is legacy format iff it's exactly 64 hex characters (a raw
/// SHA-256 digest). Real Argon2 PHC strings always start with `$argon2` and
/// are a different length/alphabet, so this can't misfire on a real one.
fn looks_like_legacy_hash(stored: &str) -> bool {
    stored.len() == 64 && stored.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Verify a submitted pattern against the stored hash for `username`/`home`.
/// Fails closed: if no pattern has been configured, authentication fails
/// rather than silently succeeding.
///
/// Transparently migrates old installs: if the file on disk is still in the
/// legacy bare-SHA-256 format and the pattern checks out against it, this
/// re-hashes the pattern with Argon2id and rewrites the file before
/// returning `Ok`, so the weak format only ever gets read once per user.
pub fn verify_pattern(username: &str, home: impl AsRef<Path>, pattern: &[u8]) -> Result<(), String> {
    validate_pattern_len(pattern)?;
    let path = pattern_hash_path(home.as_ref());
    let stored = std::fs::read_to_string(&path)
        .map_err(|_| "No pattern configured for this user — set one up in Settings → Security".to_string())?;
    let stored = stored.trim();

    if looks_like_legacy_hash(stored) {
        if legacy_sha256_hash(username, pattern) == stored {
            // Correct pattern under the old scheme — migrate silently.
            // A failure to rewrite here isn't fatal to *this* login (the
            // pattern was already proven correct); it just means we'll
            // try the migration again next time.
            let _ = save_pattern(username, home, pattern);
            return Ok(());
        }
        return Err("Pattern not recognised".to_string());
    }

    let parsed = PasswordHash::new(stored)
        .map_err(|e| format!("corrupt pattern hash on disk: {e}"))?;
    let material = pattern_material(username, pattern);
    Argon2::default()
        .verify_password(&material, &parsed)
        .map_err(|_| "Pattern not recognised".to_string())
}

/// Store a new pattern-lock hash for `username`/`home`, creating the
/// `.config/Blue-Environment` directory if needed. Always writes the
/// current (Argon2id) format, regardless of what was there before.
pub fn save_pattern(username: &str, home: impl AsRef<Path>, pattern: &[u8]) -> Result<(), String> {
    validate_pattern_len(pattern)?;
    let path = pattern_hash_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let hash = hash_pattern(username, pattern)?;
    std::fs::write(&path, hash).map_err(|e| e.to_string())
}

/// Remove a user's stored pattern-lock hash, if any.
pub fn delete_pattern(home: impl AsRef<Path>) -> Result<(), String> {
    let path = pattern_hash_path(home);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Whether `username` has a pattern-lock hash on disk.
pub fn pattern_is_configured(home: impl AsRef<Path>) -> bool {
    pattern_hash_path(home).exists()
}

/// Whether `username` has at least one fingerprint enrolled via fprintd.
/// Shells out to `fprintd-list`, same as both BEDM's greeter and the main
/// desktop session previously did independently.
pub fn has_fingerprint(username: &str) -> bool {
    Command::new("fprintd-list")
        .arg(username)
        .output()
        .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).contains("no fingers"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    // Each test gets its own throwaway "home" directory under the system
    // temp dir so parallel `cargo test` runs never share a pattern.hash
    // file with each other (same isolation concern the module docs for
    // BLUE_MESSAGES_DIR / BLUE_THEMES_DIR call out elsewhere in this repo).
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    fn temp_home() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("blue-auth-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn same_pattern_different_users_hash_differently() {
        let a = hash_pattern("alice", &[0, 1, 2, 5]).unwrap();
        let b = hash_pattern("bob", &[0, 1, 2, 5]).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn two_hashes_of_the_same_pattern_never_match_verbatim() {
        // Argon2's random per-hash salt means the stored PHC strings
        // themselves differ even for the identical (username, pattern).
        let a = hash_pattern("alice", &[0, 1, 2, 5]).unwrap();
        let b = hash_pattern("alice", &[0, 1, 2, 5]).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn rejects_short_patterns() {
        assert!(validate_pattern_len(&[0, 1]).is_err());
        assert!(validate_pattern_len(&[0, 1, 2, 3]).is_ok());
    }

    #[test]
    fn save_then_verify_round_trips() {
        let home = temp_home();
        save_pattern("alice", &home, &[0, 1, 2, 5]).unwrap();
        assert!(verify_pattern("alice", &home, &[0, 1, 2, 5]).is_ok());
        assert!(verify_pattern("alice", &home, &[0, 1, 2, 6]).is_err());
    }

    #[test]
    fn legacy_sha256_hash_still_verifies_and_gets_migrated() {
        let home = temp_home();
        let path = pattern_hash_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, legacy_sha256_hash("alice", &[0, 1, 2, 5])).unwrap();

        // Old-format hash still authenticates the correct pattern...
        assert!(verify_pattern("alice", &home, &[0, 1, 2, 5]).is_ok());

        // ...and is transparently rewritten to Argon2id on that success.
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!looks_like_legacy_hash(on_disk.trim()));
        assert!(on_disk.starts_with("$argon2"));

        // Subsequent logins go through the new format and still work.
        assert!(verify_pattern("alice", &home, &[0, 1, 2, 5]).is_ok());
        assert!(verify_pattern("alice", &home, &[9, 8, 7, 6]).is_err());
    }

    #[test]
    fn wrong_pattern_against_legacy_hash_is_rejected_without_migrating() {
        let home = temp_home();
        let path = pattern_hash_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, legacy_sha256_hash("alice", &[0, 1, 2, 5])).unwrap();

        assert!(verify_pattern("alice", &home, &[1, 1, 1, 1]).is_err());
        // File must be untouched — a failed attempt migrates nothing.
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(looks_like_legacy_hash(on_disk.trim()));
    }
}
