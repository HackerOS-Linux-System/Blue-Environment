use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE_NAME: &str = "wayfire.ini";
const MARKER: &str = "blue-environment";
const AUTOSTART_KEY: &str = "blue_environment";

fn default_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
}

/// Unlike labwc/sway (a directory holding several files), wayfire keeps
/// everything in one `.ini` file directly under the config dir — `custom`
/// is that directory either way, for a consistent `*_config_dir` meaning
/// across all three backends in `config.hk`.
pub fn user_config_path(custom_dir: Option<&Path>) -> PathBuf {
    custom_dir.map(|d| d.join(CONFIG_FILE_NAME)).unwrap_or_else(|| default_dir().join(CONFIG_FILE_NAME))
}

fn system_config_paths() -> Vec<PathBuf> {
    vec![PathBuf::from("/etc/wayfire.ini"), PathBuf::from("/etc/xdg/wayfire.ini")]
}

const DEFAULT_CONFIG: &str = "\
# Blue Environment — wayfire configuration (generated default)
#
# Used when Blue Environment runs on the wayfire backend
# (~/.config/Blue-Environment/config.hk  ->  [backend] compositor => wayfire).
# Blue Environment only generates this file when it doesn't exist yet, so
# edit it freely — it will never be overwritten. Delete it to get a fresh
# default back. wayfire re-reads most of this only on the next login; a few
# [input] values apply live.

[core]
plugins = autostart command decoration foreign-toplevel place window-rules wm-actions ipc
preferred_decoration_mode = client

[autostart]
# This is the line that actually starts the Blue shell. Blue looks for the
# \"blue_environment\" key specifically (see backend/wayfire_config.rs) —
# rename it and Blue will add a second entry next time rather than finding
# this one.
blue_environment = @CHILD_CMD@

[input]
xkb_layout = @XKB_LAYOUT@
cursor_theme = default
cursor_size = 24
kb_repeat_delay = 400
kb_repeat_rate = 40

[decoration]
active_color = #1e293bee
inactive_color = #1e293b99
border_size = 1

[command]
binding_terminal = <super> KEY_T | <ctrl> <alt> KEY_T
command_terminal = @CTL@ open-terminal
binding_start_menu = <super> KEY_SPACE
command_start_menu = @CTL@ toggle-start-menu
binding_fullscreen_menu = <super> KEY_TAB
command_fullscreen_menu = @CTL@ fullscreen-menu
binding_control_center = <ctrl> <alt> KEY_C
command_control_center = @CTL@ toggle-control-center
binding_clipboard = <ctrl> <shift> KEY_V
command_clipboard = @CTL@ toggle-clipboard
binding_screenshot = KEY_PRINT
command_screenshot = @CTL@ screenshot
binding_lock = <super> KEY_L
command_lock = @CTL@ lock
binding_show_desktop = <super> KEY_D
command_show_desktop = @CTL@ show-desktop
binding_volume_up = KEY_VOLUMEUP
command_volume_up = pactl set-sink-volume @DEFAULT_SINK@ +5%
binding_volume_down = KEY_VOLUMEDOWN
command_volume_down = pactl set-sink-volume @DEFAULT_SINK@ -5%
binding_mute = KEY_MUTE
command_mute = pactl set-sink-mute @DEFAULT_SINK@ toggle

[switcher]
next_view = <alt> KEY_TAB
prev_view = <alt> <shift> KEY_TAB

[wm-actions]
toggle_fullscreen = <super> KEY_UP
minimize = <super> KEY_DOWN
";

#[derive(Debug, Clone)]
struct Params {
    ctl_command: String,
    xkb_layout: String,
}

impl Default for Params {
    fn default() -> Self {
        Params { ctl_command: "blue-environment".into(), xkb_layout: "us".into() }
    }
}

fn params_from_user_settings() -> Params {
    let mut p = Params::default();
    if let Ok(exe) = std::env::current_exe() {
        p.ctl_command = exe.to_string_lossy().to_string();
    }
    let path = dirs::home_dir().unwrap_or_default().join(".config/Blue-Environment/settings.json");
    if let Ok(text) = fs::read_to_string(path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(lang) = v.get("language").and_then(|l| l.as_str()).filter(|l| !l.is_empty()) {
                p.xkb_layout = super::labwc_config::layout_for_language(lang);
            }
        }
    }
    p
}

fn render_default(p: &Params, child_cmd: &str) -> String {
    DEFAULT_CONFIG
        .replace("@CHILD_CMD@", child_cmd)
        // Every `command_X = @CTL@ <subcommand>` line needs the actual
        // `--ctl` flag before the subcommand (see backend::shell_ipc) —
        // baked in here once rather than repeated in every binding line.
        .replace("@CTL@", &format!("{} --ctl", p.ctl_command))
        .replace("@XKB_LAYOUT@", &p.xkb_layout)
}

fn is_blue(path: &Path) -> bool {
    fs::read_to_string(path).map(|t| t.contains(MARKER)).unwrap_or(false)
}

fn has_autostart_entry(text: &str) -> bool {
    in_section(text, "autostart").any(|line| {
        line.split_once('=').map(|(k, v)| k.trim() == AUTOSTART_KEY && v.contains(super::ARG_WAYFIRE_CHILD)).unwrap_or(false)
    })
}

/// Iterates the raw (untrimmed-of-comments) lines that fall inside
/// `[section]`, stopping at the next `[...]` header.
fn in_section<'a>(text: &'a str, section: &'a str) -> impl Iterator<Item = &'a str> {
    let mut in_section = false;
    text.lines().filter(move |raw| {
        let line = raw.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_section = name.eq_ignore_ascii_case(section);
            return false;
        }
        in_section && !line.is_empty() && !line.starts_with('#')
    })
}

/// Inserts `key = value` under `[section]` in `text`, creating the section
/// if it doesn't exist yet. If `key` is already set anywhere in that
/// section, its value is replaced in place instead of adding a duplicate.
/// Every other line is left byte-for-byte untouched.
fn upsert_ini_key(text: &str, section: &str, key: &str, value: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_target = false;
    let mut section_found = false;
    let mut done = false;
    let mut lines = text.lines().peekable();
    while let Some(raw) = lines.next() {
        let trimmed = raw.trim();
        if let Some(name) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            // Leaving the target section without having found the key:
            // insert it right before this next header.
            if in_target && !done {
                out.push(format!("{key} = {value}"));
                done = true;
            }
            in_target = name.eq_ignore_ascii_case(section);
            section_found |= in_target;
            out.push(raw.to_string());
            continue;
        }
        if in_target && !done {
            if let Some((k, _)) = trimmed.split_once('=') {
                if k.trim() == key {
                    out.push(format!("{key} = {value}"));
                    done = true;
                    continue;
                }
            }
        }
        out.push(raw.to_string());
    }
    if in_target && !done {
        out.push(format!("{key} = {value}"));
    }
    if !section_found {
        if out.last().map(|l| !l.trim().is_empty()).unwrap_or(false) {
            out.push(String::new());
        }
        out.push(format!("[{section}]"));
        out.push(format!("{key} = {value}"));
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// Sets a single `key = value` under `[section]` in the wayfire.ini at
/// `path`, preserving everything else. Used for live-ish settings changes
/// (keyboard layout, cursor theme) — see `backend::set_keyboard_layout`.
pub fn set_ini_value(path: &Path, section: &str, key: &str, value: &str) -> Result<(), String> {
    if value.contains('\n') || value.contains('\r') {
        return Err("invalid value".into());
    }
    let text = fs::read_to_string(path).unwrap_or_default();
    let updated = upsert_ini_key(&text, section, key, value);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, updated).map_err(|e| e.to_string())
}

pub struct StartResult {
    pub launch_config_path: PathBuf,
    pub generated_files: Vec<PathBuf>,
}

/// Finds an existing wayfire.ini (user's `config_dir` first, then wayfire's
/// own default locations), and returns a config path safe to hand to
/// `wayfire -c` that is guaranteed to start the Blue shell.
pub fn ensure_startable_config(custom_dir: Option<&Path>, generate: bool, child_cmd: &str) -> Result<StartResult, String> {
    let own_path = user_config_path(custom_dir);
    let mut generated = Vec::new();

    // Same reasoning as sway_config: a system-wide path only counts as
    // "existing" when it was actually prepared for Blue, not when it's
    // merely the wayfire package's own stock default.
    let existing = if own_path.is_file() {
        Some(own_path.clone())
    } else if custom_dir.is_none() {
        system_config_paths().into_iter().find(|p| is_blue(p))
    } else {
        None
    };

    if let Some(existing) = existing {
        let text = fs::read_to_string(&existing).map_err(|e| format!("{}: {e}", existing.display()))?;
        if has_autostart_entry(&text) {
            return Ok(StartResult { launch_config_path: existing, generated_files: generated });
        }
        // Minimal, targeted, idempotent edit — see the module docs for why
        // this is the one exception to "never touch an existing config".
        let updated = upsert_ini_key(&text, "autostart", AUTOSTART_KEY, child_cmd);
        fs::write(&existing, updated).map_err(|e| e.to_string())?;
        return Ok(StartResult { launch_config_path: existing, generated_files: generated });
    }

    if !generate {
        return Err(format!("no wayfire config found at {} and generation is disabled", own_path.display()));
    }
    let params = params_from_user_settings();
    if let Some(dir) = own_path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&own_path, render_default(&params, child_cmd)).map_err(|e| e.to_string())?;
    generated.push(own_path.clone());
    Ok(StartResult { launch_config_path: own_path, generated_files: generated })
}

pub fn has_prepared_config(custom_dir: Option<&Path>) -> bool {
    let own = user_config_path(custom_dir);
    if is_blue(&own) {
        return true;
    }
    custom_dir.is_none() && system_config_paths().iter().any(|p| is_blue(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_default_when_nothing_exists() {
        let dir = std::env::temp_dir().join(format!("blue-wf-test-a-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let r = ensure_startable_config(Some(&dir), true, "'/x/blue-environment' --wayfire-child").unwrap();
        assert_eq!(r.launch_config_path, dir.join("wayfire.ini"));
        assert_eq!(r.generated_files, vec![dir.join("wayfire.ini")]);
        let text = fs::read_to_string(&r.launch_config_path).unwrap();
        assert!(text.contains("blue_environment = '/x/blue-environment' --wayfire-child"));
        assert!(text.contains("foreign-toplevel"));
        assert!(text.contains("@DEFAULT_SINK@"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Same bug class as sway's `set $ctl` test: every `command_X` binding
    /// must call the binary with the real `--ctl` flag, not just the bare
    /// subcommand.
    #[test]
    fn command_bindings_use_the_real_ctl_flag() {
        let dir = std::env::temp_dir().join(format!("blue-wf-test-e-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let r = ensure_startable_config(Some(&dir), true, "'/x/blue-environment' --wayfire-child").unwrap();
        let text = fs::read_to_string(&r.launch_config_path).unwrap();
        for line in text.lines().filter(|l| l.trim_start().starts_with("command_") && l.contains("/x/blue-environment")) {
            assert!(line.contains("--ctl "), "command binding missing --ctl: {line}");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn adds_autostart_entry_to_existing_ini_without_touching_anything_else() {
        let dir = std::env::temp_dir().join(format!("blue-wf-test-b-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let original = "# my wayfire setup\n[core]\nplugins = foreign-toplevel expo\n\n[input]\nxkb_layout = de\n";
        fs::write(dir.join("wayfire.ini"), original).unwrap();

        let r = ensure_startable_config(Some(&dir), true, "'/x/blue-environment' --wayfire-child").unwrap();
        assert_eq!(r.launch_config_path, dir.join("wayfire.ini"));
        assert!(r.generated_files.is_empty());

        let updated = fs::read_to_string(&r.launch_config_path).unwrap();
        assert!(updated.contains("plugins = foreign-toplevel expo"));
        assert!(updated.contains("xkb_layout = de"));
        assert!(updated.contains("[autostart]\nblue_environment = '/x/blue-environment' --wayfire-child"));

        // Idempotent: running it again must not add a second entry.
        let r2 = ensure_startable_config(Some(&dir), true, "'/x/blue-environment' --wayfire-child").unwrap();
        let text2 = fs::read_to_string(&r2.launch_config_path).unwrap();
        assert_eq!(text2.matches("blue_environment").count(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn upsert_ini_key_replaces_in_place_or_appends() {
        let text = "[input]\nxkb_layout = us\ncursor_size = 24\n";
        let out = upsert_ini_key(text, "input", "xkb_layout", "pl");
        assert_eq!(out, "[input]\nxkb_layout = pl\ncursor_size = 24\n");

        let out = upsert_ini_key(text, "input", "cursor_theme", "Breeze");
        assert_eq!(out, "[input]\nxkb_layout = us\ncursor_size = 24\ncursor_theme = Breeze\n");

        let out = upsert_ini_key("[core]\nplugins = a\n", "input", "xkb_layout", "pl");
        assert_eq!(out, "[core]\nplugins = a\n\n[input]\nxkb_layout = pl\n");
    }

    #[test]
    fn set_ini_value_rejects_newlines() {
        let path = std::env::temp_dir().join("blue-wf-should-not-exist.ini");
        assert!(set_ini_value(&path, "input", "xkb_layout", "pl\nevil").is_err());
    }
}
