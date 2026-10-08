use futures_util::StreamExt;
use serde::Serialize;
use std::time::Duration;
use zbus::zvariant::{DynamicType, OwnedObjectPath};
use zbus::{Connection, Proxy};

const DEST: &str = "org.freedesktop.PackageKit";
const ROOT_PATH: &str = "/org/freedesktop/PackageKit";
const ROOT_IFACE: &str = "org.freedesktop.PackageKit";
const TX_IFACE: &str = "org.freedesktop.PackageKit.Transaction";

/// Bitfield filtrów PackageKit: wartość enuma `PK_FILTER_ENUM_*` przesunięta o 1.
const FILTER_NONE: u64 = 1 << 1;
/// `PK_TRANSACTION_FLAG_ENUM_ONLY_TRUSTED` (1 << 1).
const FLAG_ONLY_TRUSTED: u64 = 1 << 1;

// Wartości `PkInfoEnum`.
const INFO_INSTALLED: u32 = 1;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PkPackage {
    /// Pełne `package_id` (`nazwa;wersja;arch;źródło`) — tego używa PackageKit.
    pub id: String,
    pub name: String,
    pub version: String,
    pub arch: String,
    pub source: String,
    pub summary: String,
    pub installed: bool,
}

/// Rozbija `package_id` PackageKit na (nazwa, wersja, arch, źródło).
pub fn parse_package_id(id: &str) -> Option<(String, String, String, String)> {
    let mut it = id.splitn(4, ';');
    let (n, v, a, d) = (it.next()?, it.next()?, it.next()?, it.next().unwrap_or(""));
    (!n.is_empty()).then(|| (n.into(), v.into(), a.into(), d.into()))
}

fn package_from_signal(info: u32, id: &str, summary: &str) -> Option<PkPackage> {
    let (name, version, arch, source) = parse_package_id(id)?;
    Some(PkPackage {
        id: id.to_string(),
        name,
        version,
        arch,
        installed: info == INFO_INSTALLED || source.starts_with("installed"),
        source,
        summary: summary.to_string(),
    })
}

struct TxResult {
    packages: Vec<PkPackage>,
}

async fn run_tx<A>(method: &str, args: &A) -> Result<TxResult, String>
where
    A: serde::Serialize + DynamicType,
{
    let conn = Connection::system().await.map_err(|e| format!("brak połączenia z system bus: {e}"))?;
    let root = Proxy::new(&conn, DEST, ROOT_PATH, ROOT_IFACE)
        .await
        .map_err(|e| format!("PackageKit niedostępny: {e}"))?;
    let path: OwnedObjectPath = root
        .call("CreateTransaction", &())
        .await
        .map_err(|e| format!("PackageKit niedostępny: {e}"))?;
    let tx = Proxy::new(&conn, DEST, path, TX_IFACE).await.map_err(|e| e.to_string())?;

    let mut pkgs = tx.receive_signal("Package").await.map_err(|e| e.to_string())?;
    let mut errs = tx.receive_signal("ErrorCode").await.map_err(|e| e.to_string())?;
    let mut fin = tx.receive_signal("Finished").await.map_err(|e| e.to_string())?;

    tx.call_method(method, args).await.map_err(|e| format!("{method}: {e}"))?;

    let mut result = TxResult { packages: vec![] };
    let mut error: Option<String> = None;
    let work = async {
        loop {
            tokio::select! {
                Some(m) = pkgs.next() => {
                    if let Ok((info, id, summary)) = m.body().deserialize::<(u32, String, String)>() {
                        if let Some(p) = package_from_signal(info, &id, &summary) { result.packages.push(p); }
                    }
                }
                Some(m) = errs.next() => {
                    if let Ok((_code, details)) = m.body().deserialize::<(u32, String)>() { error = Some(details); }
                }
                Some(_) = fin.next() => break,
                else => break,
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(20 * 60), work)
        .await
        .map_err(|_| "przekroczono czas operacji PackageKit".to_string())?;
    match error {
        Some(e) => Err(e),
        None => Ok(result),
    }
}

fn valid_term(s: &str) -> bool {
    !s.trim().is_empty() && s.len() < 200 && !s.contains(|c: char| c.is_control())
}

// ───────────────────────── komendy ─────────────────────────

#[tauri::command]
pub async fn pk_available() -> bool {
    run_tx("Resolve", &(FILTER_NONE, vec!["bash"])).await.is_ok()
}

#[tauri::command]
pub async fn pk_search(query: String) -> Result<Vec<PkPackage>, String> {
    if !valid_term(&query) {
        return Err("puste zapytanie".into());
    }
    Ok(run_tx("SearchNames", &(FILTER_NONE, vec![query.trim().to_string()])).await?.packages)
}

#[tauri::command]
pub async fn pk_get_updates() -> Result<Vec<PkPackage>, String> {
    Ok(run_tx("GetUpdates", &(FILTER_NONE,)).await?.packages)
}

#[tauri::command]
pub async fn pk_refresh_cache() -> Result<(), String> {
    run_tx("RefreshCache", &(false,)).await.map(|_| ())
}

fn check_ids(ids: &[String]) -> Result<(), String> {
    if ids.is_empty() || ids.iter().any(|i| parse_package_id(i).is_none() || !valid_term(i)) {
        return Err("nieprawidłowy identyfikator pakietu".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn pk_install(package_ids: Vec<String>) -> Result<(), String> {
    check_ids(&package_ids)?;
    run_tx("InstallPackages", &(FLAG_ONLY_TRUSTED, package_ids)).await.map(|_| ())
}

#[tauri::command]
pub async fn pk_remove(package_ids: Vec<String>, autoremove: bool) -> Result<(), String> {
    check_ids(&package_ids)?;
    // allow_deps = true: PackageKit odmówi, jeśli usunięcie zepsułoby system.
    run_tx("RemovePackages", &(FLAG_ONLY_TRUSTED, package_ids, true, autoremove)).await.map(|_| ())
}

#[tauri::command]
pub async fn pk_update(package_ids: Vec<String>) -> Result<(), String> {
    check_ids(&package_ids)?;
    run_tx("UpdatePackages", &(FLAG_ONLY_TRUSTED, package_ids)).await.map(|_| ())
}


/// Wybiera z wyników `Resolve` pakiet o dokładnej nazwie w żądanym stanie.
pub fn pick_package(pkgs: &[PkPackage], name: &str, installed: bool) -> Option<PkPackage> {
    pkgs.iter().find(|p| p.name == name && p.installed == installed).cloned()
}

async fn resolve_name(name: &str) -> Result<Vec<PkPackage>, String> {
    if !valid_term(name) {
        return Err("nieprawidłowa nazwa pakietu".into());
    }
    Ok(run_tx("Resolve", &(FILTER_NONE, vec![name.trim().to_string()])).await?.packages)
}

/// Instaluje pakiet po NAZWIE (UI Blue Software operuje na nazwach, a
/// PackageKit na pełnych `package_id` — tu je rozwiązujemy).
#[tauri::command]
pub async fn pk_install_name(name: String) -> Result<(), String> {
    let found = resolve_name(&name).await?;
    if pick_package(&found, &name, true).is_some() {
        return Ok(()); // już zainstalowany
    }
    let p = pick_package(&found, &name, false).ok_or_else(|| format!("Nie znaleziono pakietu „{name}” w repozytoriach"))?;
    run_tx("InstallPackages", &(FLAG_ONLY_TRUSTED, vec![p.id])).await.map(|_| ())
}

#[tauri::command]
pub async fn pk_remove_name(name: String) -> Result<(), String> {
    let found = resolve_name(&name).await?;
    let p = pick_package(&found, &name, true).ok_or_else(|| format!("Pakiet „{name}” nie jest zainstalowany"))?;
    run_tx("RemovePackages", &(FLAG_ONLY_TRUSTED, vec![p.id], true, false)).await.map(|_| ())
}

#[tauri::command]
pub async fn pk_update_name(name: String) -> Result<(), String> {
    if !valid_term(&name) {
        return Err("nieprawidłowa nazwa pakietu".into());
    }
    let updates = run_tx("GetUpdates", &(FILTER_NONE,)).await?.packages;
    let p = updates.into_iter().find(|p| p.name == name).ok_or_else(|| format!("Brak aktualizacji dla „{name}”"))?;
    run_tx("UpdatePackages", &(FLAG_ONLY_TRUSTED, vec![p.id])).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_id() {
        let (n, v, a, d) = parse_package_id("firefox;128.0-1;amd64;ubuntu-main").unwrap();
        assert_eq!((n.as_str(), v.as_str(), a.as_str(), d.as_str()), ("firefox", "128.0-1", "amd64", "ubuntu-main"));
        assert!(parse_package_id(";1;x;y").is_none());
        assert!(parse_package_id("bezsrednikow").is_none());
    }

    #[test]
    fn installed_detected_from_info_or_source() {
        assert!(package_from_signal(1, "a;1;x;installed:main", "s").unwrap().installed);
        assert!(package_from_signal(2, "a;1;x;installed", "s").unwrap().installed);
        assert!(!package_from_signal(2, "a;1;x;main", "s").unwrap().installed);
    }

    #[test]
    fn picks_exact_name_in_requested_state() {
        let mk = |n: &str, inst: bool| package_from_signal(if inst { 1 } else { 2 }, &format!("{n};1;x;repo"), "s").unwrap();
        let v = vec![mk("vim-common", false), mk("vim", true), mk("vim", false)];
        assert_eq!(pick_package(&v, "vim", true).unwrap().installed, true);
        assert_eq!(pick_package(&v, "vim", false).unwrap().name, "vim");
        assert!(pick_package(&v, "emacs", false).is_none());
    }

    #[test]
    fn validates_ids_and_terms() {
        assert!(check_ids(&[]).is_err());
        assert!(check_ids(&["x".into()]).is_err());
        assert!(check_ids(&["x;1;y;z".into()]).is_ok());
        assert!(!valid_term("  "));
        assert!(!valid_term("a\nb"));
    }
}
