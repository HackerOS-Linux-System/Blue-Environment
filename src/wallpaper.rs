use image::DynamicImage;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

const DIRS: &[&str] = &[
    "/usr/share/wallpapers/",
"/usr/share/backgrounds/",
"/usr/local/share/wallpapers/",
"/usr/local/share/backgrounds/",
// KDE
"/usr/share/wallpapers/Next/contents/images/",
"/usr/share/wallpapers/Breeze/contents/images/",
// GNOME / common distros
"/usr/share/gnome-background-properties/",
"/usr/share/pixmaps/",
];
const EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp"];

#[derive(Debug, Clone)]
pub struct WallpaperEntry {
    pub path: PathBuf,
    pub name: String,
    pub width: u32,
    pub height: u32,
}

pub struct WallpaperManager {
    pub current: String,
    pub entries: Vec<WallpaperEntry>,
    pub image: Option<DynamicImage>,
}

impl WallpaperManager {
    pub fn new(initial: &str) -> Self {
        let mut m = Self {
            current: initial.to_string(),
            entries: Vec::new(),
            image: None,
        };
        m.scan();
        m.load(initial);
        m
    }

    pub fn scan(&mut self) {
        self.entries.clear();
        let home = std::env::var("HOME").unwrap_or_default();
        let user_dir = format!("{}/.local/share/wallpapers/", home);

        let mut dirs: Vec<&str> = DIRS.to_vec();
        dirs.push(&user_dir);

        for dir in dirs {
            for e in WalkDir::new(dir)
                .max_depth(3)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                {
                    let path = e.path();
                    let ext = path
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_lowercase())
                    .unwrap_or_default();
                    if !EXTS.contains(&ext.as_str()) {
                        continue;
                    }

                    let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Wallpaper")
                    .replace(['-', '_'], " ")
                    .split_whitespace()
                    .map(capitalize)
                    .collect::<Vec<_>>()
                    .join(" ");

                    let (w, h) = image::image_dimensions(path).unwrap_or((1920, 1080));
                    debug!("Wallpaper: {} ({}×{})", name, w, h);
                    self.entries.push(WallpaperEntry {
                        path: path.to_owned(),
                                      name,
                                      width: w,
                                      height: h,
                    });
                }
        }
        info!("Wallpapers found: {}", self.entries.len());
    }

    pub fn load(&mut self, path: &str) -> bool {
        if Path::new(path).exists() {
            if let Ok(img) = image::open(path) {
                info!("Loaded wallpaper: {} ({}×{})", path, img.width(), img.height());
                self.current = path.to_string();
                self.image = Some(img);
                return true;
            }
        }

        // Fallbacks
        let fallbacks = [
            "/usr/share/wallpapers/default.png",
            "/usr/share/wallpapers/default.jpg",
            "/usr/share/backgrounds/default.png",
            "/usr/share/backgrounds/default.jpg",
            "/usr/share/backgrounds/gnome/adwaita-l.jpg",
            "/usr/share/backgrounds/gnome/adwaita-d.jpg",
            "/usr/share/backgrounds/warty-final-ubuntu.png",
        ];
        for fb in &fallbacks {
            if Path::new(fb).exists() {
                if let Ok(img) = image::open(fb) {
                    info!("Wallpaper fallback: {}", fb);
                    self.current = fb.to_string();
                    self.image = Some(img);
                    return true;
                }
            }
        }

        // First scanned entry
        if let Some(first) = self.entries.first().map(|e| e.path.clone()) {
            if let Ok(img) = image::open(&first) {
                self.current = first.to_string_lossy().to_string();
                self.image = Some(img);
                return true;
            }
        }

        warn!("No wallpaper found — rendering solid color");
        false
    }

    pub fn set(&mut self, path: &str) -> bool {
        self.load(path)
    }

    /// RGBA pixel data for GPU upload
    pub fn rgba(&self) -> Option<(Vec<u8>, u32, u32)> {
        self.image.as_ref().map(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            (rgba.into_raw(), w, h)
        })
    }

    /// (name, path) pairs for the settings UI
    pub fn list(&self) -> Vec<(String, String)> {
        self.entries
        .iter()
        .map(|e| (e.name.clone(), e.path.to_string_lossy().to_string()))
        .collect()
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
