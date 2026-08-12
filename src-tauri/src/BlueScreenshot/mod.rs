use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn compute_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let pics = home.join("Pictures").join("Screenshots");
    let _ = fs::create_dir_all(&pics);
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    Some(pics.join(format!("screenshot-{}.png", ts)))
}

/// Computes (and ensures the directory exists for) a fresh screenshot
/// path without capturing anything — used by the unified screenshot flow
/// in `compositorBridge.ts`'s `takeScreenshotUnified()` so the compositor
/// IPC path (which needs a path to write to, but has no reason to know
/// Tauri's `dirs` conventions) and the CLI fallback in `take_screenshot`
/// below agree on where screenshots live.
#[tauri::command]
pub fn default_screenshot_path() -> String {
    compute_path().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
}

/// CLI-based fallback screenshot capture. Only reached from the frontend
/// when the compositor IPC path (`CompositorBridge.takeScreenshot`, which
/// now goes through the `wlr-screencopy-v1` protocol implemented in
/// `compositor/src/protocols/screencopy.rs`) doesn't respond in time —
/// i.e. sessions where Blue Compositor isn't actually running the show
/// (X11, or a different Wayland compositor), where these external tools
/// are the only option anyway.
#[tauri::command]
pub fn take_screenshot() -> String {
    let Some(path_buf) = compute_path() else { return String::new() };
    let path = path_buf.to_string_lossy().to_string();
    // Try Wayland/X11 tools in order of preference: grim (Wayland),
    // import from ImageMagick (X11), scrot (X11), flameshot, spectacle.
    let cmd = format!(
        "grim '{p}' 2>/dev/null || import -window root '{p}' 2>/dev/null || \
         scrot '{p}' 2>/dev/null || flameshot full -p '{p}' 2>/dev/null || \
         spectacle -b -o '{p}' 2>/dev/null",
        p = path
    );
    match Command::new("sh").arg("-c").arg(&cmd).status() {
        Ok(s) if s.success() => path,
        _ => String::new(),
    }
}
