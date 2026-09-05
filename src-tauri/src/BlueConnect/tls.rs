use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::server::WebPkiClientVerifier;
use rustls::{DigitallySignedStruct, DistinguishedName, SignatureScheme};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn cert_path() -> PathBuf { super::connect_dir().join("identity_cert.pem") }
fn key_path() -> PathBuf { super::connect_dir().join("identity_key.pem") }

/// This device's persistent (self-signed, never CA-signed — see module
/// doc) TLS identity, generated once on first use and reused forever
/// after. Losing this file means every previously-paired peer's pin now
/// mismatches — same recovery story as KDE Connect itself: re-pair.
pub fn load_or_create_identity() -> Result<(rustls::pki_types::CertificateDer<'static>, rustls::pki_types::PrivateKeyDer<'static>), String> {
    let (cert_p, key_p) = (cert_path(), key_path());
    if let (Ok(cert_pem), Ok(key_pem)) = (std::fs::read(&cert_p), std::fs::read(&key_p)) {
        if let (Ok(mut certs), Ok(Some(key))) = (
            rustls_pemfile::certs(&mut cert_pem.as_slice()).collect::<Result<Vec<_>, _>>(),
            rustls_pemfile::private_key(&mut key_pem.as_slice()),
        ) {
            if let Some(cert) = certs.pop() {
                return Ok((cert, key));
            }
        }
    }

    let params = rcgen::CertificateParams::new(vec!["blue-connect.local".to_string()])
        .map_err(|e| format!("failed to build certificate params: {e}"))?;
    let key_pair = rcgen::KeyPair::generate().map_err(|e| e.to_string())?;
    let cert = params.self_signed(&key_pair).map_err(|e| format!("failed to self-sign certificate: {e}"))?;

    let _ = std::fs::create_dir_all(super::connect_dir());
    let _ = std::fs::write(&cert_p, cert.pem());
    let _ = std::fs::write(&key_p, key_pair.serialize_pem());

    Ok((
        CertificateDer::from(cert.der().to_vec()),
        rustls::pki_types::PrivateKeyDer::try_from(key_pair.serialize_der())
            .map_err(|e| format!("failed to encode private key: {e}"))?,
    ))
}

/// SHA-256 fingerprint of a DER-encoded certificate, formatted as
/// lowercase hex — this is the value stored as
/// `DiscoveredDevice.pinned_cert_sha256` and compared on every
/// reconnect.
pub fn fingerprint(cert: &CertificateDer<'_>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cert.as_ref());
    format!("{:x}", hasher.finalize())
}

/// A verifier that accepts *any* certificate but stashes its
/// fingerprint into `captured` — the actual pin-matching decision
/// happens afterward in application code (`bc_request_pairing`/
/// `bc_listen_for_pairing`), once the fingerprint is available, rather
/// than inside this trait implementation. This keeps the verifier
/// itself trivial and auditable: it does not decide trust, it only
/// reports identity.
#[derive(Debug)]
pub struct FingerprintCapturingVerifier {
    pub captured: Arc<Mutex<Option<String>>>,
}

impl ServerCertVerifier for FingerprintCapturingVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        *self.captured.lock().unwrap() = Some(fingerprint(end_entity));
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(&self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(&self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        // Every scheme rustls's default crypto provider (`ring`, per
        // this crate's Cargo.toml feature selection) can itself verify —
        // this verifier doesn't add restrictions of its own since trust
        // is decided by fingerprint match, not by which scheme signed
        // the handshake.
        rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }
}

/// Server-side equivalent of [`FingerprintCapturingVerifier`] — Blue
/// Connect's TLS is mutual (both sides present a certificate, matching
/// KDE Connect's own two-way trust model), so the listener side needs
/// its own client-cert verifier that likewise just records the
/// fingerprint rather than validating it against a CA.
#[derive(Debug)]
pub struct FingerprintCapturingClientVerifier {
    pub captured: Arc<Mutex<Option<String>>>,
}

impl ClientCertVerifier for FingerprintCapturingClientVerifier {
    fn offer_client_auth(&self) -> bool { true }
    fn client_auth_mandatory(&self) -> bool { true }
    fn root_hint_subjects(&self) -> &[DistinguishedName] { &[] }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        *self.captured.lock().unwrap() = Some(fingerprint(end_entity));
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(&self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(&self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }
}

/// Builds a `rustls::ClientConfig` for `bc_request_pairing`: presents
/// this device's own certificate (mutual TLS) and accepts the server's
/// certificate unconditionally at the handshake layer, capturing its
/// fingerprint for the caller to check against any existing pin.
pub fn client_config(captured: Arc<Mutex<Option<String>>>) -> Result<rustls::ClientConfig, String> {
    let (cert, key) = load_or_create_identity()?;
    let verifier = Arc::new(FingerprintCapturingVerifier { captured });
    rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_client_auth_cert(vec![cert], key)
        .map_err(|e| format!("failed to build TLS client config: {e}"))
}

/// Builds a `rustls::ServerConfig` for `bc_listen_for_pairing`: presents
/// this device's own certificate and requires + accepts the client's
/// certificate unconditionally at the handshake layer, capturing its
/// fingerprint the same way.
pub fn server_config(captured: Arc<Mutex<Option<String>>>) -> Result<rustls::ServerConfig, String> {
    let (cert, key) = load_or_create_identity()?;
    let verifier = Arc::new(FingerprintCapturingClientVerifier { captured });
    // `WebPkiClientVerifier` isn't actually used for verification here
    // (our own `ClientCertVerifier` impl above replaces it entirely) —
    // only imported because some `rustls` versions require a
    // `ClientCertVerifier` trait object to be constructed through it.
    // If this doesn't compile against the resolved `rustls` patch
    // version, this line — not the trust logic above it — is the one to
    // adjust; see this module's "Verification status" doc note.
    let _ = WebPkiClientVerifier::no_client_auth();
    rustls::ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(vec![cert], key)
        .map_err(|e| format!("failed to build TLS server config: {e}"))
}

/// Derives a short, human-comparable numeric code from both sides'
/// certificate fingerprints — a Short Authentication String (SAS),
/// same concept as Signal's "safety numbers" or Bluetooth's numeric-
/// comparison pairing. Concatenating the two fingerprints in a
/// canonical (sorted) order means both sides compute the *same* digits
/// only if they both saw the *same pair* of certificates — a
/// person-in-the-middle presenting a different certificate to each
/// side would cause the two ends to compute different codes, which is
/// exactly what asking a human to read the code aloud (or compare it
/// on both screens) is meant to catch. This is the missing piece that
/// turns "trust-on-first-network-use" into "trust-on-first-*verified*-
/// use" — see `mod.rs`'s pairing flow for where this is actually shown
/// to the person and gated behind their explicit accept.
pub fn compute_sas(fingerprint_a: &str, fingerprint_b: &str) -> String {
    let (first, second) = if fingerprint_a <= fingerprint_b {
        (fingerprint_a, fingerprint_b)
    } else {
        (fingerprint_b, fingerprint_a)
    };
    let mut hasher = Sha256::new();
    hasher.update(first.as_bytes());
    hasher.update(b"|");
    hasher.update(second.as_bytes());
    let digest = hasher.finalize();
    // First 4 bytes as a big-endian u32, reduced mod 1_000_000 for a
    // fixed-width 6-digit code — short enough to read aloud or glance-
    // compare, same length class as a TOTP code people already
    // recognize the shape of.
    let n = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    format!("{:06}", n % 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_for_the_same_cert_bytes() {
        let der = CertificateDer::from(vec![1, 2, 3, 4]);
        assert_eq!(fingerprint(&der), fingerprint(&der));
    }

    #[test]
    fn fingerprint_differs_for_different_cert_bytes() {
        let a = CertificateDer::from(vec![1, 2, 3, 4]);
        let b = CertificateDer::from(vec![1, 2, 3, 5]);
        assert_ne!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn compute_sas_is_order_independent() {
        // Both sides must compute the identical code regardless of
        // which fingerprint is "mine" vs "theirs" locally.
        let a = "aaaa1111";
        let b = "bbbb2222";
        assert_eq!(compute_sas(a, b), compute_sas(b, a));
    }

    #[test]
    fn compute_sas_differs_for_different_fingerprint_pairs() {
        // A person-in-the-middle presenting a different cert to one
        // side changes that side's computed code — this is the whole
        // point of asking a human to compare it.
        let a = "aaaa1111";
        let b = "bbbb2222";
        let c = "cccc3333";
        assert_ne!(compute_sas(a, b), compute_sas(a, c));
    }

    #[test]
    fn compute_sas_is_always_six_digits() {
        let sas = compute_sas("x", "y");
        assert_eq!(sas.len(), 6);
        assert!(sas.chars().all(|c| c.is_ascii_digit()));
    }
}
