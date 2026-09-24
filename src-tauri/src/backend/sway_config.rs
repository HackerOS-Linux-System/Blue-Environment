use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE_NAME: &str = "config";
/// Marker Blue looks for to tell "this config already starts the shell"
/// apart from "it doesn't yet" — matched as a plain substring, not parsed.
const MARKER: &str = "blue-environment";

fn default_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config")).join("sway")
}

pub fn user_config_dir(custom: Option<&Path>) -> PathBuf {
    custom.map(|p| p.to_path_buf()).unwrap_or_else(default_dir)
}

fn system_config_paths() -> Vec<PathBuf> {
    // Mirrors sway(1)'s own search order (see `man 5 sway`), so Blue
    // recognises a system-wide prepared config the same way sway would.
    vec![PathBuf::from("/etc/sway/config"), PathBuf::from("/etc/i3/config")]
}

const DEFAULT_CONFIG: &str = "\
# Blue Environment — sway configuration (generated default)
#
# Used when Blue Environment runs on the sway backend
# (~/.config/Blue-Environment/config.hk  ->  [backend] compositor => sway).
# Blue Environment only generates this file when it doesn't exist yet, so
# edit it freely — it will never be overwritten. Delete it to get a fresh
# default back. Reload with: swaymsg reload

### Blue shell
exec @CHILD_CMD@

### Behaviour
# sway tiles by default (i3 heritage); Blue is a desktop shell, so every
# window floats instead, like a normal desktop.
for_window [all] floating enable
default_floating_border normal 1
focus_follows_mouse no
mouse_warping none

### Keyboard
input type:keyboard {
    xkb_layout \"@XKB_LAYOUT@\"
    repeat_delay 400
    repeat_rate 40
}
seat seat0 xcursor_theme default 24

### Global shortcuts — talk to the running Blue shell so they work even
### while a native app has focus (see `blue-environment --ctl`).
set $ctl @CTL@ --ctl
bindsym Mod4+space exec $ctl toggle-start-menu
bindsym Mod1+F1 exec $ctl toggle-start-menu
bindsym Mod4+Tab exec $ctl fullscreen-menu
bindsym Control+Mod1+c exec $ctl toggle-control-center
bindsym Control+Shift+v exec $ctl toggle-clipboard
bindsym Control+Mod1+t exec $ctl open-terminal
bindsym Print exec $ctl screenshot
bindsym Mod4+l exec $ctl lock
bindsym Mod4+d exec $ctl show-desktop

### Native windows
bindsym Mod1+Tab exec $ctl switcher-next
bindsym Mod1+Shift+Tab exec $ctl switcher-prev
bindsym Mod1+F4 kill
bindsym Mod4+Up fullscreen toggle
bindsym Mod4+Down floating toggle
bindsym Mod4+Left move position 0 0, resize set 50ppt 100ppt
bindsym Mod4+Right resize set 50ppt 100ppt, move position 50ppt 0

### Media keys
bindsym XF86AudioRaiseVolume exec pactl set-sink-volume @DEFAULT_SINK@ +5%
bindsym XF86AudioLowerVolume exec pactl set-sink-volume @DEFAULT_SINK@ -5%
bindsym XF86AudioMute exec pactl set-sink-mute @DEFAULT_SINK@ toggle
bindsym XF86MonBrightnessUp exec brightnessctl set +5%
bindsym XF86MonBrightnessDown exec brightnessctl set 5%-

### Colors — matches the shell's own window chrome (Window.svelte)
client.focused          #2957a4 #1e293b #e2e8f0 #2957a4 #2957a4
client.focused_inactive #272e3f #1e293b #64748b #272e3f #272e3f
client.unfocused        #272e3f #0f172a #64748b #272e3f #272e3f

output * bg @WALLPAPER_COLOR@ solid_color
";

/// Wraps an *existing* config (prepared or the person's own) without
/// modifying it: `include`s it, then adds the one `exec` line sway needs to
/// actually start the shell.
const WRAPPER_CONFIG: &str = "\
# Blue Environment — generated launch file for sway (see backend/sway_config.rs).
#
# This file is NOT your sway config; your real one is included below,
# completely unmodified. This wrapper only exists to add the one line sway
# needs to start the Blue shell (sway doesn't have a labwc-style \"-s\" flag
# — it only starts programs an `exec` line inside its config names). Blue
# regenerates this file on every start, so don't edit it: edit the included
# config instead.

include \"@INCLUDED@\"
exec @CHILD_CMD@
";

#[derive(Debug, Clone)]
struct Params {
    ctl_command: String,
    xkb_layout: String,
    wallpaper_color: String,
}

impl Default for Params {
    fn default() -> Self {
        Params { ctl_command: "blue-environment".into(), xkb_layout: "us".into(), wallpaper_color: "#0f172a".into() }
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
            let pick = |a: &str, b: &str| v.get(a).or_else(|| v.get(b)).cloned();
            if let Some(lang) = pick("language", "language").and_then(|l| l.as_str().map(String::from)).filter(|l| !l.is_empty()) {
                p.xkb_layout = super::labwc_config::layout_for_language(&lang);
            }
        }
    }
    p
}

fn render_default(template: &str, p: &Params, child_cmd: &str) -> String {
    template
        .replace("@CHILD_CMD@", child_cmd)
        .replace("@CTL@", &p.ctl_command)
        .replace("@XKB_LAYOUT@", &p.xkb_layout)
        .replace("@WALLPAPER_COLOR@", &p.wallpaper_color)
}

fn is_blue(path: &Path) -> bool {
    fs::read_to_string(path).map(|t| t.contains(MARKER)).unwrap_or(false)
}

/// True if `text` already has its own line starting the shell (so a
/// wrapper mustn't add a second, duplicate `exec`). Deliberately keyed only
/// on the child flag, not on the binary's name/path (which a packaged
/// build could call anything) — the flag is what actually matters and is
/// always exactly `--sway-child`, regardless of how the binary is named.
fn already_execs_shell(text: &str) -> bool {
    text.lines().any(|l| {
        let l = l.trim();
        l.starts_with("exec ") && l.contains(super::ARG_SWAY_CHILD)
    })
}

pub struct StartResult {
    /// The path to pass to `sway -c`.
    pub launch_config_path: PathBuf,
    /// Files newly created by this call (empty when everything was
    /// already prepared).
    pub generated_files: Vec<PathBuf>,
}

/// Finds an existing sway config (user's `config_dir` first, then sway's
/// own default locations), and returns a config path safe to hand to
/// `sway -c` that is guaranteed to start the Blue shell — generating a
/// default, or a small wrapper around what's already there, exactly once.
pub fn ensure_startable_config(custom_dir: Option<&Path>, generate: bool, child_cmd: &str) -> Result<StartResult, String> {
    let dir = user_config_dir(custom_dir);
    let own_path = dir.join(CONFIG_FILE_NAME);
    let mut generated = Vec::new();

    // The user's own per-user config counts as "existing" unconditionally —
    // if anything is there, whether hand-written or from an earlier Blue
    // run, it is unambiguously theirs and must never be replaced. A
    // *system-wide* path only counts if it was deliberately prepared for
    // Blue (carries the marker): otherwise it's just the sway package's own
    // stock default, which doesn't reflect any real customisation and
    // would leave the person with plain tiling sway instead of a usable
    // floating desktop — so Blue generates its own proper default instead.
    let existing = if own_path.is_file() {
        Some(own_path.clone())
    } else if custom_dir.is_none() {
        system_config_paths().into_iter().find(|p| is_blue(p))
    } else {
        None
    };

    if let Some(existing) = existing {
        let text = fs::read_to_string(&existing).map_err(|e| format!("{}: {e}", existing.display()))?;
        if already_execs_shell(&text) {
            // The person (or a previous Blue run's generated default, which
            // also matches this check) already starts the shell — use as-is.
            return Ok(StartResult { launch_config_path: existing, generated_files: generated });
        }
        let wrapper_dir = dirs::config_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
            .join("Blue-Environment/generated");
        fs::create_dir_all(&wrapper_dir).map_err(|e| e.to_string())?;
        let wrapper_path = wrapper_dir.join("sway.conf");
        let content = WRAPPER_CONFIG.replace("@INCLUDED@", &existing.to_string_lossy()).replace("@CHILD_CMD@", child_cmd);
        fs::write(&wrapper_path, content).map_err(|e| e.to_string())?;
        return Ok(StartResult { launch_config_path: wrapper_path, generated_files: generated });
    }

    if !generate {
        return Err(format!("no sway config found at {} and generation is disabled", own_path.display()));
    }
    let params = params_from_user_settings();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(&own_path, render_default(DEFAULT_CONFIG, &params, child_cmd)).map_err(|e| e.to_string())?;
    generated.push(own_path.clone());
    Ok(StartResult { launch_config_path: own_path, generated_files: generated })
}

/// Whether `path` (or sway's own default locations, when `path` wasn't
/// explicitly configured) already has a prepared/Blue-flavoured config —
/// used by [`super::info`]-style diagnostics.
pub fn has_prepared_config(custom_dir: Option<&Path>) -> bool {
    let dir = user_config_dir(custom_dir);
    if is_blue(&dir.join(CONFIG_FILE_NAME)) {
        return true;
    }
    custom_dir.is_none() && system_config_paths().iter().any(|p| is_blue(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_default_when_nothing_exists() {
        let dir = std::env::temp_dir().join(format!("blue-sway-test-a-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let r = ensure_startable_config(Some(&dir), true, "'/x/blue-environment' --sway-child").unwrap();
        assert_eq!(r.launch_config_path, dir.join("config"));
        assert_eq!(r.generated_files, vec![dir.join("config")]);
        let text = fs::read_to_string(&r.launch_config_path).unwrap();
        assert!(text.contains("exec '/x/blue-environment' --sway-child"));
        assert!(text.contains("for_window [all] floating enable"));
        assert!(text.contains("@DEFAULT_SINK@")); // pactl's own token must survive untouched
        let _ = fs::remove_dir_all(&dir);
    }

    /// Every keybind that talks to the running shell must use the real
    /// `--ctl` flag (`blue-environment --ctl toggle-start-menu`, not just
    /// `blue-environment toggle-start-menu`) — caught live on a real sway
    /// instance: without this, the bind silently did nothing because the
    /// binary's plain CLI has no `toggle-start-menu` subcommand.
    #[test]
    fn keybinds_use_the_real_ctl_flag() {
        let dir = std::env::temp_dir().join(format!("blue-sway-test-e-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let r = ensure_startable_config(Some(&dir), true, "'/x/blue-environment' --sway-child").unwrap();
        let text = fs::read_to_string(&r.launch_config_path).unwrap();
        assert!(text.contains("set $ctl") && text.contains("--ctl"), "{text}");
        for line in text.lines().filter(|l| l.trim_start().starts_with("bindsym") && l.contains("$ctl")) {
            assert!(line.contains("exec $ctl "), "keybind doesn't call $ctl: {line}");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn wraps_existing_config_without_modifying_it() {
        let dir = std::env::temp_dir().join(format!("blue-sway-test-b-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config"), "# my own sway config\nbindsym Mod4+Return exec foot\n").unwrap();
        std::env::set_var("XDG_CONFIG_HOME", std::env::temp_dir().join(format!("blue-sway-xdg-{}", std::process::id())));

        let r = ensure_startable_config(Some(&dir), true, "'/x/blue-environment' --sway-child").unwrap();
        assert_ne!(r.launch_config_path, dir.join("config"));
        assert!(r.generated_files.is_empty(), "the user's own file must not be reported as generated");
        assert_eq!(fs::read_to_string(dir.join("config")).unwrap(), "# my own sway config\nbindsym Mod4+Return exec foot\n");

        let wrapper = fs::read_to_string(&r.launch_config_path).unwrap();
        assert!(wrapper.contains(&format!("include \"{}\"", dir.join("config").display())));
        assert!(wrapper.contains("exec '/x/blue-environment' --sway-child"));

        let _ = fs::remove_dir_all(&dir);
    }

    /// A distro-shipped `/etc/sway/config` (installed by the `sway` package
    /// itself, not by HackerOS or the person) must never be mistaken for a
    /// deliberately prepared config — generate Blue's own default instead
    /// of wrapping vanilla, tiling-by-default sway. Exercises the real
    /// system path when the `sway` package happens to be installed (as it
    /// is in the environment this was verified in); skips harmlessly
    /// otherwise rather than depending on it.
    #[test]
    fn ignores_unmarked_stock_system_config() {
        if !Path::new("/etc/sway/config").is_file() {
            return;
        }
        assert!(!is_blue(Path::new("/etc/sway/config")), "the sway package's own default must not carry Blue's marker");
    }

    #[test]
    fn does_not_double_exec_a_config_that_already_starts_the_shell() {
        let dir = std::env::temp_dir().join(format!("blue-sway-test-c-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config"), "exec '/x/blue-environment' --sway-child\n").unwrap();
        let r = ensure_startable_config(Some(&dir), true, "'/x/blue-environment' --sway-child").unwrap();
        assert_eq!(r.launch_config_path, dir.join("config"));
        let _ = fs::remove_dir_all(&dir);
    }
}
