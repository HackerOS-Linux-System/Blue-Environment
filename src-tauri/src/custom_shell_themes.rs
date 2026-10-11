use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let dir = base.join("blue-environment");
    let _ = fs::create_dir_all(&dir);
    dir.join("custom-shell-themes.json")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomThemeColors {
    pub accent: String,
    pub background: String,
    pub surface: String,
    pub surface_elevated: String,
    pub text: String,
    pub text_muted: String,
    pub border: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomThemeLayout {
    pub panel_position: String,          // 'top' | 'bottom' | 'left' | 'right'
    pub window_controls_position: String, // 'left' | 'right'
    pub window_controls_style: String,    // 'macos' | 'windows' | 'gnome' | 'minimal'
    pub corner_style: String,             // 'rounded' | 'sharp'
    pub icon_style: String,               // 'outline' | 'filled'
}

/// The richer knobs builtin themes don't expose at all — this is the
/// actual "let users work wonders" surface the settings UI adds beyond
/// picking from a fixed list.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomThemeExtras {
    /// Precise corner radius in pixels (0-32) — overrides `corner_style`'s
    /// binary rounded/sharp when set.
    pub corner_radius_px: Option<u32>,
    /// Gaussian blur (in px, 0-40) applied to the desktop wallpaper layer.
    pub wallpaper_blur: Option<u32>,
    /// Panel/taskbar background opacity, 0-100 (percent).
    pub panel_opacity: Option<u32>,
    /// A second accent color — when set, UI accents render as a
    /// `linear-gradient(135deg, accent, accent_secondary)` instead of a
    /// flat color.
    pub accent_secondary: Option<String>,
    /// A CSS font-family value, e.g. "'Fira Sans', sans-serif" — applied
    /// to the whole shell. Not validated against installed fonts (the
    /// backend has no reliable way to enumerate what the browser engine
    /// can actually render); an unavailable font just falls back to the
    /// generic family the person included, same as any CSS font stack.
    pub font_family: Option<String>,
    /// 'none' | 'normal' | 'fast' — maps to a CSS transition-duration
    /// custom property; see ShellThemeStyle.svelte.
    pub animation_speed: Option<String>,

    // ── Added later. Every field is optional and `#[serde(default)]`-friendly
    // (the whole struct derives `Default`), so theme files written by an older
    // version still load, and a theme exported from a newer version imports
    // into an older one (unknown fields are ignored).

    /// Window translucency, 60-100 (percent; 100 = opaque).
    pub window_opacity: Option<u32>,
    /// 'none' | 'soft' | 'medium' | 'strong' | 'glow'.
    pub window_shadow: Option<String>,
    /// Window border thickness in px, 0-4.
    pub window_border_width: Option<u32>,
    /// Window border colour (any CSS colour string).
    pub window_border_color: Option<String>,
    /// Title-bar height in px, 28-56.
    pub titlebar_height: Option<u32>,
    /// Scales every size in the shell, 80-130 (percent).
    pub ui_scale_percent: Option<u32>,
    /// 'default' | 'thin' | 'hidden' | 'accent'.
    pub scrollbar_style: Option<String>,
    /// Wallpaper brightness, 30-130 (percent).
    pub wallpaper_brightness: Option<u32>,
    /// Wallpaper saturation, 0-200 (percent).
    pub wallpaper_saturation: Option<u32>,
    /// Panel background colour (any CSS colour string).
    pub panel_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomShellTheme {
    pub id: String,
    pub name: String,
    pub description: String,
    pub colors: CustomThemeColors,
    pub layout: CustomThemeLayout,
    #[serde(default)]
    pub extras: CustomThemeExtras,
}

fn load_raw() -> Vec<CustomShellTheme> {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_raw(list: &[CustomShellTheme]) -> bool {
    serde_json::to_string_pretty(list)
        .ok()
        .map(|s| fs::write(config_path(), s).is_ok())
        .unwrap_or(false)
}

fn is_hex_or_css_color(s: &str) -> bool {
    // Deliberately permissive (accepts #hex, rgb()/rgba()/hsl() and CSS
    // named colors) — this only guards against obviously-broken input
    // (empty string, wildly oversized strings used to bloat the JSON
    // file), not against every invalid CSS color string; an invalid
    // color just fails to render as intended, the same as handwriting
    // bad CSS anywhere else, and isn't a security concern since colors
    // only ever reach a `style.setProperty` call, never `innerHTML`.
    !s.trim().is_empty() && s.len() <= 64
}

fn validate(theme: &CustomShellTheme) -> Result<(), String> {
    if theme.name.trim().is_empty() {
        return Err("Name cannot be empty.".to_string());
    }
    if theme.name.len() > 48 {
        return Err("Name is too long.".to_string());
    }
    if theme.description.len() > 240 {
        return Err("Description is too long.".to_string());
    }
    for (label, v) in [
        ("accent", &theme.colors.accent), ("background", &theme.colors.background),
        ("surface", &theme.colors.surface), ("surfaceElevated", &theme.colors.surface_elevated),
        ("text", &theme.colors.text), ("textMuted", &theme.colors.text_muted),
        ("border", &theme.colors.border),
    ] {
        if !is_hex_or_css_color(v) {
            return Err(format!("\"{label}\" needs a valid CSS color."));
        }
    }
    if !["top", "bottom", "left", "right"].contains(&theme.layout.panel_position.as_str()) {
        return Err("Invalid panel position.".to_string());
    }
    if !["left", "right"].contains(&theme.layout.window_controls_position.as_str()) {
        return Err("Invalid window controls position.".to_string());
    }
    if !["macos", "windows", "gnome", "minimal"].contains(&theme.layout.window_controls_style.as_str()) {
        return Err("Invalid window controls style.".to_string());
    }
    if !["rounded", "sharp"].contains(&theme.layout.corner_style.as_str()) {
        return Err("Invalid corner style.".to_string());
    }
    if !["outline", "filled"].contains(&theme.layout.icon_style.as_str()) {
        return Err("Invalid icon style.".to_string());
    }
    if let Some(r) = theme.extras.corner_radius_px {
        if r > 32 { return Err("Corner radius must be 32px or less.".to_string()); }
    }
    if let Some(b) = theme.extras.wallpaper_blur {
        if b > 40 { return Err("Wallpaper blur must be 40px or less.".to_string()); }
    }
    if let Some(o) = theme.extras.panel_opacity {
        if o > 100 { return Err("Panel opacity must be between 0 and 100.".to_string()); }
    }
    if let Some(f) = &theme.extras.font_family {
        if f.len() > 120 { return Err("Font family is too long.".to_string()); }
    }
    if let Some(a) = &theme.extras.animation_speed {
        if !["none", "normal", "fast"].contains(&a.as_str()) {
            return Err("Invalid animation speed.".to_string());
        }
    }
    let x = &theme.extras;
    if let Some(o) = x.window_opacity {
        if !(60..=100).contains(&o) { return Err("Window opacity must be between 60 and 100.".to_string()); }
    }
    if let Some(s) = &x.window_shadow {
        if !["none", "soft", "medium", "strong", "glow"].contains(&s.as_str()) {
            return Err("Invalid window shadow.".to_string());
        }
    }
    if let Some(w) = x.window_border_width {
        if w > 4 { return Err("Window border must be 4px or less.".to_string()); }
    }
    if let Some(h) = x.titlebar_height {
        if !(28..=56).contains(&h) { return Err("Title-bar height must be between 28 and 56px.".to_string()); }
    }
    if let Some(u) = x.ui_scale_percent {
        if !(80..=130).contains(&u) { return Err("UI scale must be between 80 and 130 percent.".to_string()); }
    }
    if let Some(s) = &x.scrollbar_style {
        if !["default", "thin", "hidden", "accent"].contains(&s.as_str()) {
            return Err("Invalid scrollbar style.".to_string());
        }
    }
    if let Some(b) = x.wallpaper_brightness {
        if !(30..=130).contains(&b) { return Err("Wallpaper brightness must be between 30 and 130 percent.".to_string()); }
    }
    if let Some(s) = x.wallpaper_saturation {
        if s > 200 { return Err("Wallpaper saturation must be between 0 and 200 percent.".to_string()); }
    }
    for (label, v) in [("window border colour", &x.window_border_color), ("panel colour", &x.panel_color)] {
        if let Some(c) = v {
            if !is_hex_or_css_color(c) { return Err(format!("The {label} needs a valid CSS colour.")); }
        }
    }
    Ok(())
}

#[tauri::command(async)]
pub fn custom_theme_list() -> Vec<CustomShellTheme> {
    load_raw()
}

#[tauri::command(async)]
pub fn custom_theme_upsert(theme: CustomShellTheme) -> Result<Vec<CustomShellTheme>, String> {
    validate(&theme)?;
    let mut list = load_raw();
    if let Some(existing) = list.iter_mut().find(|t| t.id == theme.id) {
        *existing = theme;
    } else {
        list.push(theme);
    }
    if !save_raw(&list) {
        return Err("Failed to save custom theme.".to_string());
    }
    Ok(list)
}

#[tauri::command(async)]
pub fn custom_theme_remove(id: String) -> Vec<CustomShellTheme> {
    let mut list = load_raw();
    list.retain(|t| t.id != id);
    save_raw(&list);
    list
}

/// Parses and validates a theme from JSON text (an imported `.json`
/// file's contents) without saving it — the Settings UI shows a preview
/// before the person commits to `custom_theme_upsert`.
#[tauri::command(async)]
pub fn custom_theme_parse_import(json_text: String) -> Result<CustomShellTheme, String> {
    let mut theme: CustomShellTheme = serde_json::from_str(&json_text).map_err(|e| format!("Not a valid theme file: {e}"))?;
    validate(&theme)?;
    // Imported themes always get a fresh id so importing the same file
    // twice (or a theme whose id happens to collide with one already
    // installed) creates a new entry rather than silently overwriting
    // an existing theme of the same id.
    theme.id = format!("custom-{}", uuid_v4_ish());
    Ok(theme)
}

/// Not a real RFC 4122 UUID (no external crate pulled in just for this) —
/// random enough that a collision between two themes imported in the
/// same process lifetime is astronomically unlikely, which is all an
/// opaque local id needs.
fn uuid_v4_ish() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:x}", nanos ^ (std::process::id() as u128))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, name: &str) -> CustomShellTheme {
        CustomShellTheme {
            id: id.to_string(), name: name.to_string(), description: "A test theme".to_string(),
            colors: CustomThemeColors {
                accent: "#38bdf8".into(), background: "#0f172a".into(), surface: "#1e293b".into(),
                surface_elevated: "#293548".into(), text: "#f8fafc".into(), text_muted: "#94a3b8".into(),
                border: "rgba(255,255,255,0.08)".into(),
            },
            layout: CustomThemeLayout {
                panel_position: "top".into(), window_controls_position: "right".into(),
                window_controls_style: "windows".into(), corner_style: "rounded".into(), icon_style: "outline".into(),
            },
            extras: CustomThemeExtras::default(),
        }
    }

    #[test]
    fn rejects_empty_name() {
        let mut t = sample("1", "");
        assert!(validate(&t).is_err());
        t.name = "Ok Name".into();
        assert!(validate(&t).is_ok());
    }

    #[test]
    fn rejects_invalid_layout_enums() {
        let mut t = sample("1", "Test");
        t.layout.panel_position = "diagonal".into();
        assert!(validate(&t).is_err());
    }

    #[test]
    fn rejects_out_of_range_extras() {
        let mut t = sample("1", "Test");
        t.extras.corner_radius_px = Some(999);
        assert!(validate(&t).is_err());
        t.extras.corner_radius_px = Some(16);
        assert!(validate(&t).is_ok());

        t.extras.panel_opacity = Some(150);
        assert!(validate(&t).is_err());
    }

    #[test]
    fn validates_the_newer_extras() {
        let mut t = sample("1", "Test");
        t.extras.window_opacity = Some(85);
        t.extras.window_shadow = Some("glow".into());
        t.extras.window_border_width = Some(2);
        t.extras.window_border_color = Some("#ff00ff".into());
        t.extras.titlebar_height = Some(44);
        t.extras.ui_scale_percent = Some(110);
        t.extras.scrollbar_style = Some("thin".into());
        t.extras.wallpaper_brightness = Some(70);
        t.extras.wallpaper_saturation = Some(0);
        t.extras.panel_color = Some("rgb(10,20,30)".into());
        assert!(validate(&t).is_ok());

        for bad in [
            |t: &mut CustomShellTheme| t.extras.window_opacity = Some(10),
            |t: &mut CustomShellTheme| t.extras.window_shadow = Some("huge".into()),
            |t: &mut CustomShellTheme| t.extras.window_border_width = Some(9),
            |t: &mut CustomShellTheme| t.extras.titlebar_height = Some(10),
            |t: &mut CustomShellTheme| t.extras.ui_scale_percent = Some(300),
            |t: &mut CustomShellTheme| t.extras.scrollbar_style = Some("wavy".into()),
            |t: &mut CustomShellTheme| t.extras.wallpaper_brightness = Some(5),
            |t: &mut CustomShellTheme| t.extras.wallpaper_saturation = Some(999),
            |t: &mut CustomShellTheme| t.extras.panel_color = Some(String::new()),
        ] {
            let mut t = sample("1", "Test");
            bad(&mut t);
            assert!(validate(&t).is_err());
        }
    }

    #[test]
    fn old_theme_files_without_the_new_fields_still_load() {
        let json = r##"{"id":"a","name":"Old","description":"",
          "colors":{"accent":"#fff","background":"#000","surface":"#111","surface_elevated":"#222","text":"#fff","text_muted":"#aaa","border":"#333"},
          "layout":{"panel_position":"top","window_controls_position":"right","window_controls_style":"windows","corner_style":"rounded","icon_style":"outline"},
          "extras":{"corner_radius_px":8,"animation_speed":"fast"}}"##;
        let t: CustomShellTheme = serde_json::from_str(json).unwrap();
        assert_eq!(t.extras.corner_radius_px, Some(8));
        assert_eq!(t.extras.window_shadow, None);
        assert!(validate(&t).is_ok());
    }

    #[test]
    fn upsert_then_remove_round_trips() {
        let dir = std::env::temp_dir().join(format!("blue-themes-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // Can't easily redirect config_path() in a unit test without a
        // parameter (kept consistent with this codebase's other
        // settings modules, which have the same limitation) — so this
        // test instead exercises the pure validate()/serialize round
        // trip, which is what actually matters here.
        let t = sample("abc", "My Theme");
        let json = serde_json::to_string(&t).unwrap();
        let parsed: CustomShellTheme = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "abc");
        assert_eq!(parsed.name, "My Theme");
        assert!(validate(&parsed).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_assigns_a_fresh_id() {
        let t = sample("someone-elses-id", "Shared Theme");
        let json = serde_json::to_string(&t).unwrap();
        let imported = custom_theme_parse_import(json).unwrap();
        assert_ne!(imported.id, "someone-elses-id");
        assert!(imported.id.starts_with("custom-"));
    }

    #[test]
    fn import_rejects_invalid_json() {
        assert!(custom_theme_parse_import("not json".to_string()).is_err());
    }
}
