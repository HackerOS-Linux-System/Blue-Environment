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
        .join(".legendaryos/Blue-Environment/apps")
}

pub fn ensure_dirs() {
    let _ = fs::create_dir_all(cache_dir());
    let _ = fs::create_dir_all(config_dir());
    let _ = fs::create_dir_all(apps_dir());
    let _ = fs::create_dir_all(
        dirs::home_dir()
            .unwrap_or(PathBuf::from("/tmp"))
            .join(".local/share/blue-env"),
    );
}

// ── User configuration ─────────────────────────────────────────────────────

/// The whole-shell settings blob, round-tripped through
/// `commands::config::save_config`/`load_config` as a JSON string (see
/// those two — `save_config` takes and parses a raw `String` rather
/// than a structured Tauri command argument, precisely so the frontend
/// can send its entire `UserConfig` object in one call).
///
/// `#[serde(rename_all = "camelCase")]` here is not cosmetic — it fixes
/// a real, severe bug this struct had until now. The frontend's
/// `SystemBridge.saveConfig` (systemBridge.ts) does
/// `JSON.stringify(config)` on a TypeScript object with genuinely
/// camelCase keys (`panelOpacity`, `themeName`, `accentColor`,
/// `displayScale`, `panelEnabled`, `nightLightEnabled`, …). Without this
/// attribute, every multi-word field here has a *different* name
/// (`panel_opacity`, `theme_name`, …) than the JSON key it's supposed to
/// deserialize from. `serde_json::from_str::<UserConfig>` then fails —
/// not per-field, for the *whole struct*, since none of these fields had
/// `#[serde(default)]` either — and `save_config`'s
/// `.unwrap_or_default()` silently swallows that error and writes a
/// completely blank, all-default `UserConfig` to `settings.json` instead
/// of whatever the person actually changed. The very first time any
/// setting was ever saved, every *other* setting silently reset to its
/// Rust default (empty wallpaper, empty theme, `panel_enabled: false`,
/// …) on disk — invisible in the same session (the frontend also mirrors
/// every save to `localStorage` and prefers that in-memory state while
/// running), but restored as pure defaults on the next full app restart,
/// since `load_config`'s result already isn't literal `"{}"` by that
/// point, so the frontend trusts it over `localStorage` — see
/// `loadConfig()`'s comment in systemBridge.ts. In short: nothing a
/// person configured ever actually survived a restart. Single-word
/// fields (`wallpaper`, `theme`, `language`) happened to still match by
/// coincidence, which is likely why this went unnoticed — most of the
/// UI still looked "mostly right" after a restart.
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
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
    /// 'icon-and-label' | 'icon-only' — see TopBar.svelte's Start button
    /// and PanelSection.svelte's "App Launcher" card. Empty string (the
    /// `Default` value) is treated as 'icon-and-label' by the frontend,
    /// matching the button's original, always-on label.
    #[serde(default)]
    pub start_button_label_mode: String,
    /// A lucide icon name from the same closed set
    /// `file_type_associations.rs`'s `ALLOWED_ICONS` already defines
    /// (reused rather than duplicated — see that constant's doc
    /// comment) — empty string means "use the default Command icon".
    #[serde(default)]
    pub start_button_icon: String,
}

pub fn save_user_config(config: &UserConfig) {
    let _ = fs::write(
        config_dir().join("settings.json"),
        serde_json::to_string_pretty(config).unwrap_or_default(),
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

const CACHE_TTL_SECS: u64 = 3600;

pub fn load_app_cache() -> Option<Vec<CachedApp>> {
    let content = fs::read_to_string(cache_dir().join("apps.json")).ok()?;
    let cache: AppCache = serde_json::from_str(&content).ok()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if now.saturating_sub(cache.timestamp) > CACHE_TTL_SECS {
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

// ── Window state ───────────────────────────────────────────────────────────

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
    fs::read_to_string(cache_dir().join("windows.json"))
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

    if let Ok(json) = serde_json::to_string_pretty(&RecentApps { apps: recent }) {
        let _ = fs::write(path, json);
    }
}

pub fn get_recent_apps() -> Vec<String> {
    fs::read_to_string(cache_dir().join("recent_apps.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<RecentApps>(&s).ok())
        .map(|r| r.apps)
        .unwrap_or_default()
}

// ── External apps ─────────────────────────────────────────────────────────

pub fn list_external_apps() -> Vec<CachedApp> {
    let dir = apps_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut apps = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return apps,
    };

    for entry in entries.flatten() {
        let app_dir = entry.path();
        if !app_dir.is_dir() {
            continue;
        }

        let app_name = entry.file_name().to_string_lossy().to_string();
        let binary = match find_binary_in_dir(&app_dir, &app_name) {
            Some(b) => b,
            None => continue,
        };

        let icon = ["icon.png", "icon.svg", "icon.jpg"]
            .iter()
            .map(|n| app_dir.join(n))
            .find(|p| p.exists())
            .map(|p| format!("file://{}", p.to_string_lossy()))
            .unwrap_or_else(|| {
                // No bundled icon shipped with this external app — fall
                // back to the system icon theme chain (Papirus etc.) by
                // app name before giving up. Previously this case just
                // produced an empty string unconditionally, so any
                // externally-installed app without its own icon.png/svg
                // rendered with no icon at all even when a matching
                // system icon existed.
                crate::icon_resolver::resolve_icon(&app_name)
            });

        let display_name = app_name
            .replace(['-', '_'], " ")
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
            id: format!("legendaryos.{}", app_name),
            name: display_name,
            comment: String::new(),
            icon,
            exec: binary.to_string_lossy().to_string(),
            categories: vec!["LegendaryOS".to_string()],
            desktop_file: String::new(),
            is_external: true,
        });
    }

    apps
}

fn find_binary_in_dir(dir: &std::path::Path, preferred_name: &str) -> Option<PathBuf> {
    let exact = dir.join(preferred_name);
    if exact.exists() && is_executable(&exact) {
        return Some(exact);
    }

    let skip_ext = ["png", "svg", "jpg", "xpm", "desktop", "json", "toml", "txt", "md"];
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
            if skip_ext.contains(&ext.as_str()) {
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

// ── Custom themes ─────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThemeDefinition {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for the config-persistence bug described in
    /// `UserConfig`'s doc comment: every multi-word field must
    /// (de)serialize under its camelCase name, since that's what the
    /// frontend's `JSON.stringify` on its own camelCase TS object
    /// actually produces.
    #[test]
    fn serializes_multi_word_fields_as_camel_case() {
        let cfg = UserConfig {
            panel_opacity: 0.9,
            theme_name: "blue-default".to_string(),
            accent_color: "blue".to_string(),
            night_light_enabled: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("\"panelOpacity\""), "expected camelCase key, got: {json}");
        assert!(json.contains("\"themeName\""), "expected camelCase key, got: {json}");
        assert!(json.contains("\"accentColor\""), "expected camelCase key, got: {json}");
        assert!(json.contains("\"nightLightEnabled\""), "expected camelCase key, got: {json}");
        assert!(!json.contains("panel_opacity"), "must not contain the old snake_case key");
    }

    #[test]
    fn round_trips_a_realistic_frontend_payload_without_losing_fields() {
        // Exactly the shape SystemBridge.saveConfig actually sends —
        // camelCase keys, straight from systemBridge.ts's own default
        // object literal — this is the payload that used to silently
        // deserialize into an all-default UserConfig before this
        // struct had `#[serde(rename_all = "camelCase")]`.
        let payload = r#"{
            "wallpaper": "", "theme": "dark", "themeName": "blue-default",
            "accentColor": "blue", "displayScale": 1.0, "desktopPath": "HOME/Desktop",
            "panelEnabled": true, "panelPosition": "top", "panelSize": 40,
            "panelOpacity": 0.9, "language": "en", "nightLightEnabled": false,
            "nightLightTemperature": 4000, "nightLightSchedule": "manual",
            "nightLightStartHour": 20, "nightLightEndHour": 6,
            "startButtonLabelMode": "icon-only", "startButtonIcon": "Rocket"
        }"#;
        let parsed: UserConfig = serde_json::from_str(payload).expect("must deserialize a real frontend payload");
        assert_eq!(parsed.theme_name, "blue-default");
        assert!(parsed.panel_enabled, "panel_enabled must not have silently reset to its false default");
        assert_eq!(parsed.panel_opacity, 0.9);
        assert_eq!(parsed.panel_size, 40);
        assert_eq!(parsed.start_button_label_mode, "icon-only");
        assert_eq!(parsed.start_button_icon, "Rocket");
    }

    #[test]
    fn missing_new_fields_default_to_empty_string_not_an_error() {
        // A settings.json written before startButtonLabelMode/Icon
        // existed must still load cleanly (`#[serde(default)]` on both).
        let payload = r#"{"wallpaper": "", "theme": "dark"}"#;
        let parsed: UserConfig = serde_json::from_str(payload).expect("old configs without the new fields must still parse");
        assert_eq!(parsed.start_button_label_mode, "");
        assert_eq!(parsed.start_button_icon, "");
    }
}
