#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use super::error::{StoreError, StoreResult};
use super::manifest::{Manifest, PackageKind};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DOMAIN: &str = "blue-package-v1";
const SYSTEM_KEYS: &str = "/usr/share/blue-environment/trusted-publishers.json";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrustedKey {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub public_key: String,
}

/// What the install dialog shows about authenticity.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum SignatureStatus {
    /// No signature in blue.hk.
    Unsigned,
    /// Valid signature by a key the person / system trusts.
    #[serde(rename_all = "camelCase")]
    Trusted { signer: String, name: String },
    /// Valid-looking, but the signer is not in any trusted list.
    #[serde(rename_all = "camelCase")]
    UnknownSigner { signer: String },
    /// Signature present but wrong / malformed / unverifiable.
    #[serde(rename_all = "camelCase")]
    Invalid { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Policy {
    Warn,
    Require,
}

/// The exact bytes that get signed.
pub fn signed_message(kind: PackageKind, id: &str, version: &str, sha256_hex: &str) -> Vec<u8> {
    format!("{DOMAIN}\n{}\n{id}\n{version}\n{}", kind.as_str(), sha256_hex.to_ascii_lowercase()).into_bytes()
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// Pure verification against an explicit key list.
pub fn verify(m: &Manifest, sha256_hex: &str, keys: &[TrustedKey]) -> SignatureStatus {
    let (Some(sig_b64), signer) = (m.signature.as_deref(), m.signer.as_deref()) else {
        return SignatureStatus::Unsigned;
    };
    let Some(signer) = signer else {
        return SignatureStatus::Invalid { reason: "blue.hk has a `signature` but no `signer`".into() };
    };
    if sha256_hex.len() != 64 {
        return SignatureStatus::Invalid { reason: "a signed package must publish its `sha256` in blue.hk".into() };
    }
    let Ok(sig_bytes) = b64().decode(sig_b64.trim()) else {
        return SignatureStatus::Invalid { reason: "the signature is not valid base64".into() };
    };
    let Ok(sig) = Signature::from_slice(&sig_bytes) else {
        return SignatureStatus::Invalid { reason: "the signature has the wrong length".into() };
    };
    let Some(key) = keys.iter().find(|k| k.id == signer) else {
        // Still check it is well-formed so the UI can tell "unknown" from "garbage".
        return SignatureStatus::UnknownSigner { signer: signer.to_string() };
    };
    let Some(vk) = b64().decode(key.public_key.trim()).ok().and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok()).and_then(|b| VerifyingKey::from_bytes(&b).ok()) else {
        return SignatureStatus::Invalid { reason: format!("the trusted key \"{signer}\" is malformed") };
    };
    let msg = signed_message(m.kind, &m.id, &m.version, sha256_hex);
    match vk.verify(&msg, &sig) {
        Ok(()) => SignatureStatus::Trusted { signer: signer.to_string(), name: if key.name.is_empty() { signer.to_string() } else { key.name.clone() } },
        Err(_) => SignatureStatus::Invalid { reason: "the signature does not match this package".into() },
    }
}

/// `Require` lets only trusted signatures through; `Warn` still blocks forged ones.
pub fn enforce(status: &SignatureStatus, policy: Policy) -> StoreResult<()> {
    match (status, policy) {
        (SignatureStatus::Trusted { .. }, _) => Ok(()),
        (SignatureStatus::Invalid { reason }, _) => Err(StoreError::new("signature_invalid", format!("The package signature is invalid: {reason}"))
            .hint("Nothing was installed. The package may have been tampered with.")),
        (SignatureStatus::Unsigned, Policy::Require) => Err(StoreError::new("signature_required", "This package is not signed")
            .hint("Your policy only allows packages signed by a trusted publisher (store/policy.json).")),
        (SignatureStatus::UnknownSigner { signer }, Policy::Require) => Err(StoreError::new("signature_untrusted", format!("The publisher \"{signer}\" is not trusted"))
            .hint("Add their public key to ~/.config/Blue-Environment/store/trusted-publishers.json if you trust them.")),
        (_, Policy::Warn) => Ok(()),
    }
}

// ── configuration files ────────────────────────────────────────────────────

fn read_keys(path: PathBuf) -> Vec<TrustedKey> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn load_trusted_keys() -> Vec<TrustedKey> {
    let mut keys = read_keys(PathBuf::from(SYSTEM_KEYS));
    keys.extend(read_keys(super::permissions::config_dir().join("trusted-publishers.json")));
    keys
}

pub fn load_policy() -> Policy {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct P {
        signature_policy: Policy,
    }
    std::fs::read_to_string(super::permissions::config_dir().join("policy.json")).ok().and_then(|t| serde_json::from_str::<P>(&t).ok()).map(|p| p.signature_policy).unwrap_or(Policy::Warn)
}

/// Verify with the configured keys.
pub fn verify_configured(m: &Manifest, sha256_hex: &str) -> SignatureStatus {
    verify(m, sha256_hex, &load_trusted_keys())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    const SHA: &str = "3f2a9c1d3f2a9c1d3f2a9c1d3f2a9c1d3f2a9c1d3f2a9c1d3f2a9c1d3f2a9c1d";

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }
    fn trusted(sk: &SigningKey, id: &str) -> TrustedKey {
        TrustedKey { id: id.into(), name: "ACME Ltd".into(), public_key: b64().encode(sk.verifying_key().to_bytes()) }
    }
    fn manifest(sk: Option<&SigningKey>, signer: Option<&str>) -> Manifest {
        let mut m = Manifest {
            id: "notes".into(), name: "Notes".into(), kind: PackageKind::App, version: "1.2.0".into(), author: "a".into(), description: String::new(),
            icon: None, archive: None, sha256: Some(SHA.into()), homepage: None, license: None, category: None, min_blue_version: None,
            entry: "index.js".into(), style: None, width: None, height: None, min_width: None, min_height: None, plugin_kind: None,
            permissions: vec![], signature: None, signer: signer.map(String::from),
        };
        if let Some(sk) = sk {
            m.signature = Some(b64().encode(sk.sign(&signed_message(m.kind, &m.id, &m.version, SHA)).to_bytes()));
        }
        m
    }

    #[test]
    fn a_correct_signature_by_a_trusted_key_verifies() {
        let sk = key(7);
        let m = manifest(Some(&sk), Some("acme"));
        assert_eq!(verify(&m, SHA, &[trusted(&sk, "acme")]), SignatureStatus::Trusted { signer: "acme".into(), name: "ACME Ltd".into() });
    }

    #[test]
    fn unsigned_packages_are_reported_as_such() {
        assert_eq!(verify(&manifest(None, None), SHA, &[]), SignatureStatus::Unsigned);
    }

    #[test]
    fn tampering_with_anything_signed_invalidates_it() {
        let sk = key(7);
        let keys = [trusted(&sk, "acme")];
        let good = manifest(Some(&sk), Some("acme"));
        let mut other_version = good.clone();
        other_version.version = "9.9.9".into();
        let mut other_id = good.clone();
        other_id.id = "evil".into();
        let mut other_kind = good.clone();
        other_kind.kind = PackageKind::Plugin;
        for m in [other_version, other_id, other_kind] {
            assert!(matches!(verify(&m, SHA, &keys), SignatureStatus::Invalid { .. }));
        }
        let other_hash = "0".repeat(64);
        assert!(matches!(verify(&good, &other_hash, &keys), SignatureStatus::Invalid { .. }), "different archive bytes");
    }

    #[test]
    fn a_signature_by_a_different_key_under_a_trusted_id_is_invalid() {
        let m = manifest(Some(&key(1)), Some("acme"));
        assert!(matches!(verify(&m, SHA, &[trusted(&key(2), "acme")]), SignatureStatus::Invalid { .. }));
    }

    #[test]
    fn unknown_signers_and_malformed_input_are_distinguished() {
        let sk = key(3);
        assert_eq!(verify(&manifest(Some(&sk), Some("nobody")), SHA, &[trusted(&sk, "acme")]), SignatureStatus::UnknownSigner { signer: "nobody".into() });
        let mut bad = manifest(Some(&sk), Some("acme"));
        bad.signature = Some("%%%".into());
        assert!(matches!(verify(&bad, SHA, &[]), SignatureStatus::Invalid { .. }));
        bad.signature = Some(b64().encode([1u8; 10]));
        assert!(matches!(verify(&bad, SHA, &[]), SignatureStatus::Invalid { .. }));
        let mut no_signer = manifest(Some(&sk), None);
        no_signer.signer = None;
        assert!(matches!(verify(&no_signer, SHA, &[]), SignatureStatus::Invalid { .. }));
        assert!(matches!(verify(&manifest(Some(&sk), Some("acme")), "abc", &[]), SignatureStatus::Invalid { .. }), "needs a published sha256");
    }

    #[test]
    fn policy_enforcement() {
        let trusted_s = SignatureStatus::Trusted { signer: "a".into(), name: "A".into() };
        let invalid = SignatureStatus::Invalid { reason: "x".into() };
        let unknown = SignatureStatus::UnknownSigner { signer: "z".into() };
        for p in [Policy::Warn, Policy::Require] {
            assert!(enforce(&trusted_s, p).is_ok());
            assert_eq!(enforce(&invalid, p).unwrap_err().code, "signature_invalid", "forgeries are never installed, even under `warn`");
        }
        assert!(enforce(&SignatureStatus::Unsigned, Policy::Warn).is_ok());
        assert!(enforce(&unknown, Policy::Warn).is_ok());
        assert_eq!(enforce(&SignatureStatus::Unsigned, Policy::Require).unwrap_err().code, "signature_required");
        assert_eq!(enforce(&unknown, Policy::Require).unwrap_err().code, "signature_untrusted");
    }

    /// Known-answer vector produced by `scripts/blue-package-sign.mjs` (Node's
    /// own ed25519) so the two implementations are proven interoperable.
    #[test]
    fn node_script_signatures_verify_here() {
        let vec_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/package-signature-vector.json");
        let Ok(text) = std::fs::read_to_string(&vec_path) else { return };
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct V { public_key: String, kind: String, id: String, version: String, sha256: String, signature: String }
        let v: V = serde_json::from_str(&text).expect("fixture json");
        let mut m = manifest(None, Some("fixture"));
        m.kind = PackageKind::parse(&v.kind).expect("kind");
        m.id = v.id;
        m.version = v.version;
        m.signature = Some(v.signature);
        let keys = [TrustedKey { id: "fixture".into(), name: "fixture".into(), public_key: v.public_key }];
        assert!(matches!(verify(&m, &v.sha256, &keys), SignatureStatus::Trusted { .. }));
    }
}
