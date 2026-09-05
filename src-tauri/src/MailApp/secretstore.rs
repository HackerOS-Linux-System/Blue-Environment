use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use std::path::PathBuf;

fn key_path(mail_dir: &std::path::Path) -> PathBuf {
    mail_dir.join("secret.key")
}

/// Loads the local encryption key, generating and persisting a new random
/// one on first use. The file is created with `0600` permissions on Unix
/// (owner read/write only) so other local accounts can't read it outright.
fn load_or_create_key(mail_dir: &std::path::Path) -> std::io::Result<[u8; 32]> {
    let path = key_path(mail_dir);
    if let Ok(raw) = std::fs::read(&path) {
        if raw.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&raw);
            return Ok(key);
        }
        // Wrong length (corrupted or from some future format) — fall
        // through and regenerate rather than fail closed forever. Any
        // passwords encrypted under the old key become unreadable, but
        // that's the same "re-enter your password" recovery path a user
        // hits if they ever lose this file at all.
    }
    std::fs::create_dir_all(mail_dir)?;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    std::fs::write(&path, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

/// Encrypts `plaintext` under the local key, returning a single
/// self-contained base64 string (12-byte nonce prefix + ciphertext) safe
/// to store as one JSON string field.
pub fn encrypt(mail_dir: &std::path::Path, plaintext: &str) -> String {
    let Ok(key) = load_or_create_key(mail_dir) else { return String::new(); };
    let Ok(cipher) = Aes256Gcm::new_from_slice(&key) else { return String::new(); };
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let Ok(ct) = cipher.encrypt(nonce, plaintext.as_bytes()) else { return String::new(); };
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ct);
    BASE64.encode(combined)
}

/// Decrypts a string produced by [`encrypt`]. Returns an empty string
/// (never panics) if the key is missing/rotated or the blob is corrupt —
/// callers treat that the same as "no password configured", which just
/// makes the next IMAP/SMTP call fail with an auth error the user can
/// recover from by re-entering credentials in Settings.
pub fn decrypt(mail_dir: &std::path::Path, encoded: &str) -> String {
    let Ok(key) = load_or_create_key(mail_dir) else { return String::new(); };
    let Ok(cipher) = Aes256Gcm::new_from_slice(&key) else { return String::new(); };
    let Ok(combined) = BASE64.decode(encoded) else { return String::new(); };
    if combined.len() < 12 { return String::new(); }
    let (nonce_bytes, ct) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ct)
        .ok()
        .and_then(|pt| String::from_utf8(pt).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_then_decrypt_round_trips() {
        let dir = std::env::temp_dir().join(format!("blue-mail-secretstore-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let enc = encrypt(&dir, "hunter2");
        assert!(!enc.is_empty());
        assert_ne!(enc, "hunter2");
        assert_eq!(decrypt(&dir, &enc), "hunter2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn decrypting_garbage_yields_empty_string_not_a_panic() {
        let dir = std::env::temp_dir().join(format!("blue-mail-secretstore-test-garbage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(decrypt(&dir, "not-valid-base64!!"), "");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
