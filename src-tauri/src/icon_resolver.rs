use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::RwLock;

/// User-chosen icon theme override (Settings → Personalization → Icon
/// Theme). `None` means "use the auto-detected system theme + Papirus
/// fallback chain below", same as before this was made configurable.
static USER_ICON_THEME: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));

pub fn set_icon_theme(theme: Option<String>) {
    if let Ok(mut guard) = USER_ICON_THEME.write() {
        *guard = theme;
    }
}

pub fn get_icon_theme() -> Option<String> {
    USER_ICON_THEME.read().ok().and_then(|g| g.clone())
}

/// Lists installed icon themes by scanning for `index.theme` files —
/// used by the Settings "Icon Theme" picker so it only ever offers themes
/// that are actually present, rather than a hardcoded guess list.
pub fn list_installed_icon_themes() -> Vec<String> {
    let mut themes = Vec::new();
    let search_roots = [
        PathBuf::from("/usr/share/icons"),
        dirs::home_dir().unwrap_or_default().join(".local/share/icons"),
        dirs::home_dir().unwrap_or_default().join(".icons"),
    ];
    for root in &search_roots {
        let Ok(entries) = std::fs::read_dir(root) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("index.theme").exists() && crate::icon_store::is_icon_theme_dir(&path) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !themes.iter().any(|t: &String| t == name) {
                        themes.push(name.to_string());
                    }
                }
            }
        }
    }
    themes.sort();
    themes
}

/// Icon themes tried, in priority order, before falling back to
/// linicon's own automatic fallback chain (which ultimately ends at
/// "hicolor", the spec-mandated default).
fn preferred_themes() -> Vec<String> {
    let mut themes = Vec::new();

    // Explicit user override (Settings) takes priority over everything.
    if let Some(t) = get_icon_theme() {
        themes.push(t);
    }

    // Respect the user's actual configured icon theme first, if linicon
    // can determine one (it checks kdeglobals, gsettings, gtk-3.0
    // settings.ini, .gtkrc-2.0, and theme.conf, in that order).
    if let Some(t) = linicon::get_system_theme() {
        if !themes.iter().any(|x| x == &t) {
            themes.push(t);
        }
    }

    // Papirus is the practical default for Blue Environment. It ships
    // with (near-)complete coverage of common Linux applications and is
    // pre-installed or easily installable on virtually every distro
    // Blue targets. We try every shipped variant in case only one of
    // them happens to be installed.
    for variant in ["Papirus", "Papirus-Dark", "Papirus-Light", "ePapirus", "ePapirus-Dark"] {
        if !themes.iter().any(|t| t == variant) {
            themes.push(variant.to_string());
        }
    }

    themes
}

/// Sizes tried per theme, largest-reasonable first — Blue's app grid and
/// taskbar both render around 40-64px, so we ask for that first rather
/// than settling for whatever a theme happens to list first internally.
const PREFERRED_SIZES: &[u16] = &[64, 48, 128, 32, 16];

/// Resolves `icon_name` (as found in a .desktop file's `Icon=` field, or
/// a bare executable/app name for apps that don't have one) to an actual
/// icon file path, returned as a `file://` URI ready to hand to the
/// frontend. Returns an empty string if no icon could be found anywhere.
///
/// `icon_name` may also already be an absolute path (some .desktop files
/// do this) — that case is resolved directly without going through theme
/// lookup at all.
pub fn resolve_icon(icon_name: &str) -> String {
    if icon_name.is_empty() {
        return String::new();
    }

    if icon_name.starts_with('/') {
        return if PathBuf::from(icon_name).exists() {
            format!("file://{}", icon_name)
        } else {
            String::new()
        };
    }

    if let Some(path) = lookup_via_linicon(icon_name) {
        return format!("file://{}", path.to_string_lossy());
    }

    // Last-resort safety net for systems where theme parsing fails
    // entirely (e.g. a minimal container with no index.theme files at
    // all) — keeps the old fixed-path behaviour working rather than ever
    // silently returning nothing when a file genuinely exists at one of
    // these conventional locations.
    legacy_fallback_search(icon_name)
}

fn lookup_via_linicon(icon_name: &str) -> Option<PathBuf> {
    for theme in preferred_themes().iter() {
        for &size in PREFERRED_SIZES {
            if let Some(Ok(icon)) = linicon::lookup_icon(icon_name)
                .from_theme(theme)
                .with_size(size)
                .next()
            {
                return Some(icon.path);
            }
        }
    }

    // No explicitly-preferred theme matched — let linicon search through
    // its automatically-detected system theme (if different from the
    // ones above) and its full Inherits= fallback chain, ending at
    // hicolor per spec.
    for result in linicon::lookup_icon(icon_name) {
        if let Ok(icon) = result {
            return Some(icon.path);
        }
    }

    None
}

fn legacy_fallback_search(icon_name: &str) -> String {
    let search_dirs = [
        "/usr/share/icons/Papirus/64x64/apps",
        "/usr/share/icons/Papirus/48x48/apps",
        "/usr/share/icons/Papirus-Dark/64x64/apps",
        "/usr/share/icons/hicolor/128x128/apps",
        "/usr/share/icons/hicolor/64x64/apps",
        "/usr/share/icons/hicolor/48x48/apps",
        "/usr/share/icons/hicolor/scalable/apps",
        "/usr/share/icons/Adwaita/48x48/apps",
        "/usr/share/pixmaps",
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
    String::new()
}
