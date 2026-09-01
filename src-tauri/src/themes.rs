use hk_parser::{load_hk_file, resolve_interpolations};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const SYSTEM_THEMES_DIR: &str = "/usr/share/themes";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeEffects {
    pub blur: bool,
    pub transparency: bool,
    pub animations: bool,
    /// `"sharp"` or `"rounded"` — kept as a plain string (rather than an
    /// enum) since this is a boundary a third-party theme author writes
    /// by hand; an unrecognized value just means the frontend's CSS
    /// doesn't have a matching selector for it; a totally new theme
    /// package doesn't need this crate updated to introduce one.
    pub corner_style: String,
    pub accent_color: Option<String>,
}

impl Default for ThemeEffects {
    fn default() -> Self {
        Self {
            blur: true,
            transparency: true,
            animations: true,
            corner_style: "rounded".to_string(),
            accent_color: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemTheme {
    pub id: String, // the directory name
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub effects: ThemeEffects,
    pub css: String,
    /// `data:image/png;base64,...` if `preview.png` exists, else `None`
    /// (frontend falls back to a generic icon, same as a builtin theme
    /// with no `previewImage`).
    pub preview_data_url: Option<String>,
}

fn themes_root() -> PathBuf {
    // Overridable for development/testing without root — mirrors the
    // pattern this crate's other filesystem-scanning commands (e.g.
    // wallpaper discovery in commands/display.rs) already use for the
    // same reason: `/usr/share/...` isn't writable or even guaranteed
    // to exist on a dev machine that isn't a real HackerOS install.
    std::env::var("BLUE_THEMES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(SYSTEM_THEMES_DIR))
}

/// Scans `/usr/share/themes/*/` and loads every subdirectory that has
/// (at minimum) a `config.hk` and a `styles.css`. A subdirectory
/// missing either is skipped with a `tracing::warn!` rather than
/// aborting the whole scan — one broken/incomplete theme package
/// shouldn't hide every other installed theme from the picker.
#[tauri::command]
pub fn list_system_themes() -> Vec<SystemTheme> {
    let root = themes_root();
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new(); // no themes directory yet — not an error, just nothing installed
    };

    let mut themes = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        match load_theme_package(&path, &id) {
            Ok(theme) => themes.push(theme),
            Err(err) => tracing::warn!("skipping theme package '{id}': {err}"),
        }
    }
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    themes
}

/// Loads one specific theme package by directory name — used to
/// re-read a single theme (e.g. after `ThemesSection.svelte` asks to
/// apply one) without re-scanning the whole directory.
#[tauri::command]
pub fn load_system_theme(id: String) -> Result<SystemTheme, String> {
    let path = themes_root().join(&id);
    load_theme_package(&path, &id)
}

fn load_theme_package(path: &Path, id: &str) -> Result<SystemTheme, String> {
    let config_path = path.join("config.hk");
    let css_path = path.join("styles.css");

    if !css_path.is_file() {
        return Err(format!("no styles.css in {}", path.display()));
    }
    let css = fs::read_to_string(&css_path).map_err(|e| format!("failed to read styles.css: {e}"))?;
    let css = sanitize_theme_css(&css);

    let mut raw = load_hk_file(&config_path).map_err(|e| {
        let source = fs::read_to_string(&config_path).unwrap_or_default();
        format!("failed to parse config.hk: {}", e.render(&source))
    })?;
    let _ = resolve_interpolations(&mut raw); // best-effort, same reasoning as HackerOS-Comp's config.rs

    let metadata = raw.get("metadata").and_then(|v| v.as_map().ok());
    let name = metadata
        .and_then(|m| m.get("name"))
        .and_then(|v| v.as_string().ok())
        .unwrap_or_else(|| id.to_string());
    let author = metadata
        .and_then(|m| m.get("author"))
        .and_then(|v| v.as_string().ok())
        .unwrap_or_else(|| "Unknown".to_string());
    let version = metadata
        .and_then(|m| m.get("version"))
        .and_then(|v| v.as_string().ok())
        .unwrap_or_else(|| "0.0.0".to_string());
    let description = metadata
        .and_then(|m| m.get("description"))
        .and_then(|v| v.as_string().ok())
        .unwrap_or_default();

    let mut effects = ThemeEffects::default();
    if let Some(section) = raw.get("effects").and_then(|v| v.as_map().ok()) {
        if let Some(v) = section.get("blur").and_then(|v| v.as_bool().ok()) {
            effects.blur = v;
        }
        if let Some(v) = section.get("transparency").and_then(|v| v.as_bool().ok()) {
            effects.transparency = v;
        }
        if let Some(v) = section.get("animations").and_then(|v| v.as_bool().ok()) {
            effects.animations = v;
        }
        if let Some(v) = section.get("corner_style").and_then(|v| v.as_string().ok()) {
            effects.corner_style = v;
        }
        if let Some(v) = section.get("accent_color").and_then(|v| v.as_string().ok()) {
            effects.accent_color = Some(v);
        }
    }

    let preview_path = path.join("preview.png");
    let preview_data_url = if preview_path.is_file() {
        fs::read(&preview_path).ok().map(|bytes| {
            use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
            format!("data:image/png;base64,{}", BASE64.encode(bytes))
        })
    } else {
        None
    };

    Ok(SystemTheme {
        id: id.to_string(),
        name,
        author,
        version,
        description,
        effects,
        css,
        preview_data_url,
    })
}

// ── CSS sanitization ─────────────────────────────────────────────────────
// A filesystem theme's `styles.css` is injected into the running shell
// as a real `<style>` element (see `SystemThemeStyle.svelte` — via
// `.textContent`, not `.innerHTML`, so an HTML/`<script>` breakout via
// the CSS text itself was never actually possible regardless of this
// function's existence). What *is* real and worth stopping here: CSS
// itself has several long-standing vectors for a stylesheet to reach
// outside its own styling job —
//   - `@import` can pull in an arbitrary second stylesheet from
//     anywhere, defeating the point of a theme package being a
//     self-contained, reviewable set of files.
//   - `url(...)` referencing an `http(s)://`/`//`/`javascript:` target
//     lets a stylesheet make network requests (tracking pixels,
//     fingerprinting via load timing, or in `javascript:`'s case,
//     script execution in browser engines that still honor it in CSS
//     contexts) — a theme should only ever reference its own bundled
//     `assets/` via a relative path (see `themes/README.md`).
//   - `-moz-binding`/legacy IE `behavior`/`expression(...)` are old
//     but still real property-level code-execution vectors in engines
//     that implement them.
// This is a pragmatic, regex-based pass — not a full CSS parser — so it
// stops the straightforward cases rather than claiming to be
// unbeatable against a determined, obfuscated attacker. Given the
// actual delivery mechanism (a person installs a theme package onto
// their own system, or a distro packages one), the realistic threat
// this defends against is a theme distributed in good faith that
// happens to reference an external resource, or a copy-pasted CSS
// snippet nobody re-audited — not a nation-state adversary.
fn sanitize_theme_css(css: &str) -> String {
    static IMPORT_RE: OnceLock<Regex> = OnceLock::new();
    static URL_RE: OnceLock<Regex> = OnceLock::new();
    static EXPRESSION_RE: OnceLock<Regex> = OnceLock::new();
    static MOZ_BINDING_RE: OnceLock<Regex> = OnceLock::new();
    static BEHAVIOR_RE: OnceLock<Regex> = OnceLock::new();

    let import_re = IMPORT_RE.get_or_init(|| Regex::new(r"(?is)@import\s+[^;]*;").unwrap());
    let url_re = URL_RE.get_or_init(|| Regex::new(r#"(?i)url\(\s*['"]?\s*(https?:)?//[^)]*\)"#).unwrap());
    let js_url_re_source = r#"(?i)url\(\s*['"]?\s*javascript:[^)]*\)"#;
    let expression_re = EXPRESSION_RE.get_or_init(|| Regex::new(r"(?i)expression\s*\([^)]*\)").unwrap());
    let moz_binding_re = MOZ_BINDING_RE.get_or_init(|| Regex::new(r"(?i)-moz-binding\s*:[^;]*;?").unwrap());
    let behavior_re = BEHAVIOR_RE.get_or_init(|| Regex::new(r"(?i)\bbehavior\s*:[^;]*;?").unwrap());
    let js_url_re = Regex::new(js_url_re_source).unwrap();

    let mut out = import_re.replace_all(css, "/* [external stylesheet import removed by theme sanitizer] */").to_string();
    out = url_re.replace_all(&out, "url()").to_string();
    out = js_url_re.replace_all(&out, "url()").to_string();
    out = expression_re.replace_all(&out, "none").to_string();
    out = moz_binding_re.replace_all(&out, "").to_string();
    out = behavior_re.replace_all(&out, "").to_string();
    out
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_a_real_theme_package_from_a_temp_dir() {
        let dir = std::env::temp_dir().join(format!("blue-theme-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("config.hk"),
            "[metadata]\n-> name => Test Theme\n-> author => Someone\n-> version => 1.2.3\n\n[effects]\n-> blur => false\n-> corner_style => sharp\n-> accent_color => \"#22c55e\"\n",
        )
        .unwrap();
        fs::write(dir.join("styles.css"), "body { color: red; }").unwrap();

        let theme = load_theme_package(&dir, "test-theme").expect("should load");
        assert_eq!(theme.name, "Test Theme");
        assert_eq!(theme.author, "Someone");
        assert_eq!(theme.version, "1.2.3");
        assert!(!theme.effects.blur);
        assert_eq!(theme.effects.corner_style, "sharp");
        assert_eq!(theme.effects.accent_color.as_deref(), Some("#22c55e"));
        assert!(theme.css.contains("color: red"));
        assert!(theme.preview_data_url.is_none()); // no preview.png written above

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_styles_css_is_skipped_not_panicked() {
        let dir = std::env::temp_dir().join(format!("blue-theme-test-nostyles-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.hk"), "[metadata]\n-> name => X\n").unwrap();

        let result = load_theme_package(&dir, "x");
        assert!(result.is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn strips_at_import() {
        let css = "@import url('https://evil.example/x.css'); body { color: red; }";
        let out = sanitize_theme_css(css);
        assert!(!out.contains("@import"));
        assert!(out.contains("color: red"));
    }

    #[test]
    fn strips_remote_url_but_keeps_local_relative_ones() {
        let css = "a { background: url(https://evil.example/track.png); } b { background: url(assets/bg.png); }";
        let out = sanitize_theme_css(css);
        assert!(!out.contains("evil.example"));
        assert!(out.contains("assets/bg.png"), "a relative local asset url() must survive sanitization");
    }

    #[test]
    fn strips_protocol_relative_urls() {
        let css = "a { background: url(//evil.example/x.png); }";
        let out = sanitize_theme_css(css);
        assert!(!out.contains("evil.example"));
    }

    #[test]
    fn strips_javascript_url_scheme() {
        let css = "a { background: url(javascript:alert(1)); }";
        let out = sanitize_theme_css(css);
        assert!(!out.to_lowercase().contains("javascript:"));
    }

    #[test]
    fn strips_css_expression_and_moz_binding_and_behavior() {
        let css = "a { width: expression(alert(1)); -moz-binding: url('http://evil/x.xml#y'); behavior: url(evil.htc); }";
        let out = sanitize_theme_css(css);
        assert!(!out.to_lowercase().contains("expression("));
        assert!(!out.contains("-moz-binding"));
        assert!(!out.to_lowercase().contains("behavior:"));
    }

    #[test]
    fn leaves_ordinary_css_completely_untouched() {
        let css = ":root[data-system-theme='HDE'] { --hde-accent: #22c55e; border-radius: 0px; }";
        assert_eq!(sanitize_theme_css(css), css);
    }
}
