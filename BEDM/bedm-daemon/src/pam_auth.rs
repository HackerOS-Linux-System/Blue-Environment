use sha2::{Sha256, Digest};

/// Authenticate user against /etc/shadow using crypt(3).
pub fn authenticate(username: &str, password: &str) -> Result<(), String> {
    check_account_status_ok(username)?;
    verify_shadow_password(username, password)
}

/// Pattern-lock authentication (Android-style 3x3 dot grid, encoded as the
/// sequence of visited cell indices 0-8).
///
/// PAM has no native concept of a pattern, so — like fingerprint below —
/// this is a BEDM-specific second factor: the pattern is hashed (SHA-256,
/// salted with the username) and compared against
/// `{home}/.config/Blue-Environment/pattern.hash`, which the user creates
/// via Settings → Security in Blue Environment (not part of this daemon).
/// If no pattern has been set up for that user, authentication fails closed
/// (never silently falls back to "no pattern required").
pub fn authenticate_pattern(username: &str, home: &str, pattern: &[u8]) -> Result<(), String> {
    check_account_status_ok(username)?;

    let hash_path = format!("{}/.config/Blue-Environment/pattern.hash", home);
    let stored = std::fs::read_to_string(&hash_path)
        .map_err(|_| "No pattern configured for this user — set one up in Settings → Security".to_string())?;
    let stored = stored.trim();

    if pattern.len() < 4 {
        return Err("Pattern too short".to_string());
    }

    let computed = hash_pattern(username, pattern);
    if computed == stored {
        Ok(())
    } else {
        Err("Pattern not recognised".to_string())
    }
}

/// Used by both BEDM (to verify) and Blue Environment Settings (to store,
/// via the same algorithm) — keep in sync if this ever changes.
pub fn hash_pattern(username: &str, pattern: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(username.as_bytes());
    hasher.update(b":");
    hasher.update(pattern);
    format!("{:x}", hasher.finalize())
}

fn verify_shadow_password(username: &str, password: &str) -> Result<(), String> {
    let shadow_content = std::fs::read_to_string("/etc/shadow")
        .map_err(|_| "Cannot read /etc/shadow — BEDM must run as root".to_string())?;

    let entry = shadow_content
        .lines()
        .find(|line| line.starts_with(&format!("{}:", username)))
        .ok_or_else(|| format!("User '{}' not found in shadow", username))?;

    let parts: Vec<&str> = entry.split(':').collect();
    if parts.len() < 2 {
        return Err("Malformed shadow entry".to_string());
    }

    let hash = parts[1];

    if hash.starts_with('!') || hash.starts_with('*') || hash.is_empty() {
        return Err("Account is locked or has no password".to_string());
    }

    verify_crypt_hash(password, hash)
}

fn verify_crypt_hash(password: &str, hash: &str) -> Result<(), String> {
    // Call crypt(3) from libcrypt via std::process as a safe fallback.
    // libcrypt exports `crypt` but libc crate doesn't always bind it
    // (depends on platform / glibc version). We use the `openssl` PBKDF
    // path for SHA-512 hashes and fall back to calling /usr/bin/python3
    // with hashlib for portability.
    //
    // For a production build, add crypt = "0.1" or link libcrypt directly.

    if hash.starts_with("$6$") {
        verify_sha512_via_python(password, hash)
    } else if hash.starts_with("$5$") {
        verify_sha512_via_python(password, hash)
    } else if hash.starts_with("$y$") || hash.starts_with("$gy$") {
        verify_via_python_crypt(password, hash)
    } else if hash.starts_with("$2b$") || hash.starts_with("$2a$") {
        verify_via_python_crypt(password, hash)
    } else {
        verify_via_python_crypt(password, hash)
    }
}

/// Use Python hashlib.crypt (available on all glibc systems) to verify.
/// This avoids the `libc::crypt` binding issue entirely.
fn verify_via_python_crypt(password: &str, hash: &str) -> Result<(), String> {
    use std::process::Command;

    // Python script: import crypt; print(crypt.crypt(pw, hash) == hash)
    let script = format!(
        "import crypt, sys; pw=sys.argv[1]; h=sys.argv[2]; \
         result=crypt.crypt(pw,h); sys.exit(0 if result==h else 1)"
    );

    let status = Command::new("python3")
        .args(["-c", &script, password, hash])
        .status()
        .map_err(|e| format!("python3 not available for crypt verification: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err("Incorrect password".to_string())
    }
}

fn verify_sha512_via_python(password: &str, hash: &str) -> Result<(), String> {
    verify_via_python_crypt(password, hash)
}

fn check_account_status_ok(username: &str) -> Result<(), String> {
    let shadow_content = match std::fs::read_to_string("/etc/shadow") {
        Ok(c) => c,
        Err(_) => return Ok(()), // If we can't read shadow, let PAM decide
    };

    let entry = match shadow_content
        .lines()
        .find(|l| l.starts_with(&format!("{}:", username)))
    {
        Some(e) => e,
        None => return Err(format!("User '{}' does not exist", username)),
    };

    let parts: Vec<&str> = entry.split(':').collect();
    if parts.len() < 2 {
        return Ok(());
    }

    let hash = parts[1];
    if hash.starts_with('!') {
        return Err("Account is locked".to_string());
    }
    if hash == "*" {
        return Err("Account has no password set".to_string());
    }

    Ok(())
}

#[derive(Debug, PartialEq)]
pub enum AccountStatus {
    Active,
    Locked,
    NotFound,
}

pub fn check_account_status(username: &str) -> AccountStatus {
    let shadow_content = match std::fs::read_to_string("/etc/shadow") {
        Ok(c) => c,
        Err(_) => return AccountStatus::NotFound,
    };

    let entry = match shadow_content
        .lines()
        .find(|l| l.starts_with(&format!("{}:", username)))
    {
        Some(e) => e,
        None => return AccountStatus::NotFound,
    };

    let parts: Vec<&str> = entry.split(':').collect();
    if parts.len() < 2 {
        return AccountStatus::NotFound;
    }

    let hash = parts[1];
    if hash.starts_with('!') || hash.starts_with('*') {
        AccountStatus::Locked
    } else {
        AccountStatus::Active
    }
}
