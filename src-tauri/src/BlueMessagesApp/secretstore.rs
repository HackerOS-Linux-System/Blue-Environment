use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use std::path::PathBuf;

fn key_path(dir: &std::path::Path) -> PathBuf { dir.join("xmpp_secret.key") }

fn load_or_create_key(dir: &std::path::Path) -> std::io::Result<[u8; 32]> {
    let path = key_path(dir);
    if let Ok(raw) = std::fs::read(&path) {
        if raw.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&raw);
            return Ok(key);
        }
    }
    std::fs::create_dir_all(dir)?;
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

pub fn encrypt(dir: &std::path::Path, plaintext: &str) -> String {
    let Ok(key) = load_or_create_key(dir) else { return String::new(); };
    let Ok(cipher) = Aes256Gcm::new_from_slice(&key) else { return String::new(); };
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let Ok(ct) = cipher.encrypt(nonce, plaintext.as_bytes()) else { return String::new(); };
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ct);
    BASE64.encode(combined)
}

pub fn decrypt(dir: &std::path::Path, encoded: &str) -> String {
    let Ok(key) = load_or_create_key(dir) else { return String::new(); };
    let Ok(cipher) = Aes256Gcm::new_from_slice(&key) else { return String::new(); };
    let Ok(combined) = BASE64.decode(encoded) else { return String::new(); };
    if combined.len() < 12 { return String::new(); }
    let (nonce_bytes, ct) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ct).ok().and_then(|pt| String::from_utf8(pt).ok()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trips() {
        let dir = std::env::temp_dir().join(format!("blue-messages-secretstore-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let enc = encrypt(&dir, "s3cr3t");
        assert_ne!(enc, "s3cr3t");
        assert_eq!(decrypt(&dir, &enc), "s3cr3t");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
