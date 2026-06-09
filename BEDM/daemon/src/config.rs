use hk_parser::{load_hk_file, parse_hk, resolve_interpolations, HkConfig};
use std::fs;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct BedmConfig {
    pub greeter_path: Option<String>,
    pub vt: Option<u8>,
    pub autologin_user: Option<String>,
    pub autologin_session: Option<String>,
    pub autologin_delay: Option<u64>,
    pub session_timeout: Option<u64>,
    pub theme: Option<String>,
    pub background: Option<String>,
    pub clock_format: Option<String>,
    pub show_user_list: Option<bool>,
    pub allow_root: Option<bool>,
    pub minimum_uid: Option<u32>,
    pub maximum_uid: Option<u32>,
    pub sessions_dir: Option<Vec<String>>,
    pub power: Option<PowerConfig>,
}

#[derive(Debug, Clone)]
pub struct PowerConfig {
    pub shutdown: Option<String>,
    pub reboot: Option<String>,
    pub suspend: Option<String>,
    pub hibernate: Option<String>,
}

impl Default for BedmConfig {
    fn default() -> Self {
        Self {
            greeter_path: Some("/usr/bin/bedm-greeter".to_string()),
            vt: Some(1),
            autologin_user: None,
            autologin_session: None,
            autologin_delay: Some(0),
            session_timeout: Some(0),
            theme: Some("blue".to_string()),
            background: None,
            clock_format: Some("%H:%M".to_string()),
            show_user_list: Some(true),
            allow_root: Some(false),
            minimum_uid: Some(1000),
            maximum_uid: Some(65533),
            sessions_dir: Some(vec![
                "/usr/share/wayland-sessions".to_string(),
                "/usr/share/xsessions".to_string(),
            ]),
            power: Some(PowerConfig::default()),
        }
    }
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            shutdown: Some("shutdown -h now".to_string()),
            reboot: Some("reboot".to_string()),
            suspend: Some("systemctl suspend".to_string()),
            hibernate: Some("systemctl hibernate".to_string()),
        }
    }
}

// ── Helper: get string from HkConfig section ──────────────────────────────

fn get_str(cfg: &HkConfig, section: &str, key: &str) -> Option<String> {
    let sec = cfg.get(section)?;
    let map = sec.as_map().ok()?;
    let val = map.get(key)?;
    val.as_string().ok()
}

fn get_bool(cfg: &HkConfig, section: &str, key: &str) -> Option<bool> {
    let sec = cfg.get(section)?;
    let map = sec.as_map().ok()?;
    let val = map.get(key)?;
    val.as_bool().ok()
}

fn get_u8(cfg: &HkConfig, section: &str, key: &str) -> Option<u8> {
    let sec = cfg.get(section)?;
    let map = sec.as_map().ok()?;
    let val = map.get(key)?;
    let n = val.as_number().ok()?;
    Some(n as u8)
}

fn get_u32(cfg: &HkConfig, section: &str, key: &str) -> Option<u32> {
    let sec = cfg.get(section)?;
    let map = sec.as_map().ok()?;
    let val = map.get(key)?;
    let n = val.as_number().ok()?;
    Some(n as u32)
}

fn get_u64(cfg: &HkConfig, section: &str, key: &str) -> Option<u64> {
    let sec = cfg.get(section)?;
    let map = sec.as_map().ok()?;
    let val = map.get(key)?;
    let n = val.as_number().ok()?;
    Some(n as u64)
}

fn get_str_array(cfg: &HkConfig, section: &str, key: &str) -> Option<Vec<String>> {
    let sec = cfg.get(section)?;
    let map = sec.as_map().ok()?;
    let val = map.get(key)?;
    let arr = val.as_array().ok()?;
    let result: Vec<String> = arr.iter()
        .filter_map(|v| v.as_string().ok())
        .collect();
    if result.is_empty() { None } else { Some(result) }
}

// ── Public API ─────────────────────────────────────────────────────────────

pub fn load_config(path: &str) -> Result<BedmConfig, String> {
    let mut cfg = load_hk_file(path)
        .map_err(|e| format!("Cannot load {}: {}", path, e))?;

    // Resolve ${...} interpolations including ${env:VAR}
    resolve_interpolations(&mut cfg)
        .map_err(|e| format!("Interpolation error in {}: {}", path, e))?;

    Ok(hk_to_config(&cfg))
}

pub fn load_config_str(content: &str) -> Result<BedmConfig, String> {
    let mut cfg = parse_hk(content)
        .map_err(|e| format!("Parse error: {}", e))?;
    resolve_interpolations(&mut cfg)
        .map_err(|e| format!("Interpolation error: {}", e))?;
    Ok(hk_to_config(&cfg))
}

fn hk_to_config(cfg: &HkConfig) -> BedmConfig {
    let defaults = BedmConfig::default();

    // [general] section
    let greeter_path = get_str(cfg, "general", "greeter_path")
        .or(defaults.greeter_path);
    let vt = get_u8(cfg, "general", "vt")
        .or(defaults.vt);
    let session_timeout = get_u64(cfg, "general", "session_timeout")
        .or(defaults.session_timeout);
    let theme = get_str(cfg, "general", "theme")
        .or(defaults.theme);
    let background = get_str(cfg, "general", "background");
    let clock_format = get_str(cfg, "general", "clock_format")
        .or(defaults.clock_format);
    let show_user_list = get_bool(cfg, "general", "show_user_list")
        .or(defaults.show_user_list);
    let allow_root = get_bool(cfg, "general", "allow_root")
        .or(defaults.allow_root);
    let minimum_uid = get_u32(cfg, "general", "minimum_uid")
        .or(defaults.minimum_uid);
    let maximum_uid = get_u32(cfg, "general", "maximum_uid")
        .or(defaults.maximum_uid);
    let sessions_dir = get_str_array(cfg, "general", "sessions_dir")
        .or(defaults.sessions_dir);

    // [autologin] section
    let autologin_user = get_str(cfg, "autologin", "user");
    let autologin_session = get_str(cfg, "autologin", "session");
    let autologin_delay = get_u64(cfg, "autologin", "delay")
        .or(defaults.autologin_delay);

    // [power] section
    let power = Some(PowerConfig {
        shutdown: get_str(cfg, "power", "shutdown")
            .or_else(|| defaults.power.as_ref().and_then(|p| p.shutdown.clone())),
        reboot: get_str(cfg, "power", "reboot")
            .or_else(|| defaults.power.as_ref().and_then(|p| p.reboot.clone())),
        suspend: get_str(cfg, "power", "suspend")
            .or_else(|| defaults.power.as_ref().and_then(|p| p.suspend.clone())),
        hibernate: get_str(cfg, "power", "hibernate")
            .or_else(|| defaults.power.as_ref().and_then(|p| p.hibernate.clone())),
    });

    BedmConfig {
        greeter_path,
        vt,
        autologin_user,
        autologin_session,
        autologin_delay,
        session_timeout,
        theme,
        background,
        clock_format,
        show_user_list,
        allow_root,
        minimum_uid,
        maximum_uid,
        sessions_dir,
        power,
    }
}

pub fn ensure_default_config() {
    let config_dir = "/etc/bedm";
    let config_path = "/etc/bedm/bedm.hk";

    if std::path::Path::new(config_path).exists() {
        return;
    }

    let _ = fs::create_dir_all(config_dir);
    let content = default_config_content();
    if let Err(e) = fs::write(config_path, content) {
        warn!("Could not write default config: {}", e);
    }
}

pub fn default_config_content() -> &'static str {
    r#"! /etc/bedm/bedm.hk — BEDM Display Manager Configuration
! Blue Environment Display Manager v1.0.0
! Format: HackerOS .hk (github.com/HackerOS-Linux-System)

[general]
-> greeter_path  => /usr/bin/bedm-greeter
-> vt            => 1
-> theme         => blue
-> show_user_list => true
-> allow_root    => false
-> minimum_uid   => 1000
-> maximum_uid   => 65533
-> clock_format  => %H:%M
-> sessions_dir  => ["/usr/share/wayland-sessions", "/usr/share/xsessions", "/usr/local/share/wayland-sessions"]

! Uncomment and set to enable autologin:
! [autologin]
! -> user    => username
! -> session => blue-environment
! -> delay   => 0

[power]
-> shutdown  => shutdown -h now
-> reboot    => reboot
-> suspend   => systemctl suspend
-> hibernate => systemctl hibernate
"#
}
