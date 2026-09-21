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

/// In-memory copy of the last scan. The on-disk cache (`apps.json`) still
/// exists, but re-reading/parsing it *and* re-resolving icons for every
/// external app on every Start Menu open was a measurable part of the
/// "Installed apps" freeze — this makes repeat opens instant.
static MEM_CACHE: once_cell::sync::Lazy<std::sync::Mutex<Option<(std::time::Instant, Vec<CachedApp>)>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(None));
const MEM_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(120);

pub fn clear_memory_cache() {
    if let Ok(mut g) = MEM_CACHE.lock() {
        *g = None;
    }
}

pub fn scan_desktop_apps(force_refresh: bool) -> Vec<CachedApp> {
    if !force_refresh {
        if let Ok(g) = MEM_CACHE.lock() {
            if let Some((at, apps)) = g.as_ref() {
                if at.elapsed() < MEM_CACHE_TTL {
                    return apps.clone();
                }
            }
        }
    }
    let apps = scan_desktop_apps_uncached(force_refresh);
    if let Ok(mut g) = MEM_CACHE.lock() {
        *g = Some((std::time::Instant::now(), apps.clone()));
    }
    apps
}

fn scan_desktop_apps_uncached(force_refresh: bool) -> Vec<CachedApp> {
    if !force_refresh {
        if let Some(cached) = cache::load_app_cache() {
            let mut all = cached;
            all.extend(cache::list_external_apps());
            return all;
        }
    }

    let flag_re = Regex::new(EXEC_FLAGS_RE).unwrap();
    let home = dirs::home_dir().unwrap_or(PathBuf::from("/"));

    // Was previously just 3 hardcoded paths, which silently missed a lot of
    // real installs:
    //  - Flatpak apps (very common — GNOME Software / Discover both default
    //    to it) export their .desktop files under a completely different
    //    tree that isn't covered by XDG_DATA_DIRS by default.
    //  - Snap apps similarly export to their own directory.
    //  - XDG_DATA_DIRS itself wasn't consulted at all, so any distro or
    //    environment that adds its own entries there (e.g. a Nix profile,
    //    a custom /opt install, a user-session systemd override) was
    //    invisible to the Start Menu no matter what the user installed.
    let mut search_dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        home.join(".local/share/applications"),
        // Flatpak (system-wide and per-user)
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        home.join(".local/share/flatpak/exports/share/applications"),
        // Snap
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];

    // XDG_DATA_DIRS is colon-separated; each entry gets an implicit
    // "/applications" suffix per the XDG Base Directory / Desktop Entry
    // specs. Falls back to the spec's documented default if unset.
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for dir in xdg_data_dirs.split(':').filter(|s| !s.is_empty()) {
        search_dirs.push(PathBuf::from(dir).join("applications"));
    }
    // XDG_DATA_HOME (defaults to ~/.local/share) — covers the same ground
    // as the hardcoded entry above but also respects an explicit override.
    if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
        search_dirs.push(PathBuf::from(xdg_data_home).join("applications"));
    }

    // Dedup by canonical path so overlapping entries (very likely, e.g.
    // XDG_DATA_DIRS usually already includes /usr/share) don't cause the
    // same .desktop files to be walked and considered twice.
    let mut seen_dirs: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    search_dirs.retain(|d| seen_dirs.insert(d.canonicalize().unwrap_or_else(|_| d.clone())));

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
