use crate::cache::{self, CachedApp};
use regex::Regex;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Categories that indicate a GUI application – rozszerzona lista
const GUI_CATEGORIES: &[&str] = &[
    "AudioVideo", "Audio", "Video", "Development", "Education",
"Game", "Graphics", "Network", "Office", "Science",
"Settings", "System", "Utility", "WebBrowser", "FileManager",
"TextEditor", "IDE", "Calendar", "ContactsManager",
"InstantMessaging", "VideoConference", "Calculator", "TerminalEmulator",
"Shell", "ConsoleOnly", // dopuszczamy też terminale
];

/// Exec flags to strip from exec strings
const EXEC_FLAGS_RE: &str = r"\s+%[fFuUdDnNickvm]";

fn is_gui_app(content: &str) -> bool {
    // Skip NoDisplay entries
    if content.lines().any(|l| l.trim() == "NoDisplay=true") {
        return false;
    }
    // Skip entries without a proper name section
    if !content.contains("[Desktop Entry]") {
        return false;
    }
    // Must have at least one GUI category OR an Icon (heuristic) OR be a terminal emulator
    let has_gui_category = GUI_CATEGORIES.iter().any(|cat| {
        content.lines().any(|l| {
            l.starts_with("Categories=") && l.contains(cat)
        })
    });
    let has_icon = content.lines().any(|l| l.starts_with("Icon="));
    let is_terminal = content.lines().any(|l| l.contains("Terminal=true") && l.contains("Categories=System;TerminalEmulator;"));
    // Dodatkowo: jeśli Exec wskazuje na znaną aplikację terminalową
    let is_known_terminal = content.lines().any(|l| l.starts_with("Exec=") && (l.contains("gnome-terminal") || l.contains("konsole") || l.contains("xterm") || l.contains("alacritty") || l.contains("kitty")));

    has_gui_category || has_icon || is_terminal || is_known_terminal
}

fn find_icon_path(icon_name: &str) -> String {
    if icon_name.starts_with('/') && std::path::Path::new(icon_name).exists() {
        return icon_name.to_string();
    }

    // Common icon theme paths
    let search_dirs = [
        "/usr/share/icons/hicolor/128x128/apps",
        "/usr/share/icons/hicolor/64x64/apps",
        "/usr/share/icons/hicolor/48x48/apps",
        "/usr/share/icons/hicolor/scalable/apps",
        "/usr/share/icons/Adwaita/48x48/apps",
        "/usr/share/pixmaps",
        "/usr/share/icons",
    ];
    let extensions = ["png", "svg", "xpm"];

    for dir in &search_dirs {
        for ext in &extensions {
            let path = PathBuf::from(dir).join(format!("{}.{}", icon_name, ext));
            if path.exists() {
                return format!("file://{}", path.to_string_lossy());
            }
        }
    }
    // Return original name — frontend can try to resolve via CSS/fallback
    icon_name.to_string()
}

pub fn scan_desktop_apps(force_refresh: bool) -> Vec<CachedApp> {
    // Return cached version if available and not forcing refresh
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
        if !dir.exists() { continue; }

        for entry in WalkDir::new(dir).max_depth(2).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "desktop") {
                if let Ok(content) = fs::read_to_string(path) {
                    if !is_gui_app(&content) { continue; }

                    let get = |key: &str| -> String {
                        content.lines()
                        .find(|l| l.starts_with(&format!("{}=", key)))
                        .map(|l| l.splitn(2, '=').nth(1).unwrap_or("").trim().to_string())
                        .unwrap_or_default()
                    };

                    let name = get("Name");
                    if name.is_empty() { continue; }

                    // Deduplicate by name
                    if seen_names.contains(&name) { continue; }
                    seen_names.insert(name.clone());

                    let raw_exec = get("Exec");
                    let clean_exec = flag_re.replace_all(&raw_exec, "").to_string();
                    let exec = clean_exec.trim().to_string();
                    if exec.is_empty() { continue; }

                    let icon_raw = get("Icon");
                    let icon = find_icon_path(&icon_raw);

                    let categories: Vec<String> = get("Categories")
                    .split(';')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();

                    let id = path.file_stem()
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
        }
    }

    // Sort alphabetically
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // Save to cache
    cache::save_app_cache(&apps);

    // Add external apps (not cached separately)
    apps.extend(cache::list_external_apps());

    apps
}
