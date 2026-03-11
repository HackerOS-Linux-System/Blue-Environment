use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

/// Rendering backend preference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RenderBackend {
    Auto,
    Vulkan,
    OpenGL,
}

impl Default for RenderBackend {
    fn default() -> Self {
        Self::Auto
    }
}

/// Panel position
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PanelPosition {
    Top,
    Bottom,
}

impl Default for PanelPosition {
    fn default() -> Self {
        Self::Top
    }
}

/// Main Blue Environment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueConfig {
    // Rendering
    pub render_backend: RenderBackend,

    // Appearance
    pub theme: String,
    pub wallpaper: String,
    pub panel_height: u32,
    pub panel_position: PanelPosition,

    // Effects
    pub enable_blur: bool,
    pub blur_radius: f32,
    pub enable_shadows: bool,
    pub enable_animations: bool,
    pub window_corner_radius: f32,
    pub animation_speed: f64,

    // Display
    pub scale_factor: f64,

    // Workspaces
    pub workspace_count: usize,

    // Keyboard
    pub keyboard_layout: String,
    pub keyboard_repeat_delay: u32,
    pub keyboard_repeat_rate: u32,
}

impl Default for BlueConfig {
    fn default() -> Self {
        Self {
            render_backend: RenderBackend::Auto,
            theme: "Default".to_string(),
            wallpaper: "/usr/share/wallpapers/default.png".to_string(),
            panel_height: 42,
            panel_position: PanelPosition::Top,
            enable_blur: true,
            blur_radius: 20.0,
            enable_shadows: true,
            enable_animations: true,
            window_corner_radius: 10.0,
            animation_speed: 1.0,
            scale_factor: 1.0,
            workspace_count: 4,
            keyboard_layout: "us".to_string(),
            keyboard_repeat_delay: 200,
            keyboard_repeat_rate: 25,
        }
    }
}

impl BlueConfig {
    /// Returns the XDG config directory path
    pub fn config_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        PathBuf::from(home).join(".config").join("blue-environment")
    }

    /// Returns the path to the main config file
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Load config from disk, or create default if not present
    pub fn load_or_default() -> Self {
        let path = Self::config_path();

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<Self>(&content) {
                    Ok(cfg) => {
                        info!("Loaded config from {:?}", path);
                        return cfg;
                    }
                    Err(e) => {
                        warn!("Failed to parse config {:?}: {}, using defaults", path, e);
                    }
                },
                Err(e) => {
                    warn!("Failed to read config {:?}: {}, using defaults", path, e);
                }
            }
        }

        let cfg = Self::default();
        // Try to save defaults
        let _ = cfg.save();
        cfg
    }

    /// Save config to disk
    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;

        let content = toml::to_string_pretty(self)?;
        let path = Self::config_path();
        std::fs::write(&path, &content)?;
        info!("Config saved to {:?}", path);
        Ok(())
    }

    /// Update a single field and save
    pub fn set_wallpaper(&mut self, path: &str) -> Result<()> {
        self.wallpaper = path.to_string();
        self.save()
    }

    pub fn set_theme(&mut self, theme: &str) -> Result<()> {
        self.theme = theme.to_string();
        self.save()
    }

    pub fn set_blur(&mut self, enabled: bool) -> Result<()> {
        self.enable_blur = enabled;
        self.save()
    }

    pub fn set_blur_radius(&mut self, radius: f32) -> Result<()> {
        self.blur_radius = radius;
        self.save()
    }

    pub fn set_shadows(&mut self, enabled: bool) -> Result<()> {
        self.enable_shadows = enabled;
        self.save()
    }

    pub fn set_animations(&mut self, enabled: bool) -> Result<()> {
        self.enable_animations = enabled;
        self.save()
    }

    pub fn set_corner_radius(&mut self, r: f32) -> Result<()> {
        self.window_corner_radius = r;
        self.save()
    }

    pub fn set_scale(&mut self, scale: f64) -> Result<()> {
        self.scale_factor = scale;
        self.save()
    }
}
