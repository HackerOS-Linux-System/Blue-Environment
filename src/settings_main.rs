mod config;
mod wallpaper;

use anyhow::Result;
use slint::ComponentHandle;
use std::io::Write;
use std::os::unix::net::UnixStream;
use tracing::info;
use tracing_subscriber::EnvFilter;

slint::include_modules!();

fn main() -> Result<()> {
    tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::new("info"))
    .without_time()
    .init();

    info!("Blue Settings starting...");

    let mut cfg = config::BlueConfig::load_or_default();

    let win = BlueSettings::new()?;

    // ── Populate from config ──────────────────────────────
    win.set_current_wallpaper(cfg.wallpaper.clone().into());
    win.set_current_theme(cfg.theme.clone().into());
    win.set_enable_animations(cfg.enable_animations);
    win.set_enable_blur(cfg.enable_blur);
    win.set_window_blur(cfg.blur_radius);
    win.set_enable_shadows(cfg.enable_shadows);
    win.set_corner_radius(cfg.window_corner_radius);
    win.set_scale_factor(cfg.scale_factor as f32);

    // ── Scan wallpapers ───────────────────────────────────
    let wp_list = {
        let mut mgr = wallpaper::WallpaperManager::new(&cfg.wallpaper);
        mgr.list()
    };
    info!("Wallpapers found: {}", wp_list.len());
    // Note: Slint model population is done below via on_wallpaper_selected

    // ── Page navigation ───────────────────────────────────
    let ww = win.as_weak();
    win.on_page_changed(move |p| {
        ww.upgrade().map(|w| w.set_active_page(p));
    });

    // ── Wallpaper ─────────────────────────────────────────
    win.on_wallpaper_selected(move |path| {
        let path = path.to_string();
        info!("Wallpaper → {}", path);
        ipc_set_wallpaper(&path);
        // Also update config directly
        let mut c = config::BlueConfig::load_or_default();
        c.set_wallpaper(&path).ok();
    });

    // ── Theme ─────────────────────────────────────────────
    win.on_theme_changed(move |t| {
        let mut c = config::BlueConfig::load_or_default();
        c.set_theme(&t.to_string()).ok();
        ipc_reload();
    });

    // ── Effects ───────────────────────────────────────────
    win.on_animations_toggled(move |v| {
        let mut c = config::BlueConfig::load_or_default();
        c.set_animations(v).ok();
    });
    win.on_blur_toggled(move |v| {
        let mut c = config::BlueConfig::load_or_default();
        c.set_blur(v).ok();
    });
    win.on_blur_changed(move |r| {
        let mut c = config::BlueConfig::load_or_default();
        c.set_blur_radius(r).ok();
    });
    win.on_shadows_toggled(move |v| {
        let mut c = config::BlueConfig::load_or_default();
        c.set_shadows(v).ok();
    });
    win.on_corner_radius_changed(move |r| {
        let mut c = config::BlueConfig::load_or_default();
        c.set_corner_radius(r).ok();
    });
    win.on_scale_changed(move |s| {
        let mut c = config::BlueConfig::load_or_default();
        c.set_scale(s as f64).ok();
    });

    // ── Volume ────────────────────────────────────────────
    win.on_volume_changed(|v| {
        let pct = (v * 100.0) as u32;
        let _ = std::process::Command::new("pactl")
        .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", pct)])
        .spawn();
    });

    info!("Blue Settings ready");
    win.run()?;
    Ok(())
}

// ── IPC helpers ───────────────────────────────────────────

fn ipc_socket() -> std::path::PathBuf {
    let uid = nix::unistd::getuid().as_raw();
    std::path::PathBuf::from(format!("/run/user/{}/blue-environment.sock", uid))
}

fn ipc_send_json(json: &str) {
    if let Ok(mut s) = UnixStream::connect(ipc_socket()) {
        s.write_all(json.as_bytes()).ok();
        s.write_all(b"\n").ok();
    }
}

fn ipc_set_wallpaper(path: &str) {
    let msg = serde_json::json!({ "type": "SetWallpaper", "data": { "path": path } });
    ipc_send_json(&msg.to_string());
}

fn ipc_reload() {
    let msg = serde_json::json!({ "type": "ConfigReload" });
    ipc_send_json(&msg.to_string());
}
