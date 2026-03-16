// src-tauri/src/cache.rs
// Manages all Blue Environment cache files

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn cache_dir() -> PathBuf {
    dirs::home_dir()
    .unwrap_or(PathBuf::from("/tmp"))
    .join(".cache/Blue-Environment")
}

fn config_dir() -> PathBuf {
    dirs::home_dir()
    .unwrap_or(PathBuf::from("/tmp"))
    .join(".config/Blue-Environment")
}

fn apps_dir() -> PathBuf {
    dirs::home_dir()
    .unwrap_or(PathBuf::from("/tmp"))
    .join(".hackeros/Blue-Environment/apps")
}

pub fn ensure_dirs() {
    let _ = fs::create_dir_all(cache_dir());
    let _ = fs::create_dir_all(config_dir());
    let _ = fs::create_dir_all(apps_dir());
}

// ── App cache ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CachedApp {
    pub id: String,
    pub name: String,
    pub comment: String,
    pub icon: String,
    pub exec: String,
    pub categories: Vec<String>,
    pub desktop_file: String,
    pub is_external: bool,
}

#[derive(Serialize, Deserialize)]
struct AppCache {
    version: u32,
    timestamp: u64,
    apps: Vec<CachedApp>,
}

pub fn load_app_cache() -> Option<Vec<CachedApp>> {
    let path = cache_dir().join("apps.json");
    let content = fs::read_to_string(&path).ok()?;
    let cache: AppCache = serde_json::from_str(&content).ok()?;

    // Invalidate cache older than 1 hour
    let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
    if now - cache.timestamp > 3600 {
        return None;
    }
    Some(cache.apps)
}

pub fn save_app_cache(apps: &[CachedApp]) {
    let cache = AppCache {
        version: 1,
        timestamp: SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs(),
        apps: apps.to_vec(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = fs::write(cache_dir().join("apps.json"), json);
    }
}

pub fn invalidate_app_cache() {
    let _ = fs::remove_file(cache_dir().join("apps.json"));
}

// ── Window state cache (restore last session) ─────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowCache {
    pub app_id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub workspace: u32,
    pub is_maximized: bool,
}

pub fn save_window_state(windows: &[WindowCache]) {
    if let Ok(json) = serde_json::to_string_pretty(windows) {
        let _ = fs::write(cache_dir().join("windows.json"), json);
    }
}

pub fn load_window_state() -> Vec<WindowCache> {
    let path = cache_dir().join("windows.json");
    fs::read_to_string(&path)
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default()
}

// ── Recent apps ───────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct RecentApps {
    apps: Vec<String>, // app IDs, newest first
}

pub fn record_app_launch(app_id: &str) {
    let path = cache_dir().join("recent_apps.json");
    let mut recent: Vec<String> = fs::read_to_string(&path)
    .ok()
    .and_then(|s| serde_json::from_str::<RecentApps>(&s).ok())
    .map(|r| r.apps)
    .unwrap_or_default();

    recent.retain(|a| a != app_id);
    recent.insert(0, app_id.to_string());
    recent.truncate(20);

    let data = RecentApps { apps: recent };
    if let Ok(json) = serde_json::to_string_pretty(&data) {
        let _ = fs::write(path, json);
    }
}

pub fn get_recent_apps() -> Vec<String> {
    let path = cache_dir().join("recent_apps.json");
    fs::read_to_string(&path)
    .ok()
    .and_then(|s| serde_json::from_str::<RecentApps>(&s).ok())
    .map(|r| r.apps)
    .unwrap_or_default()
}

// ── User config (settings.json) ───────────────────────────────────────────

pub fn save_user_config(config: &str) {
    let _ = fs::write(config_dir().join("settings.json"), config);
}

pub fn load_user_config() -> String {
    fs::read_to_string(config_dir().join("settings.json"))
    .unwrap_or("{}".to_string())
}

// ── External app info ─────────────────────────────────────────────────────

pub fn get_app_binary_path(app_name: &str) -> PathBuf {
    apps_dir().join(app_name).join(app_name)
}

pub fn get_app_icon_path(app_name: &str) -> PathBuf {
    apps_dir().join(app_name).join("icon.png")
}

pub fn list_external_apps() -> Vec<CachedApp> {
    let mut apps = Vec::new();
    let dir = apps_dir();
    if !dir.exists() { return apps; }

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let binary = entry.path().join(&name);
                let icon = entry.path().join("icon.png");

                if binary.exists() {
                    apps.push(CachedApp {
                        id: format!("external.{}", name),
                              name: name.replace('-', " ")
                              .split_whitespace()
                              .map(|w| {
                                  let mut c = w.chars();
                                  match c.next() {
                                      None => String::new(),
                                   Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                                  }
                              })
                              .collect::<Vec<_>>()
                              .join(" "),
                              comment: String::new(),
                              icon: if icon.exists() {
                                  format!("file://{}", icon.to_string_lossy())
                              } else {
                                  String::new()
                              },
                              exec: binary.to_string_lossy().to_string(),
                              categories: vec!["External".to_string()],
                              desktop_file: String::new(),
                              is_external: true,
                    });
                }
            }
        }
    }
    apps
}
