// BEDM configuration — parses /etc/bedm/bedm.toml

use serde::{Deserialize, Serialize};
use std::fs;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedmConfig {
    /// Path to greeter binary (default: /usr/bin/bedm-greeter)
    pub greeter_path: Option<String>,

    /// VT number to use (default: 1)
    pub vt: Option<u8>,

    /// Autologin user (if set, skip greeter)
    pub autologin_user: Option<String>,

    /// Autologin session (default: blue-environment)
    pub autologin_session: Option<String>,

    /// Autologin delay in seconds (default: 0)
    pub autologin_delay: Option<u64>,

    /// Session timeout in seconds (0 = disabled)
    pub session_timeout: Option<u64>,

    /// Theme for greeter: "blue" | "dark" | "light"
    pub theme: Option<String>,

    /// Background image path
    pub background: Option<String>,

    /// Clock format (default: "%H:%M")
    pub clock_format: Option<String>,

    /// Show user list (default: true)
    pub show_user_list: Option<bool>,

    /// Allow root login (default: false)
    pub allow_root: Option<bool>,

    /// Minimum UID for user listing (default: 1000)
    pub minimum_uid: Option<u32>,

    /// Maximum UID for user listing (default: 65533)
    pub maximum_uid: Option<u32>,

    /// Sessions directory (default: /usr/share/wayland-sessions:/usr/share/xsessions)
    pub sessions_dir: Option<Vec<String>>,

    /// Power commands
    pub power: Option<PowerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            power: Some(PowerConfig {
                shutdown: Some("shutdown -h now".to_string()),
                reboot: Some("reboot".to_string()),
                suspend: Some("systemctl suspend".to_string()),
                hibernate: Some("systemctl hibernate".to_string()),
            }),
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

pub fn load_config(path: &str) -> Result<BedmConfig, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {}", path, e))?;

    toml::from_str(&content).map_err(|e| format!("Config parse error: {}", e))
}

pub fn ensure_default_config() {
    let config_dir = "/etc/bedm";
    let config_path = "/etc/bedm/bedm.toml";

    if std::path::Path::new(config_path).exists() {
        return;
    }

    let _ = fs::create_dir_all(config_dir);
    let default = include_str!("../../../config/bedm.toml");
    if let Err(e) = fs::write(config_path, default) {
        warn!("Could not write default config: {}", e);
    }
}
