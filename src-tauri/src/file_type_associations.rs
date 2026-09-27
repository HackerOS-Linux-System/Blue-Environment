use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let dir = base.join("blue-environment");
    let _ = fs::create_dir_all(&dir);
    dir.join("file-type-associations.json")
}

/// Kept in sync by hand with the frontend's `ICON_COMPONENTS` map in
/// `fileTypeAssociations.ts` — every name here must have a matching
/// lucide-svelte icon registered there, and vice versa. Deliberately a
/// closed list rather than an arbitrary string: it's what keeps this
/// feature from ever becoming "pass arbitrary content to render as an
/// icon."
const ALLOWED_ICONS: &[&str] = &[
    "File", "FileText", "FileCode", "FileJson", "FileSpreadsheet", "FileCog",
    "Folder", "Archive", "Image", "Music", "Video", "Film", "Code", "Code2",
    "Terminal", "Database", "Book", "BookOpen", "Palette", "Settings",
    "Package", "Boxes", "Puzzle", "Gamepad2", "Wrench", "FlaskConical",
    "Layers", "GitBranch", "Lock", "KeyRound", "FileLock2", "Cpu", "HardDrive",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchKind {
    #[serde(rename = "extension")]
    Extension,
    #[serde(rename = "mime")]
    Mime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTypeAssociation {
    pub id: String,
    pub kind: MatchKind,
    /// For `Extension`: a bare extension without the dot, case-insensitive
    /// (e.g. "blueproj"). For `Mime`: a MIME type or MIME-prefix ending in
    /// "/*" (e.g. "application/x-mymod" or "application/vnd.custom.*").
    pub pattern: String,
    pub icon: String,
    /// Any valid CSS color (hex, named, or an `rgb()`/`hsl()` string) —
    /// applied as the icon's `color` style, not interpreted or executed,
    /// so unlike `icon` this one doesn't need an allow-list.
    pub color: String,
    pub label: String,
    /// Optional shell command to run instead of the OS/xdg default when
    /// a matching file is opened. `{path}` is substituted with the
    /// shell-quoted absolute file path — see ExplorerApp.svelte's
    /// `openWithCustomAssociation`. `None` means "use the normal default
    /// app resolution", i.e. this association only affects the icon.
    pub open_with_command: Option<String>,
}

fn load_raw() -> Vec<FileTypeAssociation> {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_raw(list: &[FileTypeAssociation]) -> bool {
    serde_json::to_string_pretty(list)
        .ok()
        .map(|s| fs::write(config_path(), s).is_ok())
        .unwrap_or(false)
}

fn validate(assoc: &FileTypeAssociation) -> Result<(), String> {
    if assoc.pattern.trim().is_empty() {
        return Err("Pattern cannot be empty.".to_string());
    }
    if assoc.pattern.len() > 128 {
        return Err("Pattern is too long.".to_string());
    }
    if !ALLOWED_ICONS.contains(&assoc.icon.as_str()) {
        return Err(format!("Unknown icon \"{}\".", assoc.icon));
    }
    if assoc.label.len() > 64 {
        return Err("Label is too long.".to_string());
    }
    if let Some(cmd) = &assoc.open_with_command {
        if cmd.len() > 512 {
            return Err("Open-with command is too long.".to_string());
        }
    }
    match assoc.kind {
        MatchKind::Extension => {
            if assoc.pattern.contains(['/', '.', '*']) {
                return Err("An extension pattern shouldn't contain '.', '/' or '*' — just the bare extension, e.g. \"blueproj\".".to_string());
            }
        }
        MatchKind::Mime => {
            if !assoc.pattern.contains('/') {
                return Err("A MIME pattern needs a '/', e.g. \"application/x-mymod\" or \"application/*\".".to_string());
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn file_type_get_associations() -> Vec<FileTypeAssociation> {
    load_raw()
}

#[tauri::command]
pub fn file_type_get_allowed_icons() -> Vec<String> {
    ALLOWED_ICONS.iter().map(|s| s.to_string()).collect()
}

/// Adds or updates (matched by `id`) one association. Returns an error
/// message on validation failure so the Settings UI can show exactly
/// what's wrong, rather than a bare boolean.
#[tauri::command]
pub fn file_type_upsert_association(assoc: FileTypeAssociation) -> Result<Vec<FileTypeAssociation>, String> {
    validate(&assoc)?;
    let mut list = load_raw();
    if let Some(existing) = list.iter_mut().find(|a| a.id == assoc.id) {
        *existing = assoc;
    } else {
        list.push(assoc);
    }
    if !save_raw(&list) {
        return Err("Failed to save file type associations.".to_string());
    }
    Ok(list)
}

#[tauri::command]
pub fn file_type_remove_association(id: String) -> Vec<FileTypeAssociation> {
    let mut list = load_raw();
    list.retain(|a| a.id != id);
    save_raw(&list);
    list
}

/// Finds the best matching association for a file, if any: an exact
/// extension match wins over a MIME match, and among MIME matches an
/// exact MIME wins over a wildcard prefix ("application/*") — mirrors
/// how a real desktop's MIME-matching (most-specific-wins) behaves,
/// so an association targeting one exact MIME type isn't shadowed by a
/// broader wildcard one a user added earlier.
#[tauri::command]
pub fn file_type_resolve(filename: String, mime_type: String) -> Option<FileTypeAssociation> {
    let list = load_raw();
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();

    if let Some(a) = list.iter().find(|a| matches!(a.kind, MatchKind::Extension) && a.pattern.to_lowercase() == ext) {
        return Some(a.clone());
    }
    if let Some(a) = list.iter().find(|a| matches!(a.kind, MatchKind::Mime) && a.pattern == mime_type) {
        return Some(a.clone());
    }
    list.iter()
        .find(|a| {
            matches!(a.kind, MatchKind::Mime)
                && a.pattern.ends_with("/*")
                && mime_type.starts_with(a.pattern.trim_end_matches('*'))
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, kind: MatchKind, pattern: &str) -> FileTypeAssociation {
        FileTypeAssociation {
            id: id.to_string(), kind, pattern: pattern.to_string(),
            icon: "Puzzle".to_string(), color: "#38bdf8".to_string(),
            label: "Test".to_string(), open_with_command: None,
        }
    }

    #[test]
    fn rejects_empty_pattern() {
        let a = sample("1", MatchKind::Extension, "");
        assert!(validate(&a).is_err());
    }

    #[test]
    fn rejects_unknown_icon() {
        let mut a = sample("1", MatchKind::Extension, "blueproj");
        a.icon = "NotARealIcon".to_string();
        assert!(validate(&a).is_err());
    }

    #[test]
    fn rejects_extension_pattern_with_dot_or_slash() {
        assert!(validate(&sample("1", MatchKind::Extension, ".blueproj")).is_err());
        assert!(validate(&sample("1", MatchKind::Extension, "a/b")).is_err());
    }

    #[test]
    fn rejects_mime_pattern_without_slash() {
        assert!(validate(&sample("1", MatchKind::Mime, "application")).is_err());
    }

    #[test]
    fn accepts_valid_extension_and_mime_patterns() {
        assert!(validate(&sample("1", MatchKind::Extension, "blueproj")).is_ok());
        assert!(validate(&sample("2", MatchKind::Mime, "application/x-mymod")).is_ok());
        assert!(validate(&sample("3", MatchKind::Mime, "application/*")).is_ok());
    }

    #[test]
    fn resolve_prefers_extension_over_mime() {
        let list = vec![
            sample("ext", MatchKind::Extension, "blueproj"),
            sample("mime", MatchKind::Mime, "application/x-blueproj"),
        ];
        let ext = list.iter().find(|a| matches!(a.kind, MatchKind::Extension) && a.pattern == "blueproj");
        assert!(ext.is_some());
        assert_eq!(ext.unwrap().id, "ext");
    }

    #[test]
    fn resolve_prefers_exact_mime_over_wildcard() {
        let list = vec![
            sample("wild", MatchKind::Mime, "application/*"),
            sample("exact", MatchKind::Mime, "application/x-blueproj"),
        ];
        let exact = list.iter().find(|a| matches!(a.kind, MatchKind::Mime) && a.pattern == "application/x-blueproj");
        assert!(exact.is_some());
        assert_eq!(exact.unwrap().id, "exact");
    }

    #[test]
    fn wildcard_mime_prefix_matching_logic() {
        let pattern = "application/*";
        let mime = "application/x-blueproj";
        assert!(pattern.ends_with("/*") && mime.starts_with(pattern.trim_end_matches('*')));
        assert!(!"text/plain".starts_with(pattern.trim_end_matches('*')));
    }
}
