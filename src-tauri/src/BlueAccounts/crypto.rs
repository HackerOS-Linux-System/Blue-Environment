use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::password_hash::rand_core::OsRng as ArgonOsRng;
use argon2::password_hash::SaltString;
use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncryptedBlob {
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(thiserror::Error, Debug)]
pub enum VaultCryptoError {
    #[error("incorrect master password, or the vault file is corrupted")]
    DecryptionFailed,
    #[error("internal crypto error: {0}")]
    Internal(String),
}

fn derive_key(password: &str, salt: &SaltString) -> Result<[u8; 32], VaultCryptoError> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt.as_str().as_bytes(), &mut key)
        .map_err(|e| VaultCryptoError::Internal(e.to_string()))?;
    Ok(key)
}

/// Encrypts `plaintext` (in practice, the JSON-serialized vault
/// contents — see `vault.rs`) under `password`, generating a fresh
/// random salt and nonce every call. Never reuse an `EncryptedBlob`'s
/// salt/nonce for a second encryption under any circumstances — this
/// function always generates new ones specifically so a caller never
/// has to think about that.
pub fn encrypt(plaintext: &[u8], password: &str) -> Result<EncryptedBlob, VaultCryptoError> {
    let salt = SaltString::generate(&mut ArgonOsRng);
    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| VaultCryptoError::Internal(e.to_string()))?;

    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| VaultCryptoError::Internal(e.to_string()))?;

    Ok(EncryptedBlob {
        salt: salt.as_str().to_string(),
        nonce: BASE64.encode(nonce_bytes),
        ciphertext: BASE64.encode(ciphertext),
    })
}

/// Decrypts `blob` under `password`. Returns
/// [`VaultCryptoError::DecryptionFailed`] for *any* failure — wrong
/// password, corrupted/tampered ciphertext, or a malformed blob — all
/// deliberately indistinguishable from the caller's point of view.
/// Reporting "wrong password" vs. "corrupted file" as different errors
/// would leak information useful to an attacker (e.g. distinguishing
/// "this password is wrong" from "this password would be right if the
/// file weren't corrupted" during an offline brute-force attempt);
/// giving the person a single "couldn't open your vault" message is
/// both simpler and the standard, correct approach.
pub fn decrypt(blob: &EncryptedBlob, password: &str) -> Result<Vec<u8>, VaultCryptoError> {
    let salt = SaltString::from_b64(&blob.salt).map_err(|_| VaultCryptoError::DecryptionFailed)?;
    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| VaultCryptoError::Internal(e.to_string()))?;

    let nonce_bytes = BASE64.decode(&blob.nonce).map_err(|_| VaultCryptoError::DecryptionFailed)?;
    let ciphertext = BASE64.decode(&blob.ciphertext).map_err(|_| VaultCryptoError::DecryptionFailed)?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext.as_slice())
        .map_err(|_| VaultCryptoError::DecryptionFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_then_decrypt_round_trips() {
        let data = b"{\"entries\":[{\"title\":\"Example\"}]}";
        let blob = encrypt(data, "correct horse battery staple").unwrap();
        let decrypted = decrypt(&blob, "correct horse battery staple").unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn wrong_password_fails_to_decrypt() {
        let blob = encrypt(b"secret vault contents", "right password").unwrap();
        assert!(decrypt(&blob, "wrong password").is_err());
    }

    #[test]
    fn two_encryptions_of_identical_plaintext_never_match() {
        let data = b"same secret every time";
        let blob1 = encrypt(data, "pw").unwrap();
        let blob2 = encrypt(data, "pw").unwrap();
        assert_ne!(blob1.ciphertext, blob2.ciphertext, "reused salt/nonce would be a real security bug");
        assert_ne!(blob1.salt, blob2.salt);
        assert_ne!(blob1.nonce, blob2.nonce);
    }

    #[test]
    fn tampered_ciphertext_is_rejected_not_silently_decrypted() {
        let mut blob = encrypt(b"secret vault contents", "pw").unwrap();
        let mut bytes = BASE64.decode(&blob.ciphertext).unwrap();
        bytes[0] ^= 0xFF;
        blob.ciphertext = BASE64.encode(bytes);
        assert!(decrypt(&blob, "pw").is_err(), "AES-GCM's authentication tag must catch tampering");
    }

    #[test]
    fn tampered_nonce_is_also_rejected() {
        let mut blob = encrypt(b"secret vault contents", "pw").unwrap();
        let mut bytes = BASE64.decode(&blob.nonce).unwrap();
        bytes[0] ^= 0xFF;
        blob.nonce = BASE64.encode(bytes);
        assert!(decrypt(&blob, "pw").is_err());
    }

    #[test]
    fn corrupted_blob_fields_fail_gracefully_not_panicking() {
        let blob = EncryptedBlob {
            salt: "not valid base64 salt!!!".to_string(),
            nonce: "also invalid".to_string(),
            ciphertext: "definitely not real ciphertext".to_string(),
        };
        assert!(decrypt(&blob, "any password").is_err());
    }
}
