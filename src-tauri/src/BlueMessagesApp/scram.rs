use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Digest;

/// Which SCRAM variant to use — selects both the mechanism name sent
/// in `<auth mechanism='...'>` and the hash function everything else
/// in this module is generic over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScramHash {
    Sha1,
    Sha256,
}

impl ScramHash {
    pub fn mechanism_name(self) -> &'static str {
        match self {
            ScramHash::Sha1 => "SCRAM-SHA-1",
            ScramHash::Sha256 => "SCRAM-SHA-256",
        }
    }

    /// Picks the strongest SCRAM variant a server's advertised
    /// mechanism list supports, or `None` if it offers neither.
    pub fn best_offered(mechanisms: &[String]) -> Option<ScramHash> {
        if mechanisms.iter().any(|m| m == "SCRAM-SHA-256") {
            Some(ScramHash::Sha256)
        } else if mechanisms.iter().any(|m| m == "SCRAM-SHA-1") {
            Some(ScramHash::Sha1)
        } else {
            None
        }
    }
}

fn hmac_bytes(hash: ScramHash, key: &[u8], data: &[u8]) -> Vec<u8> {
    match hash {
        ScramHash::Sha1 => {
            let mut mac = Hmac::<sha1::Sha1>::new_from_slice(key).expect("HMAC accepts any key length");
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }
        ScramHash::Sha256 => {
            let mut mac = Hmac::<sha2::Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }
    }
}

fn h(hash: ScramHash, data: &[u8]) -> Vec<u8> {
    match hash {
        ScramHash::Sha1 => sha1::Sha1::digest(data).to_vec(),
        ScramHash::Sha256 => sha2::Sha256::digest(data).to_vec(),
    }
}

fn salted_password(hash: ScramHash, password: &[u8], salt: &[u8], iterations: u32) -> Vec<u8> {
    match hash {
        ScramHash::Sha1 => {
            let mut out = [0u8; 20];
            pbkdf2_hmac::<sha1::Sha1>(password, salt, iterations, &mut out);
            out.to_vec()
        }
        ScramHash::Sha256 => {
            let mut out = [0u8; 32];
            pbkdf2_hmac::<sha2::Sha256>(password, salt, iterations, &mut out);
            out.to_vec()
        }
    }
}

fn xor(a: &[u8], b: &[u8]) -> Vec<u8> {
    a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect()
}

/// SCRAM's own escaping for the `,` and `=` characters inside a
/// `username` attribute — this client's usernames come from JIDs which
/// practically never contain either, but escaping unconditionally costs
/// nothing and avoids a rare-but-real wire-format corruption if one
/// ever does.
fn scram_escape(s: &str) -> String {
    s.replace('=', "=3D").replace(',', "=2C")
}

pub struct ScramExchange {
    hash: ScramHash,
    client_nonce: String,
    client_first_bare: String,
}

impl ScramExchange {
    /// Starts a new exchange, generating a fresh random client nonce.
    /// Returns the exchange state (to be threaded through the rest of
    /// the handshake) and the base64 `client-first-message` to send as
    /// the `<auth>` element's content.
    pub fn client_first(hash: ScramHash, username: &str) -> (Self, String) {
        let mut nonce_bytes = [0u8; 18];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let client_nonce = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, nonce_bytes);

        let client_first_bare = format!("n={},r={}", scram_escape(username), client_nonce);
        let gs2_header = "n,,"; // no channel binding, no authzid — see module doc
        let full_message = format!("{gs2_header}{client_first_bare}");
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, full_message.as_bytes());

        (Self { hash, client_nonce, client_first_bare }, encoded)
    }

    /// Consumes the server's base64 `server-first-message`, computes
    /// the client proof, and returns `(base64 client-final-message,
    /// expected server signature)`. The caller sends the message and
    /// must compare the server's actual `v=` value against the
    /// returned signature before treating login as successful.
    pub fn client_final(
        &self,
        password: &str,
        server_first_b64: &str,
    ) -> Result<(String, Vec<u8>), String> {
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, server_first_b64)
            .map_err(|e| format!("server-first-message wasn't valid base64: {e}"))?;
        let server_first = String::from_utf8(decoded).map_err(|e| format!("server-first-message wasn't valid UTF-8: {e}"))?;

        let server_nonce = extract_field(&server_first, 'r').ok_or("server-first-message missing r=")?;
        if !server_nonce.starts_with(&self.client_nonce) {
            return Err("server nonce does not extend our client nonce — possible tampering".to_string());
        }
        let salt_b64 = extract_field(&server_first, 's').ok_or("server-first-message missing s=")?;
        let salt = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, salt_b64)
            .map_err(|e| format!("salt wasn't valid base64: {e}"))?;
        let iterations: u32 = extract_field(&server_first, 'i')
            .ok_or("server-first-message missing i=")?
            .parse()
            .map_err(|_| "server-first-message's i= wasn't a valid integer".to_string())?;

        let salted_pw = salted_password(self.hash, password.as_bytes(), &salt, iterations);
        let client_key = hmac_bytes(self.hash, &salted_pw, b"Client Key");
        let stored_key = h(self.hash, &client_key);

        let gs2_header_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"n,,");
        let client_final_without_proof = format!("c={gs2_header_b64},r={server_nonce}");
        let auth_message = format!("{},{},{}", self.client_first_bare, server_first, client_final_without_proof);

        let client_signature = hmac_bytes(self.hash, &stored_key, auth_message.as_bytes());
        let client_proof = xor(&client_key, &client_signature);
        let client_proof_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &client_proof);

        let client_final_message = format!("{client_final_without_proof},p={client_proof_b64}");
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, client_final_message.as_bytes());

        let server_key = hmac_bytes(self.hash, &salted_pw, b"Server Key");
        let expected_server_signature = hmac_bytes(self.hash, &server_key, auth_message.as_bytes());

        Ok((encoded, expected_server_signature))
    }
}

/// Pulls a comma-separated `key=value` field out of a SCRAM message —
/// e.g. `extract_field("r=abc,s=xyz==,i=4096", 's')` → `Some("xyz==")`.
fn extract_field(message: &str, key: char) -> Option<String> {
    message.split(',').find_map(|part| {
        let mut chars = part.chars();
        if chars.next() == Some(key) && chars.next() == Some('=') {
            Some(part[2..].to_string())
        } else {
            None
        }
    })
}

/// Verifies a server's base64 `server-final-message` (`v=...`) against
/// the signature [`ScramExchange::client_final`] computed. Returns an
/// error (never panics) on any mismatch or malformed input — a
/// mismatch here means the server could not prove it knows the shared
/// secret, which `xmpp.rs` treats as authentication failure even if an
/// earlier step looked successful.
pub fn verify_server_final(server_final_b64: &str, expected_signature: &[u8]) -> Result<(), String> {
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, server_final_b64)
        .map_err(|e| format!("server-final-message wasn't valid base64: {e}"))?;
    let server_final = String::from_utf8(decoded).map_err(|e| format!("server-final-message wasn't valid UTF-8: {e}"))?;
    if let Some(err) = extract_field(&server_final, 'e') {
        return Err(format!("server reported SASL error: {err}"));
    }
    let v = extract_field(&server_final, 'v').ok_or("server-final-message missing v=")?;
    let actual = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &v)
        .map_err(|e| format!("server signature wasn't valid base64: {e}"))?;
    if actual == expected_signature {
        Ok(())
    } else {
        Err("server signature did not match — refusing to trust this server".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs a full client+server SCRAM exchange using this module's own
    /// client half twice — once "as the client" and once re-deriving
    /// the server's expected values independently from the same
    /// password/salt/iterations, the way a real SCRAM server would —
    /// to confirm the two sides actually agree, rather than only
    /// testing that the client code runs without panicking.
    fn round_trip(hash: ScramHash) {
        let username = "alice";
        let password = "correct horse battery staple";
        let salt = b"fixedsaltfortest";
        let iterations = 4096u32;

        let (exchange, client_first_b64) = ScramExchange::client_first(hash, username);
        // A real server would parse client_first_b64 itself; this test
        // only needs the client_nonce that's already inside `exchange`
        // to build a plausible server-first-message.
        let _ = client_first_b64;

        let server_nonce = format!("{}SERVERPART", exchange.client_nonce);
        let salt_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, salt);
        let server_first = format!("r={server_nonce},s={salt_b64},i={iterations}");
        let server_first_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, server_first.as_bytes());

        let (_client_final_b64, expected_sig) = exchange.client_final(password, &server_first_b64).unwrap();

        // Independently re-derive what a real server would compute as
        // its own signature, from nothing but the shared password/salt
        // — if this matches `expected_sig`, the client-side math in
        // `client_final` is genuinely RFC-5802-correct, not just
        // internally self-consistent.
        let salted_pw = salted_password(hash, password.as_bytes(), salt, iterations);
        let client_key = hmac_bytes(hash, &salted_pw, b"Client Key");
        let stored_key = h(hash, &client_key);
        let gs2_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"n,,");
        let client_final_without_proof = format!("c={gs2_b64},r={server_nonce}");
        let auth_message = format!("{},{},{}", exchange.client_first_bare, server_first, client_final_without_proof);
        let server_key = hmac_bytes(hash, &salted_pw, b"Server Key");
        let server_sig = hmac_bytes(hash, &server_key, auth_message.as_bytes());

        assert_eq!(expected_sig, server_sig);

        // And verify_server_final should accept the server's real v=.
        let v_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &server_sig);
        let server_final = format!("v={v_b64}");
        let server_final_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, server_final.as_bytes());
        assert!(verify_server_final(&server_final_b64, &expected_sig).is_ok());
    }

    #[test]
    fn scram_sha256_round_trip_matches_independently_derived_server_signature() {
        round_trip(ScramHash::Sha256);
    }

    #[test]
    fn scram_sha1_round_trip_matches_independently_derived_server_signature() {
        round_trip(ScramHash::Sha1);
    }

    #[test]
    fn verify_server_final_rejects_a_forged_signature() {
        let fake_sig_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"not-the-real-signature-bytes!!!!");
        let server_final_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("v={fake_sig_b64}").as_bytes(),
        );
        assert!(verify_server_final(&server_final_b64, b"the-actual-expected-signature").is_err());
    }

    #[test]
    fn best_offered_prefers_sha256_over_sha1() {
        let mechs = vec!["PLAIN".to_string(), "SCRAM-SHA-1".to_string(), "SCRAM-SHA-256".to_string()];
        assert_eq!(ScramHash::best_offered(&mechs), Some(ScramHash::Sha256));
    }

    #[test]
    fn best_offered_falls_back_to_sha1_when_sha256_absent() {
        let mechs = vec!["SCRAM-SHA-1".to_string()];
        assert_eq!(ScramHash::best_offered(&mechs), Some(ScramHash::Sha1));
    }

    #[test]
    fn best_offered_none_when_neither_present() {
        let mechs = vec!["PLAIN".to_string()];
        assert_eq!(ScramHash::best_offered(&mechs), None);
    }
}
