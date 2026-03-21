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

// ── User configuration ─────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct UserConfig {
    pub wallpaper: String,
    pub theme: String,
    pub theme_name: String,
    pub accent_color: String,
    pub display_scale: f32,
    pub desktop_path: String,
    pub panel_enabled: bool,
    pub panel_position: String,
    pub panel_size: u32,
    pub panel_opacity: f32,
    pub language: String,
    pub night_light_enabled: bool,
    pub night_light_temperature: u32,
    pub night_light_schedule: String,
    pub night_light_start_hour: u32,
    pub night_light_end_hour: u32,
}

pub fn save_user_config(config: &UserConfig) {
    let _ = fs::write(
        config_dir().join("settings.json"),
                      serde_json::to_string_pretty(config).unwrap(),
    );
}

pub fn load_user_config() -> String {
    fs::read_to_string(config_dir().join("settings.json"))
    .unwrap_or_else(|_| "{}".to_string())
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

// ── Window state cache ────────────────────────────────────────────────────

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
    apps: Vec<String>,
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

// ── External apps ─────────────────────────────────────────────────────────

pub fn list_external_apps() -> Vec<CachedApp> {
    let mut apps = Vec::new();
    let dir = apps_dir();
    if !dir.exists() {
        return apps;
    }

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let app_dir = entry.path();
            if !app_dir.is_dir() {
                continue;
            }

            let app_name = entry.file_name().to_string_lossy().to_string();

            // Find binary
            let binary = find_binary_in_dir(&app_dir, &app_name);
            let binary = match binary {
                Some(b) => b,
                None => continue,
            };

            // Icon
            let icon = ["icon.png", "icon.svg", "icon.jpg", "icon.xpm"]
            .iter()
            .map(|name| app_dir.join(name))
            .find(|p| p.exists())
            .map(|p| format!("file://{}", p.to_string_lossy()))
            .unwrap_or_default();

            // Display name
            let display_name = app_name
            .replace('-', " ")
            .replace('_', " ")
            .split_whitespace()
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                 Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

            apps.push(CachedApp {
                id: format!("hackeros.{}", app_name),
                      name: display_name,
                      comment: String::new(),
                      icon,
                      exec: binary.to_string_lossy().to_string(),
                      categories: vec!["HackerOS".to_string()],
                      desktop_file: String::new(),
                      is_external: true,
            });
        }
    }
    apps
}

fn find_binary_in_dir(dir: &std::path::Path, preferred_name: &str) -> Option<std::path::PathBuf> {
    let exact = dir.join(preferred_name);
    if exact.exists() && is_executable(&exact) {
        return Some(exact);
    }

    let skip_extensions = ["png", "svg", "jpg", "xpm", "desktop", "json", "toml", "txt", "md"];
    let mut executables = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
            if skip_extensions.contains(&ext.as_str()) {
                continue;
            }
            if is_executable(&path) {
                executables.push(path);
            }
        }
    }

    executables.into_iter().next()
}

fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
    .map(|m| m.permissions().mode() & 0o111 != 0)
    .unwrap_or(false)
}

// ── Theme definition for custom themes ────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThemeDefinition {
    pub id: String,
    pub name: String,
    pub r#type: String, // "builtin" or "custom"
    pub css: Option<String>,
    pub colors: Option<ThemeColors>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThemeColors {
    pub primary: String,
    pub secondary: String,
    pub text: String,
    pub accent: String,
}
