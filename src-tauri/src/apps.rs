use crate::cache::{self, CachedApp};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

const GUI_CATEGORIES: &[&str] = &[
    "AudioVideo", "Audio", "Video", "Development", "Education",
"Game", "Graphics", "Network", "Office", "Science",
"Settings", "System", "Utility", "WebBrowser", "FileManager",
"TextEditor", "IDE", "Calendar", "ContactsManager",
"InstantMessaging", "VideoConference", "Calculator", "TerminalEmulator",
];

const EXEC_FLAGS_RE: &str = r"\s+%[fFuUdDnNickvm]";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub comment: String,
    pub icon: String,
    pub exec: String,
    pub categories: Vec<String>,
    pub desktop_file: String,
    pub is_external: bool,
}

impl From<CachedApp> for AppInfo {
    fn from(c: CachedApp) -> Self {
        AppInfo {
            id: c.id,
            name: c.name,
            comment: c.comment,
            icon: c.icon,
            exec: c.exec,
            categories: c.categories,
            desktop_file: c.desktop_file,
            is_external: c.is_external,
        }
    }
}

pub async fn get_installed_apps() -> Result<Vec<AppInfo>, String> {
    let apps = tokio::task::spawn_blocking(|| scan_desktop_apps(false))
    .await
    .map_err(|e| e.to_string())?;
    Ok(apps.into_iter().map(AppInfo::from).collect())
}

pub async fn launch_app(app_id: &str) -> Result<bool, String> {
    let app_id = app_id.to_string();
    tokio::task::spawn_blocking(move || {
        let apps = scan_desktop_apps(false);
        if let Some(app) = apps.iter().find(|a| a.id == app_id) {
            let exec = app.exec.clone();
            std::process::Command::new("sh")
            .arg("-c")
            .arg(&exec)
            .spawn()
            .map(|_| true)
            .unwrap_or(false)
        } else {
            false
        }
    })
    .await
    .map_err(|e| e.to_string())
}

fn is_gui_app(content: &str) -> bool {
    if content.lines().any(|l| l.trim() == "NoDisplay=true") {
        return false;
    }
    if !content.contains("[Desktop Entry]") {
        return false;
    }
    let has_gui_category = GUI_CATEGORIES.iter().any(|cat| {
        content
        .lines()
        .any(|l| l.starts_with("Categories=") && l.contains(cat))
    });
    let has_icon = content.lines().any(|l| l.starts_with("Icon="));
    has_gui_category || has_icon
}

// Icon lookup is now handled by the shared `icon_resolver` module (uses
// the `linicon` crate — full FreeDesktop Icon Theme spec support,
// including Papirus and any other installed theme, not just a fixed
// hicolor/Adwaita path list). See src-tauri/src/icon_resolver.rs.
fn find_icon_path(icon_name: &str) -> String {
    crate::icon_resolver::resolve_icon(icon_name)
}

pub fn scan_desktop_apps(force_refresh: bool) -> Vec<CachedApp> {
    if !force_refresh {
        if let Some(cached) = cache::load_app_cache() {
            let mut all = cached;
            all.extend(cache::list_external_apps());
            return all;
        }
    }

    let flag_re = Regex::new(EXEC_FLAGS_RE).unwrap();
    let search_dirs = [
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        dirs::home_dir()
        .unwrap_or(PathBuf::from("/"))
        .join(".local/share/applications"),
    ];

    let mut apps: Vec<CachedApp> = Vec::new();
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(dir).max_depth(2).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.extension().map_or(false, |e| e == "desktop") {
                continue;
            }
            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if !is_gui_app(&content) {
                continue;
            }

            let get = |key: &str| -> String {
                content
                .lines()
                .find(|l| l.starts_with(&format!("{}=", key)))
                .map(|l| l.splitn(2, '=').nth(1).unwrap_or("").trim().to_string())
                .unwrap_or_default()
            };

            let name = get("Name");
            if name.is_empty() {
                continue;
            }
            if seen_names.contains(&name) {
                continue;
            }
            seen_names.insert(name.clone());

            let raw_exec = get("Exec");
            let exec = flag_re.replace_all(&raw_exec, "").trim().to_string();
            if exec.is_empty() {
                continue;
            }

            let icon = find_icon_path(&get("Icon"));
            let categories: Vec<String> = get("Categories")
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

            let id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| name.clone());

            apps.push(CachedApp {
                id,
                name,
                comment: get("Comment"),
                      icon,
                      exec,
                      categories,
                      desktop_file: path.to_string_lossy().to_string(),
                      is_external: false,
            });
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    cache::save_app_cache(&apps);
    apps.extend(cache::list_external_apps());
    apps
}
