#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use super::error::{StoreError, StoreResult};
use super::manifest::PackageKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Low,
    Medium,
    High,
}

pub struct PermissionDef {
    pub id: &'static str,
    pub risk: Risk,
    pub title: &'static str,
    pub description: &'static str,
}

/// Single source of truth; `manifest::KNOWN_PERMISSIONS` is derived from it.
pub const CATALOG: &[PermissionDef] = &[
    PermissionDef { id: "notifications", risk: Risk::Low, title: "Show notifications", description: "Can post notifications on your desktop." },
    PermissionDef { id: "storage", risk: Risk::Low, title: "Store its own data", description: "Can keep its own settings and data. It cannot see other applications' data." },
    PermissionDef { id: "clipboard", risk: Risk::Medium, title: "Use the clipboard", description: "Can read what you copy and put text on the clipboard — which may include passwords you copied." },
    PermissionDef { id: "network", risk: Risk::Medium, title: "Use the network", description: "Can connect to the internet and to devices on your network." },
    PermissionDef { id: "files", risk: Risk::High, title: "Access your files", description: "Can read and write files and folders on this computer." },
    PermissionDef { id: "system", risk: Risk::High, title: "Control the system", description: "Can read system information and change system settings." },
    PermissionDef { id: "camera", risk: Risk::High, title: "Use the camera", description: "Can see through your camera." },
    PermissionDef { id: "microphone", risk: Risk::High, title: "Use the microphone", description: "Can listen through your microphone." },
    PermissionDef { id: "screen_capture", risk: Risk::High, title: "Capture the screen", description: "Can record or screenshot everything on your screen, including other windows." },
];

pub fn known_ids() -> Vec<&'static str> {
    CATALOG.iter().map(|d| d.id).collect()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionInfo {
    pub id: String,
    pub risk: Risk,
    pub title: String,
    pub description: String,
}

/// Catalogue entries for `ids`, most dangerous first (unknown ids are dropped —
/// the manifest parser already rejects them).
pub fn describe(ids: &[String]) -> Vec<PermissionInfo> {
    let mut out: Vec<PermissionInfo> = ids
        .iter()
        .filter_map(|id| CATALOG.iter().find(|d| d.id == id))
        .map(|d| PermissionInfo { id: d.id.to_string(), risk: d.risk, title: d.title.to_string(), description: d.description.to_string() })
        .collect();
    out.sort_by(|a, b| b.risk.cmp(&a.risk).then_with(|| a.id.cmp(&b.id)));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

pub fn overall_risk(ids: &[String]) -> Risk {
    describe(ids).iter().map(|p| p.risk).max().unwrap_or(Risk::Low)
}

/// Permissions in `new_declared` that `old_declared` did not have.
pub fn added_permissions(old_declared: &[String], new_declared: &[String]) -> Vec<String> {
    new_declared.iter().filter(|p| !old_declared.contains(p)).cloned().collect()
}

/// A grant must be a subset of what the package declared — a UI bug or a
/// forged call must not be able to hand out permissions the manifest never asked for.
pub fn validate_grant(declared: &[String], granted: &[String]) -> StoreResult<Vec<String>> {
    if let Some(extra) = granted.iter().find(|g| !declared.contains(g)) {
        return Err(StoreError::new("consent_invalid", format!("Cannot grant \"{extra}\": the package did not request it")));
    }
    let mut g: Vec<String> = granted.to_vec();
    g.sort();
    g.dedup();
    Ok(g)
}

// ── persistence ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsentRecord {
    pub version: String,
    /// What the package declared when the grant was made.
    pub declared: Vec<String>,
    pub granted: Vec<String>,
    pub granted_at: String,
    #[serde(default)]
    pub signer: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConsentFile {
    #[serde(default)]
    consents: BTreeMap<String, ConsentRecord>,
}

pub fn config_dir() -> PathBuf {
    if let Ok(d) = std::env::var("BLUE_STORE_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Blue-Environment/store")
}

fn consent_path() -> PathBuf {
    config_dir().join("consents.json")
}

fn key(kind: PackageKind, id: &str) -> String {
    format!("{}:{id}", kind.as_str())
}

fn load() -> ConsentFile {
    std::fs::read_to_string(consent_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(f: &ConsentFile) -> StoreResult<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| StoreError::io("creating the store config folder", e))?;
    let json = serde_json::to_string_pretty(f).map_err(|e| StoreError::new("internal", e.to_string()))?;
    let tmp = dir.join("consents.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| StoreError::io("saving permissions", e))?;
    std::fs::rename(&tmp, consent_path()).map_err(|e| StoreError::io("saving permissions", e))
}

pub fn record_for(kind: PackageKind, id: &str) -> Option<ConsentRecord> {
    load().consents.remove(&key(kind, id))
}

pub fn store_consent(kind: PackageKind, id: &str, rec: ConsentRecord) -> StoreResult<()> {
    let mut f = load();
    f.consents.insert(key(kind, id), rec);
    save(&f)
}

pub fn forget(kind: PackageKind, id: &str) {
    let mut f = load();
    if f.consents.remove(&key(kind, id)).is_some() {
        let _ = save(&f);
    }
}

/// What the runtime should enforce for an installed package.
pub fn effective(kind: PackageKind, id: &str, declared: &[String]) -> Vec<String> {
    match record_for(kind, id) {
        Some(rec) => declared.iter().filter(|p| rec.granted.contains(p)).cloned().collect(),
        None => declared.to_vec(), // installed before consent existed: unchanged
    }
}

/// Decides which permissions to record for an install.
///
/// * `granted = Some(list)` — the person decided in the dialog (validated against `declared`).
/// * `granted = None` — nothing to decide when `declared` is empty, or when an earlier
///   consent already covers everything declared (a plain update); otherwise
///   `consent_required`.
pub fn resolve_grant(kind: PackageKind, id: &str, declared: &[String], granted: Option<Vec<String>>) -> StoreResult<Vec<String>> {
    if let Some(g) = granted {
        return validate_grant(declared, &g);
    }
    if declared.is_empty() {
        return Ok(Vec::new());
    }
    match record_for(kind, id) {
        Some(rec) if added_permissions(&rec.declared, declared).is_empty() => {
            // keep the earlier decision, restricted to what is still declared
            Ok(rec.granted.into_iter().filter(|p| declared.contains(p)).collect())
        }
        Some(rec) => {
            let added = added_permissions(&rec.declared, declared);
            Err(StoreError::new("consent_required", format!("This update asks for new permissions: {}", added.join(", ")))
                .hint("Review and approve the new permissions to update."))
        }
        None => Err(StoreError::new("consent_required", format!("This package asks for permissions: {}", declared.join(", ")))
            .hint("Review and approve its permissions before installing. From the command line add --grant-all, or --grant <list>, or --no-permissions.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    static GUARD: Mutex<()> = Mutex::new(());

    fn env() -> (std::sync::MutexGuard<'static, ()>, PathBuf) {
        let g = GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let d = std::env::temp_dir().join(format!("blue-store-consent-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::env::set_var("BLUE_STORE_CONFIG_DIR", &d);
        (g, d)
    }
    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn catalogue_matches_what_the_manifest_parser_accepts() {
        let ids = known_ids();
        for must in ["notifications", "clipboard", "storage", "network", "files", "system", "camera", "microphone", "screen_capture"] {
            assert!(ids.contains(&must), "{must}");
        }
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "no duplicate ids");
    }

    #[test]
    fn describe_orders_the_riskiest_first_and_summarises_risk() {
        let d = describe(&v(&["storage", "camera", "network", "camera", "bogus"]));
        assert_eq!(d.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["camera", "network", "storage"]);
        assert_eq!(overall_risk(&v(&["storage", "notifications"])), Risk::Low);
        assert_eq!(overall_risk(&v(&["storage", "network"])), Risk::Medium);
        assert_eq!(overall_risk(&v(&["screen_capture"])), Risk::High);
        assert_eq!(overall_risk(&[]), Risk::Low);
    }

    #[test]
    fn a_grant_can_never_exceed_the_declaration() {
        assert_eq!(validate_grant(&v(&["network", "files"]), &v(&["files", "files"])).unwrap(), v(&["files"]));
        assert_eq!(validate_grant(&v(&["network"]), &v(&["files"])).unwrap_err().code, "consent_invalid");
        assert!(validate_grant(&v(&["network"]), &[]).unwrap().is_empty(), "granting nothing is allowed");
    }

    #[test]
    fn runtime_sees_only_what_the_person_granted() {
        let (_g, d) = env();
        store_consent(PackageKind::App, "notes", ConsentRecord { version: "1.0".into(), declared: v(&["network", "files"]), granted: v(&["network"]), granted_at: "t".into(), signer: None }).unwrap();
        assert_eq!(effective(PackageKind::App, "notes", &v(&["network", "files"])), v(&["network"]));
        assert_eq!(effective(PackageKind::App, "legacy", &v(&["files"])), v(&["files"]), "pre-consent installs are unchanged");
        forget(PackageKind::App, "notes");
        assert_eq!(effective(PackageKind::App, "notes", &v(&["network"])), v(&["network"]));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn installing_needs_a_decision_unless_nothing_is_declared() {
        let (_g, d) = env();
        assert!(resolve_grant(PackageKind::App, "a", &[], None).unwrap().is_empty());
        assert_eq!(resolve_grant(PackageKind::App, "a", &v(&["network"]), None).unwrap_err().code, "consent_required");
        assert_eq!(resolve_grant(PackageKind::App, "a", &v(&["network"]), Some(v(&["network"]))).unwrap(), v(&["network"]));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn updates_keep_the_old_grant_but_new_permissions_need_a_new_decision() {
        let (_g, d) = env();
        store_consent(PackageKind::Plugin, "p", ConsentRecord { version: "1".into(), declared: v(&["network", "clipboard"]), granted: v(&["network"]), granted_at: "t".into(), signer: None }).unwrap();
        // same declaration → silently keeps "network only"
        assert_eq!(resolve_grant(PackageKind::Plugin, "p", &v(&["network", "clipboard"]), None).unwrap(), v(&["network"]));
        // a permission dropped in the update → still fine, grant shrinks with it
        assert_eq!(resolve_grant(PackageKind::Plugin, "p", &v(&["network"]), None).unwrap(), v(&["network"]));
        // a new permission → the person must decide again
        let e = resolve_grant(PackageKind::Plugin, "p", &v(&["network", "camera"]), None).unwrap_err();
        assert_eq!(e.code, "consent_required");
        assert!(e.message.contains("camera"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn corrupt_consent_file_degrades_to_no_records() {
        let (_g, d) = env();
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("consents.json"), "{{ not json").unwrap();
        assert!(record_for(PackageKind::App, "x").is_none());
        let _ = std::fs::remove_dir_all(d);
    }
}
