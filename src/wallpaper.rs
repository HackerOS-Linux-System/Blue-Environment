use image::DynamicImage;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

const WALLPAPER_DIRS: &[&str] = &[
    "/usr/share/wallpapers/",
"/usr/share/backgrounds/",
"/usr/local/share/wallpapers/",
];
const SUPPORTED_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp"];

#[derive(Debug, Clone)]
pub struct WallpaperEntry {
    pub path: PathBuf,
    pub name: String,
    pub width: u32,
    pub height: u32,
}

pub struct WallpaperManager {
    pub current_path: String,
    pub entries: Vec<WallpaperEntry>,
    pub current_image: Option<DynamicImage>,
}

impl WallpaperManager {
    pub fn new(initial: &str) -> Self {
        let mut mgr = Self {
            current_path: initial.to_string(),
            entries: Vec::new(),
            current_image: None,
        };
        mgr.scan();
        mgr.load(initial);
        mgr
    }

    /// Scan wallpaper directories for supported image files
    pub fn scan(&mut self) {
        self.entries.clear();

        // Also include user wallpapers
        let home = std::env::var("HOME").unwrap_or_default();
        let user_dir = format!("{}/.local/share/wallpapers/", home);

        let mut dirs: Vec<&str> = WALLPAPER_DIRS.to_vec();
        let user_dir_ref = user_dir.as_str();
        dirs.push(user_dir_ref);

        for dir in dirs {
            for entry in WalkDir::new(dir)
                .max_depth(3)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                {
                    let path = entry.path();
                    let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();

                    if !SUPPORTED_EXT.contains(&ext.as_str()) {
                        continue;
                    }

                    let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Wallpaper")
                    .replace(['-', '_'], " ")
                    .split_whitespace()
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            None => String::new(),
                         Some(f) => {
                             f.to_uppercase().collect::<String>() + c.as_str()
                         }
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                    let (w, h) = image::image_dimensions(path).unwrap_or((1920, 1080));

                    self.entries.push(WallpaperEntry {
                        path: path.to_owned(),
                                      name,
                                      width: w,
                                      height: h,
                    });
                }
        }

        debug!("Found {} wallpapers", self.entries.len());
    }

    /// Load a wallpaper image into memory
    pub fn load(&mut self, path: &str) -> bool {
        // Try exact path first
        if Path::new(path).exists() {
            match image::open(path) {
                Ok(img) => {
                    info!("Loaded wallpaper: {} ({}x{})", path, img.width(), img.height());
                    self.current_path = path.to_string();
                    self.current_image = Some(img);
                    return true;
                }
                Err(e) => warn!("Failed to load wallpaper {}: {}", path, e),
            }
        }

        // Fallbacks
        let fallbacks = [
            "/usr/share/wallpapers/default.png",
            "/usr/share/wallpapers/default.jpg",
            "/usr/share/backgrounds/default.png",
        ];

        for fb in &fallbacks {
            if Path::new(fb).exists() {
                if let Ok(img) = image::open(fb) {
                    info!("Using fallback wallpaper: {}", fb);
                    self.current_path = fb.to_string();
                    self.current_image = Some(img);
                    return true;
                }
            }
        }

        // Last resort: first scanned wallpaper
        if let Some(first) = self.entries.first().map(|e| e.path.clone()) {
            if let Ok(img) = image::open(&first) {
                self.current_path = first.to_string_lossy().to_string();
                self.current_image = Some(img);
                return true;
            }
        }

        warn!("No wallpaper found, compositor will render solid color");
        false
    }

    /// Set and load a new wallpaper
    pub fn set(&mut self, path: &str) -> bool {
        self.load(path)
    }

    /// Get RGBA pixel data for rendering
    pub fn rgba_data(&self) -> Option<(Vec<u8>, u32, u32)> {
        self.current_image.as_ref().map(|img| {
            let rgba = img.to_rgba8();
            let w = rgba.width();
            let h = rgba.height();
            (rgba.into_raw(), w, h)
        })
    }

    /// List all wallpapers as (name, path) pairs for the settings UI
    pub fn list(&self) -> Vec<(String, String)> {
        self.entries
        .iter()
        .map(|e| (e.name.clone(), e.path.to_string_lossy().to_string()))
        .collect()
    }
}
