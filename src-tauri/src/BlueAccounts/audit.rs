#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use super::vault::VaultEntry;
use serde::Serialize;
use std::collections::BTreeMap;

/// Tiny deny-list of the passwords that show up at the top of every breach
/// corpus. Deliberately short: this is a sanity check, not a breach database.
const COMMON: &[&str] = &[
    "password", "passw0rd", "p@ssw0rd", "123456", "12345678", "123456789", "1234567890", "qwerty", "qwertyuiop",
    "abc123", "111111", "000000", "letmein", "welcome", "admin", "iloveyou", "monkey", "dragon", "football",
    "master", "login", "hackeros", "blue", "changeme", "secret", "hasło", "haslo",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Strength {
    VeryWeak = 0,
    Weak = 1,
    Fair = 2,
    Good = 3,
    Strong = 4,
}

/// Estimated entropy in bits (rough, intentionally conservative).
pub fn entropy_bits(pw: &str) -> f64 {
    if pw.is_empty() {
        return 0.0;
    }
    let lower = pw.to_lowercase();
    if COMMON.contains(&lower.as_str()) {
        return 4.0;
    }
    let (mut lo, mut up, mut di, mut sy, mut other) = (false, false, false, false, false);
    for c in pw.chars() {
        if c.is_ascii_lowercase() {
            lo = true
        } else if c.is_ascii_uppercase() {
            up = true
        } else if c.is_ascii_digit() {
            di = true
        } else if c.is_ascii_punctuation() || c == ' ' {
            sy = true
        } else {
            other = true
        }
    }
    let pool = (lo as u32 * 26 + up as u32 * 26 + di as u32 * 10 + sy as u32 * 33 + other as u32 * 64).max(1) as f64;
    let chars: Vec<char> = pw.chars().collect();
    // Repeating the same few characters adds almost nothing.
    let unique = chars.iter().collect::<std::collections::BTreeSet<_>>().len() as f64;
    let effective_len = (unique + (chars.len() as f64 - unique) * 0.25).max(1.0);
    let mut bits = effective_len * pool.log2();
    // Straight runs (abcd, 1234) are guessable.
    let runs = chars.windows(3).filter(|w| (w[1] as i64 - w[0] as i64 == 1) && (w[2] as i64 - w[1] as i64 == 1)).count();
    bits -= runs as f64 * 3.0;
    bits.max(1.0)
}

pub fn strength(pw: &str) -> Strength {
    match entropy_bits(pw) {
        b if b < 28.0 => Strength::VeryWeak,
        b if b < 40.0 => Strength::Weak,
        b if b < 60.0 => Strength::Fair,
        b if b < 80.0 => Strength::Good,
        _ => Strength::Strong,
    }
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditReport {
    pub total: usize,
    pub audited: usize,
    /// Ids whose password is `VeryWeak` or `Weak`.
    pub weak: Vec<String>,
    /// Groups of ids sharing an identical password.
    pub reused: Vec<Vec<String>>,
    pub empty: Vec<String>,
    /// 0–100 overall score (100 = nothing to fix).
    pub score: u8,
}

pub fn audit(entries: &[VaultEntry]) -> AuditReport {
    let managed: Vec<&VaultEntry> = entries.iter().filter(|e| !e.is_secret_service()).collect();
    let mut report = AuditReport { total: entries.len(), audited: managed.len(), score: 100, ..Default::default() };
    let mut by_pw: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for e in &managed {
        if e.password.is_empty() {
            report.empty.push(e.id.clone());
            continue;
        }
        if strength(&e.password) <= Strength::Weak {
            report.weak.push(e.id.clone());
        }
        by_pw.entry(e.password.as_str()).or_default().push(e.id.clone());
    }
    report.reused = by_pw.into_values().filter(|g| g.len() > 1).collect();
    if !managed.is_empty() {
        let reused_entries: usize = report.reused.iter().map(|g| g.len()).sum();
        let bad = report.weak.len() + report.empty.len() + reused_entries;
        let ratio = (bad.min(managed.len() * 2) as f64) / (managed.len() as f64 * 2.0);
        report.score = (100.0 - ratio * 100.0).round().clamp(0.0, 100.0) as u8;
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: &str, pw: &str) -> VaultEntry {
        VaultEntry { id: id.into(), password: pw.into(), ..Default::default() }
    }

    #[test]
    fn common_and_short_passwords_are_very_weak() {
        assert_eq!(strength("password"), Strength::VeryWeak);
        assert_eq!(strength("Password"), Strength::VeryWeak, "case must not dodge the deny-list");
        assert_eq!(strength("abc"), Strength::VeryWeak);
        assert_eq!(strength(""), Strength::VeryWeak);
    }

    #[test]
    fn long_random_passwords_are_strong() {
        assert_eq!(strength("k8#Vq!2mZp@9xL4wTr7&Bn"), Strength::Strong);
        assert!(strength("correct horse battery staple") >= Strength::Good);
    }

    #[test]
    fn repetition_and_runs_are_penalised() {
        assert!(entropy_bits("aaaaaaaaaaaa") < entropy_bits("a8Kd2xP9zQ4w"));
        assert!(entropy_bits("abcdefgh") < entropy_bits("hbdfaecg"));
    }

    #[test]
    fn audit_flags_weak_reused_and_empty() {
        let entries = vec![e("1", "password"), e("2", "k8#Vq!2mZp@9xL4wTr7&Bn"), e("3", "k8#Vq!2mZp@9xL4wTr7&Bn"), e("4", "")];
        let r = audit(&entries);
        assert_eq!(r.weak, vec!["1".to_string()]);
        assert_eq!(r.empty, vec!["4".to_string()]);
        assert_eq!(r.reused, vec![vec!["2".to_string(), "3".to_string()]]);
        assert!(r.score < 100);
    }

    #[test]
    fn clean_vault_scores_100_and_empty_vault_does_not_divide_by_zero() {
        assert_eq!(audit(&[e("1", "k8#Vq!2mZp@9xL4wTr7&Bn")]).score, 100);
        assert_eq!(audit(&[]).score, 100);
    }

    #[test]
    fn secret_service_tokens_are_not_audited() {
        let mut t = e("t", "x");
        t.source = super::super::vault::SOURCE_SECRET_SERVICE.into();
        let r = audit(&[t]);
        assert_eq!(r.audited, 0);
        assert!(r.weak.is_empty());
    }
}
