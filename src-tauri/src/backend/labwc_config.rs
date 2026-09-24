use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const RC_XML: &str = include_str!("../../resources/labwc/rc.xml");
const MENU_XML: &str = include_str!("../../resources/labwc/menu.xml");
const AUTOSTART: &str = include_str!("../../resources/labwc/autostart");
const ENVIRONMENT: &str = include_str!("../../resources/labwc/environment");
const THEMERC: &str = include_str!("../../resources/labwc/themerc");

pub const THEME_NAME: &str = "Blue-Environment";

/// What actually got written — returned for logging/tests.
#[derive(Debug, Default)]
pub struct Generated {
    pub files: Vec<PathBuf>,
}

/// Values substituted into the templates.
#[derive(Debug, Clone)]
pub struct Params {
    /// Command used by keybinds to reach the Blue shell.
    pub ctl_command: String,
    pub margin_top: u32,
    pub margin_bottom: u32,
    pub xkb_layout: String,
}

impl Default for Params {
    fn default() -> Self {
        Params { ctl_command: "blue-environment".into(), margin_top: 48, margin_bottom: 0, xkb_layout: "us".into() }
    }
}

/// Reads `panelPosition`/`panelSize`/`language` from Blue's own
/// `settings.json` (if present) so the generated margin matches the
/// person's real top-bar geometry.
pub fn params_from_user_settings() -> Params {
    let mut p = Params::default();
    if let Ok(exe) = std::env::current_exe() {
        // Absolute path — keybinds must work even if PATH is minimal.
        p.ctl_command = exe.to_string_lossy().to_string();
    }
    let path = dirs::home_dir().unwrap_or_default().join(".config/Blue-Environment/settings.json");
    if let Ok(text) = fs::read_to_string(path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            // The shell persists camelCase keys; the skeleton shipped with
            // HackerOS uses snake_case (where 0/false/"" just mean "unset").
            let pick = |a: &str, b: &str| v.get(a).or_else(|| v.get(b)).cloned();
            let size = pick("panelSize", "panel_size")
                .and_then(|s| s.as_u64())
                .filter(|s| *s > 0 && *s < 400)
                .unwrap_or(48) as u32;
            // Only the camelCase key is authoritative for "panel disabled".
            let enabled = v.get("panelEnabled").and_then(|e| e.as_bool()).unwrap_or(true);
            let bottom = pick("panelPosition", "panel_position").and_then(|s| s.as_str().map(String::from)).as_deref() == Some("bottom");
            let size = if enabled { size } else { 0 };
            p.margin_top = if bottom { 0 } else { size };
            p.margin_bottom = if bottom { size } else { 0 };
            if let Some(lang) = v.get("language").and_then(|l| l.as_str()).filter(|l| !l.is_empty()) {
                p.xkb_layout = layout_for_language(lang);
            }
        }
    }
    p
}

/// Maps a UI language code (`pl`, `de-DE`, …) to an XKB layout name.
/// Shared with `sway_config`/`wayfire_config` — all three backends need the
/// same mapping when reading Blue's own `settings.json`.
pub(crate) fn layout_for_language(lang: &str) -> String {
    let code = lang.split(['-', '_']).next().unwrap_or("en").to_lowercase();
    match code.as_str() {
        "en" | "" => "us".into(),
        other => other.to_string(),
    }
}

pub fn render(template: &str, p: &Params) -> String {
    template
        .replace("@CTL@", &p.ctl_command)
        .replace("@MARGIN_TOP@", &p.margin_top.to_string())
        .replace("@MARGIN_BOTTOM@", &p.margin_bottom.to_string())
        .replace("@XKB_LAYOUT@", &p.xkb_layout)
}

pub fn user_config_dir(custom: Option<&Path>) -> PathBuf {
    custom.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        dirs::config_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config")).join("labwc")
    })
}

fn system_config_dirs() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("/etc/xdg/labwc")];
    if let Ok(dirs) = std::env::var("XDG_CONFIG_DIRS") {
        for d in dirs.split(':').filter(|d| !d.is_empty()) {
            let p = PathBuf::from(d).join("labwc");
            if !v.contains(&p) {
                v.push(p);
            }
        }
    }
    v
}

fn theme_installed(name: &str) -> bool {
    let mut roots = vec![PathBuf::from("/usr/share/themes"), PathBuf::from("/usr/local/share/themes")];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".local/share/themes"));
        roots.push(home.join(".themes"));
    }
    roots.iter().any(|r| r.join(name).join("labwc/themerc").exists() || r.join(name).join("openbox-3/themerc").exists())
}

/// Writes `content` only if `path` does not exist yet (atomic w.r.t. races
/// with another instance thanks to `create_new`).
fn write_new(path: &Path, content: &str, executable: bool, out: &mut Generated) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut f) => {
            if f.write_all(content.as_bytes()).is_ok() {
                #[cfg(unix)]
                if executable {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
                }
                #[cfg(not(unix))]
                let _ = executable;
                out.files.push(path.to_path_buf());
            }
        }
        Err(_) => {} // already exists (or unwritable) — leave it alone
    }
}

/// Whether a usable labwc config (the person's own, or a previous Blue
/// run's generated default) is already present — used by [`super::info`]
/// diagnostics; mirrors the same `present()` check `ensure_default_config`
/// uses to decide what (if anything) it needs to generate.
pub fn has_prepared_config(custom_dir: Option<&Path>) -> bool {
    let user = user_config_dir(custom_dir);
    let system = if custom_dir.is_some() { Vec::new() } else { system_config_dirs() };
    let is_blue = |p: &Path| fs::read_to_string(p).map(|t| t.contains("Blue Environment") || t.contains("Blue-Environment")).unwrap_or(false);
    user.join("rc.xml").exists() || system.iter().any(|d| is_blue(&d.join("rc.xml")))
}

/// Makes sure a usable labwc configuration exists. Returns the list of
/// files it had to create (empty when everything was already prepared).
pub fn ensure_default_config(custom_dir: Option<&Path>) -> Generated {
    let mut out = Generated::default();
    let user = user_config_dir(custom_dir);
    let system = if custom_dir.is_some() { Vec::new() } else { system_config_dirs() };
    let params = params_from_user_settings();

    let is_blue = |p: &Path| fs::read_to_string(p).map(|t| t.contains("Blue Environment") || t.contains("Blue-Environment")).unwrap_or(false);
    let present = |name: &str| user.join(name).exists() || system.iter().any(|d| is_blue(&d.join(name)));

    if !present("rc.xml") {
        write_new(&user.join("rc.xml"), &render(RC_XML, &params), false, &mut out);
    }
    if !present("menu.xml") {
        write_new(&user.join("menu.xml"), &render(MENU_XML, &params), false, &mut out);
    }
    if !present("environment") {
        write_new(&user.join("environment"), &render(ENVIRONMENT, &params), false, &mut out);
    }
    if !present("autostart") {
        write_new(&user.join("autostart"), &render(AUTOSTART, &params), true, &mut out);
    }
    if !theme_installed(THEME_NAME) {
        let base = dirs::data_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".local/share"))
            .join("themes")
            .join(THEME_NAME);
        // labwc >= 0.8 reads `labwc/`, but 0.7.x only looks in the legacy
        // Openbox-compatible `openbox-3/` directory (verified against
        // labwc 0.7.1) — ship the same file in both.
        write_new(&base.join("labwc/themerc"), THEMERC, false, &mut out);
        write_new(&base.join("openbox-3/themerc"), THEMERC, false, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_replaces_all_placeholders() {
        let p = Params { ctl_command: "/opt/blue".into(), margin_top: 40, margin_bottom: 0, xkb_layout: "pl".into() };
        let rc = render(RC_XML, &p);
        assert!(rc.contains("/opt/blue --ctl toggle-start-menu"));
        assert!(rc.contains("top=\"40\""));
        assert!(!rc.contains("@CTL@") && !rc.contains("@MARGIN_TOP@") && !rc.contains("@XKB_LAYOUT@"));
        // pactl's own @DEFAULT_SINK@ token must survive untouched.
        assert!(rc.contains("@DEFAULT_SINK@"));
        assert!(render(ENVIRONMENT, &p).contains("XKB_DEFAULT_LAYOUT=pl"));
    }

    #[test]
    fn never_overwrites_existing_files() {
        let dir = std::env::temp_dir().join(format!("blue-labwc-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("rc.xml"), "MINE").unwrap();
        let g = ensure_default_config(Some(&dir));
        assert_eq!(fs::read_to_string(dir.join("rc.xml")).unwrap(), "MINE");
        assert!(dir.join("menu.xml").exists());
        assert!(g.files.iter().all(|f| f.file_name().unwrap() != "rc.xml"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// `--` inside an XML comment makes labwc reject the whole rc.xml
    /// (this exact bug shipped once in a draft) — guard against it.
    #[test]
    fn xml_templates_have_valid_comments() {
        for (name, tpl) in [("rc.xml", RC_XML), ("menu.xml", MENU_XML)] {
            let mut rest = tpl;
            while let Some(start) = rest.find("<!--") {
                let after = &rest[start + 4..];
                let end = after.find("-->").unwrap_or_else(|| panic!("{name}: unterminated comment"));
                assert!(!after[..end].contains("--"), "{name}: '--' inside an XML comment");
                rest = &after[end + 3..];
            }
        }
    }

    #[test]
    fn language_maps_to_layout() {
        assert_eq!(layout_for_language("en"), "us");
        assert_eq!(layout_for_language("pl"), "pl");
        assert_eq!(layout_for_language("de-DE"), "de");
    }
}
