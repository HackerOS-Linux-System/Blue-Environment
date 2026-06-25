use once_cell::sync::Lazy;
use std::path::PathBuf;

/// Icon themes tried, in priority order, before falling back to
/// linicon's own automatic fallback chain (which ultimately ends at
/// "hicolor", the spec-mandated default).
static PREFERRED_THEMES: Lazy<Vec<String>> = Lazy::new(|| {
    let mut themes = Vec::new();

    // Respect the user's actual configured icon theme first, if linicon
    // can determine one (it checks kdeglobals, gsettings, gtk-3.0
    // settings.ini, .gtkrc-2.0, and theme.conf, in that order).
    if let Some(t) = linicon::get_system_theme() {
        themes.push(t);
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
});

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
    for theme in PREFERRED_THEMES.iter() {
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
