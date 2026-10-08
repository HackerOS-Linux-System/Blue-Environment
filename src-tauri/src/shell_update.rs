use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const VERSION_URL: &str =
    "https://raw.githubusercontent.com/HackerOS-Linux-System/Blue-Environment/main/config/version.hacker";
const RELEASE_BASE: &str =
    "https://github.com/HackerOS-Linux-System/Blue-Environment/releases/download";
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

// ---------------------------------------------------------------- ścieżki --

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Blue-Environment/shell-update.json")
}

pub fn staged_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Blue-Environment/staged")
}

// ------------------------------------------------------------------ stan ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateState {
    /// Automatyczne pobieranie aktualizacji powłoki (wyłączane w Ustawieniach).
    pub auto_update: bool,
    /// Wersja, którą użytkownik kliknął „Ignoruj".
    pub ignored_version: Option<String>,
    /// Wersja pobrana i gotowa — zadziała po restarcie.
    pub staged_version: Option<String>,
    /// Ostatnio znaleziona, jeszcze nie zainstalowana wersja.
    pub available_version: Option<String>,
    pub last_check: Option<String>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            auto_update: true,
            ignored_version: None,
            staged_version: None,
            available_version: None,
            last_check: None,
        }
    }
}

fn load_state() -> UpdateState {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(state: &UpdateState) -> Result<(), String> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(state).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    fs::rename(tmp, path).map_err(|e| e.to_string())
}

// --------------------------------------------------------------- wersje ----

/// Wersja działającej powłoki.
pub fn running_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Wyciąga pierwszą wersję `x.y` lub `x.y.z` z tekstu (jak regex w `blue`).
pub fn parse_version_text(raw: &str) -> Option<String> {
    let re = regex::Regex::new(r"(\d+\.\d+(?:\.\d+)?)").ok()?;
    re.captures(raw).map(|c| c[1].to_string())
}

fn version_parts(v: &str) -> Vec<u64> {
    v.trim_start_matches('v')
        .split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

/// Porównanie numeryczne; brakujące segmenty liczą się jako 0 (0.8 == 0.8.0).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let (pa, pb) = (version_parts(a), version_parts(b));
    for i in 0..pa.len().max(pb.len()) {
        let o = pa.get(i).copied().unwrap_or(0).cmp(&pb.get(i).copied().unwrap_or(0));
        if o != Ordering::Equal {
            return o;
        }
    }
    Ordering::Equal
}

// ------------------------------------------------------------ sieć/instal --

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("Blue-Environment/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(300))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

/// Pobiera numer najnowszej wersji.
async fn fetch_remote_version() -> Result<String, String> {
    let body = http()?
        .get(VERSION_URL)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    parse_version_text(&body).ok_or_else(|| "nieprawidłowy plik version.hacker".to_string())
}

/// Najwyższa wersja, jaką ten użytkownik już ma (działająca albo staged).
fn newest_local(state: &UpdateState) -> String {
    let mut best = running_version();
    if let Some(s) = &state.staged_version {
        if compare_versions(s, &best) == Ordering::Greater {
            best = s.clone();
        }
    }
    best
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
}

/// Jeden przebieg sprawdzenia. `Ok(Some)` = jest nowsza, nieignorowana wersja.
pub async fn check() -> Result<Option<UpdateInfo>, String> {
    let remote = fetch_remote_version().await?;
    let mut state = load_state();
    state.last_check = Some(chrono::Utc::now().to_rfc3339());
    let local = newest_local(&state);
    let newer = compare_versions(&remote, &local) == Ordering::Greater;
    state.available_version = newer.then(|| remote.clone());
    save_state(&state)?;
    if newer && state.ignored_version.as_deref() != Some(remote.as_str()) {
        Ok(Some(UpdateInfo { version: remote, current: running_version() }))
    } else {
        Ok(None)
    }
}

/// Pobiera binarkę do `staged/` i atomowo przełącza symlink `current`.
pub async fn install(version: &str) -> Result<(), String> {
    let version = version.trim_start_matches('v');
    if parse_version_text(version).as_deref() != Some(version) {
        return Err("nieprawidłowy numer wersji".into());
    }
    let dir = staged_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let final_path = dir.join(format!("blue-environment-{version}"));
    let part_path = dir.join(format!("blue-environment-{version}.part"));
    let url = format!("{RELEASE_BASE}/v{version}/blue-environment");
    let client = http()?;

    let resp = client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("pobieranie nie powiodło się: {e}"))?;

    let mut hasher = Sha256::new();
    let mut file = fs::File::create(&part_path).map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    let mut total: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            let _ = fs::remove_file(&part_path);
            format!("przerwane pobieranie: {e}")
        })?;
        hasher.update(&chunk);
        total += chunk.len() as u64;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
    }
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);

    if total < 1_000_000 {
        let _ = fs::remove_file(&part_path);
        return Err("pobrany plik jest podejrzanie mały".into());
    }

    // Suma kontrolna jest opcjonalna: weryfikujemy, gdy release ją publikuje.
    if let Ok(r) = client.get(format!("{url}.sha256")).send().await {
        if r.status().is_success() {
            if let Ok(text) = r.text().await {
                let expected = text.split_whitespace().next().unwrap_or("").to_lowercase();
                let actual = format!("{:x}", hasher.finalize());
                if !expected.is_empty() && expected != actual {
                    let _ = fs::remove_file(&part_path);
                    return Err("suma SHA-256 się nie zgadza — porzucam pobraną wersję".into());
                }
            }
        }
    }

    fs::set_permissions(&part_path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    fs::rename(&part_path, &final_path).map_err(|e| e.to_string())?;

    // Atomowe przełączenie: tymczasowy symlink + rename nad `current`.
    let tmp_link = dir.join("current.new");
    let _ = fs::remove_file(&tmp_link);
    std::os::unix::fs::symlink(&final_path, &tmp_link).map_err(|e| e.to_string())?;
    fs::rename(&tmp_link, dir.join("current")).map_err(|e| e.to_string())?;
    fs::write(dir.join("current.version"), format!("{version}\n")).map_err(|e| e.to_string())?;

    let mut state = load_state();
    state.staged_version = Some(version.to_string());
    state.available_version = None;
    save_state(&state)?;

    prune_old(&dir, version);
    Ok(())
}

/// Zostawia tylko aktualną i poprzednią binarkę (na wypadek ręcznego rollbacku).
fn prune_old(dir: &PathBuf, keep: &str) {
    let mut bins: Vec<(String, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_prefix("blue-environment-")
                .filter(|v| !v.ends_with(".part"))
                .map(|v| (v.to_string(), e.path()))
        })
        .collect();
    bins.sort_by(|a, b| compare_versions(&b.0, &a.0));
    for (ver, path) in bins.into_iter().skip(2) {
        if ver != keep {
            let _ = fs::remove_file(path);
        }
    }
}

// ----------------------------------------------------------- tło + zdarzenia

/// Cichy proces w tle — startuje razem z powłoką, użytkownik niczego nie widzi
/// dopóki nie ma nowej wersji.
pub fn spawn_background_checker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            run_once(&app).await;
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

async fn run_once(app: &AppHandle) {
    let info = match check().await {
        Ok(Some(i)) => i,
        Ok(None) => return,
        Err(e) => {
            tracing::debug!("shell-update: sprawdzenie nieudane: {e}");
            return;
        }
    };
    if load_state().auto_update {
        match install(&info.version).await {
            Ok(()) => {
                let _ = app.emit("shell-update-staged", &info);
            }
            Err(e) => {
                tracing::warn!("shell-update: automatyczna instalacja nieudana: {e}");
                // Daj użytkownikowi szansę zrobić to ręcznie.
                let _ = app.emit("shell-update-available", &info);
            }
        }
    } else {
        let _ = app.emit("shell-update-available", &info);
    }
}

// --------------------------------------------------------------- komendy ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    #[serde(flatten)]
    pub state: UpdateState,
    pub running_version: String,
}

#[tauri::command]
pub fn shell_update_state() -> UpdateStatus {
    UpdateStatus { state: load_state(), running_version: running_version() }
}

#[tauri::command]
pub fn shell_update_set_auto(enabled: bool) -> Result<(), String> {
    let mut s = load_state();
    s.auto_update = enabled;
    save_state(&s)
}

#[tauri::command]
pub fn shell_update_ignore(version: String) -> Result<(), String> {
    let mut s = load_state();
    s.ignored_version = Some(version);
    save_state(&s)
}

#[tauri::command]
pub async fn shell_update_check_now() -> Result<Option<UpdateInfo>, String> {
    // Ręczne sprawdzenie z Ustawień ignoruje flagę „ignorowana wersja".
    let mut s = load_state();
    s.ignored_version = None;
    save_state(&s)?;
    check().await
}

#[tauri::command]
pub async fn shell_update_install(app: AppHandle, version: String) -> Result<(), String> {
    install(&version).await?;
    let _ = app.emit(
        "shell-update-staged",
        UpdateInfo { version, current: running_version() },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert_eq!(compare_versions("0.8", "0.8.0"), Ordering::Equal);
        assert_eq!(compare_versions("0.9", "0.8.5"), Ordering::Greater);
        assert_eq!(compare_versions("0.10", "0.9"), Ordering::Greater);
        assert_eq!(compare_versions("v1.0.0", "0.99.9"), Ordering::Greater);
    }

    #[test]
    fn version_parsing() {
        assert_eq!(parse_version_text("0.8.1\n"), Some("0.8.1".into()));
        assert_eq!(parse_version_text("version = 1.2"), Some("1.2".into()));
        assert_eq!(parse_version_text("brak"), None);
    }

    #[test]
    fn state_defaults_to_auto() {
        assert!(UpdateState::default().auto_update);
    }
}
