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

/// Marker comment written around Blue's merged-in keybind block, so a
/// re-run of [`install_keybinds_into_existing`] is idempotent (it checks
/// for this marker before touching the file again) instead of
/// duplicating the block on every call.
const MERGED_KEYBINDS_MARKER_START: &str = "<!-- BLUE-ENVIRONMENT:MERGED-KEYBINDS:START (safe to remove this block) -->";
const MERGED_KEYBINDS_MARKER_END: &str = "<!-- BLUE-ENVIRONMENT:MERGED-KEYBINDS:END -->";

/// The subset of `RC_XML`'s `<keybind>` entries that Blue actually needs
/// to function as a shell layered on top of an existing labwc session —
/// notably including the Alt-Tab entries, which `<query
/// identifier="*blue*environment*"/>` makes safe to merge into someone
/// else's config unchanged (it hands the key to labwc's own
/// `NextWindow`/`PreviousWindow` unless the Blue shell itself is
/// focused, so it never fights with a user's own window-cycling setup
/// for their native apps — see the comment above these two binds in
/// `rc.xml`). Deliberately a small, additive, easy-to-eyeball set rather
/// than the full generated file: this path exists for someone who
/// already has and wants to keep their own labwc config, not to replace
/// it.
fn keybinds_snippet(params: &Params) -> String {
    let block = r#"    <keybind key="W-space"><action name="Execute" command="@CTL@ --ctl toggle-start-menu"/></keybind>
    <keybind key="A-Tab">
      <action name="If">
        <query identifier="*blue*environment*"/>
        <then><action name="Execute" command="@CTL@ --ctl switcher-next"/></then>
        <else><action name="NextWindow"/></else>
      </action>
    </keybind>
    <keybind key="A-S-Tab">
      <action name="If">
        <query identifier="*blue*environment*"/>
        <then><action name="Execute" command="@CTL@ --ctl switcher-prev"/></then>
        <else><action name="PreviousWindow"/></else>
      </action>
    </keybind>
    <keybind key="Print"><action name="Execute" command="@CTL@ --ctl screenshot"/></keybind>
    <keybind key="W-l"><action name="Execute" command="@CTL@ --ctl lock"/></keybind>
    <keybind key="W-d"><action name="Execute" command="@CTL@ --ctl show-desktop"/></keybind>"#;
    format!("{MERGED_KEYBINDS_MARKER_START}\n{}\n{MERGED_KEYBINDS_MARKER_END}", render(block, params))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IntegrationStatus {
    pub config_path: String,
    pub config_exists: bool,
    /// True if the file is one Blue generated itself (see `is_blue`
    /// above) OR already has the merged-keybinds marker — either way,
    /// there is nothing to repair.
    pub keybinds_present: bool,
}

/// Reports whether the *current* user's `rc.xml` already has what Blue
/// needs, without changing anything — what a "Repair labwc Integration"
/// button in Settings checks before showing itself as actionable. See
/// `install_keybinds_into_existing`'s doc comment for why this needs to
/// exist as its own step distinct from `ensure_default_config`.
pub fn check_integration(custom_dir: Option<&Path>) -> IntegrationStatus {
    let path = user_config_dir(custom_dir).join("rc.xml");
    let text = fs::read_to_string(&path).unwrap_or_default();
    let keybinds_present = text.contains("Blue Environment")
        || text.contains("Blue-Environment")
        || text.contains(MERGED_KEYBINDS_MARKER_START);
    IntegrationStatus {
        config_path: path.to_string_lossy().to_string(),
        config_exists: path.exists(),
        keybinds_present,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallReport {
    pub changed: bool,
    pub backup_path: Option<String>,
    pub message: String,
}

/// Merges Blue's required keybinds into the user's *existing* `rc.xml`,
/// rather than only ever writing a config from scratch.
///
/// WHY THIS EXISTS: `ensure_default_config` (above) only ever calls
/// `write_new`, which does nothing if `rc.xml` already exists — by
/// design, so Blue never clobbers a hand-tuned config. But that design
/// means someone who already ran labwc before installing Blue
/// Environment (an extremely common on-ramp — labwc as a daily-driver
/// WM, Blue layered on top later) ends up with an `rc.xml` that has
/// none of Blue's keybinds: no Alt-Tab hookup, no Super-key start menu,
/// nothing. Every symptom of that ("I click an app and nothing
/// happens", "Alt-Tab does nothing") looks identical to the backend
/// misdetection bug fixed in `active()` above, but has a completely
/// different fix — better backend *detection* can't inject keybindings
/// into a config file Blue never touched. This is the other half of the
/// real fix: an explicit, user-triggered, idempotent, backed-up merge.
///
/// - If `rc.xml` doesn't exist at all, delegates to
///   `ensure_default_config` instead (nothing to merge into).
/// - If it exists and already has the marker (or looks like a
///   Blue-generated file already), does nothing and reports
///   `changed: false`.
/// - Otherwise: copies the current file to `rc.xml.bak-<unix-seconds>`,
///   then inserts the keybinds block right before the first
///   `</keyboard>` it finds. If there's no `<keyboard>` section at all
///   (a very stripped-down custom config), inserts a whole new
///   `<keyboard>…</keyboard>` section right before `</labwc_config>`
///   instead, so this still works even then.
pub fn install_keybinds_into_existing(custom_dir: Option<&Path>) -> Result<InstallReport, String> {
    let path = user_config_dir(custom_dir).join("rc.xml");
    if !path.exists() {
        let generated = ensure_default_config(custom_dir);
        return Ok(InstallReport {
            changed: !generated.files.is_empty(),
            backup_path: None,
            message: "No existing rc.xml — generated Blue's default config instead.".to_string(),
        });
    }

    let original = fs::read_to_string(&path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    if original.contains(MERGED_KEYBINDS_MARKER_START) || original.contains("Blue Environment") || original.contains("Blue-Environment") {
        return Ok(InstallReport {
            changed: false,
            backup_path: None,
            message: "Blue's keybinds are already present in rc.xml — nothing to do.".to_string(),
        });
    }

    let params = params_from_user_settings();
    let snippet = keybinds_snippet(&params);

    let merged = if let Some(idx) = original.find("</keyboard>") {
        let mut s = original.clone();
        s.insert_str(idx, &format!("{snippet}\n  "));
        s
    } else if let Some(idx) = original.find("</labwc_config>") {
        let mut s = original.clone();
        s.insert_str(idx, &format!("  <keyboard>\n{snippet}\n  </keyboard>\n"));
        s
    } else {
        return Err("rc.xml doesn't look like a labwc config (no </keyboard> or </labwc_config> found) — not touching it.".to_string());
    };

    let backup_path = path.with_extension(format!("xml.bak-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)));
    fs::copy(&path, &backup_path).map_err(|e| format!("could not create backup at {}: {e}", backup_path.display()))?;
    fs::write(&path, &merged).map_err(|e| format!("could not write {}: {e}", path.display()))?;

    Ok(InstallReport {
        changed: true,
        backup_path: Some(backup_path.to_string_lossy().to_string()),
        message: "Merged Blue's keybinds into your existing rc.xml. Reload labwc's config (or log out/in) for it to take effect.".to_string(),
    })
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

    fn temp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("blue-labwc-merge-{}-{}", std::process::id(), suffix));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn check_integration_reports_missing_config() {
        let dir = temp_dir("missing");
        let status = check_integration(Some(&dir));
        assert!(!status.config_exists);
        assert!(!status.keybinds_present);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merges_keybinds_into_a_pre_existing_user_rc_xml() {
        let dir = temp_dir("existing");
        let users_own_config = "<?xml version=\"1.0\"?>\n<labwc_config>\n  <keyboard>\n    <keybind key=\"W-e\"><action name=\"Execute\" command=\"my-file-manager\"/></keybind>\n  </keyboard>\n</labwc_config>\n";
        fs::write(dir.join("rc.xml"), users_own_config).unwrap();

        let before = check_integration(Some(&dir));
        assert!(before.config_exists);
        assert!(!before.keybinds_present, "a plain user config shouldn't look like it already has Blue's keybinds");

        let report = install_keybinds_into_existing(Some(&dir)).unwrap();
        assert!(report.changed);
        assert!(report.backup_path.is_some());

        let merged = fs::read_to_string(dir.join("rc.xml")).unwrap();
        // The user's own keybind must survive untouched.
        assert!(merged.contains("my-file-manager"));
        // Blue's Alt-Tab and start-menu binds must now be present.
        assert!(merged.contains("switcher-next"));
        assert!(merged.contains("toggle-start-menu"));
        assert!(merged.contains(MERGED_KEYBINDS_MARKER_START));

        // The backup must contain exactly the original content.
        let backup_path = PathBuf::from(report.backup_path.unwrap());
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), users_own_config);

        let after = check_integration(Some(&dir));
        assert!(after.keybinds_present);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merging_twice_is_idempotent() {
        let dir = temp_dir("idempotent");
        fs::write(dir.join("rc.xml"), "<?xml version=\"1.0\"?>\n<labwc_config>\n  <keyboard>\n  </keyboard>\n</labwc_config>\n").unwrap();

        let first = install_keybinds_into_existing(Some(&dir)).unwrap();
        assert!(first.changed);
        let second = install_keybinds_into_existing(Some(&dir)).unwrap();
        assert!(!second.changed, "running the merge again once the marker is present must be a no-op");

        // Only one copy of the marker should exist, not two.
        let content = fs::read_to_string(dir.join("rc.xml")).unwrap();
        assert_eq!(content.matches(MERGED_KEYBINDS_MARKER_START).count(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn does_nothing_to_a_config_already_generated_by_blue() {
        let dir = temp_dir("already-blue");
        let params = Params::default();
        fs::write(dir.join("rc.xml"), render(RC_XML, &params)).unwrap();

        let status = check_integration(Some(&dir));
        assert!(status.keybinds_present, "Blue's own generated rc.xml should already read as having Blue's keybinds");

        let report = install_keybinds_into_existing(Some(&dir)).unwrap();
        assert!(!report.changed);
        assert!(report.backup_path.is_none(), "must not create a needless backup when nothing needs to change");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_rc_xml_falls_back_to_generating_a_default_one() {
        let dir = temp_dir("no-rc-xml-at-all");
        let report = install_keybinds_into_existing(Some(&dir)).unwrap();
        assert!(report.changed);
        assert!(dir.join("rc.xml").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
