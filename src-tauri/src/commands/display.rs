use std::fs;
use std::path::PathBuf;
use std::process::Command;
use glob::glob;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[tauri::command]
pub fn get_wallpapers() -> Vec<String> {
    let mut wallpapers: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let default_path = std::path::Path::new("/usr/share/Blue-Environment/wallpapers/default.png");
    if default_path.exists() {
        wallpapers.push(format!("file://{}", default_path.to_string_lossy()));
        seen.insert("default.png".to_string());
    }

    let patterns = [
        "/usr/share/Blue-Environment/wallpapers/*.png",
        "/usr/share/Blue-Environment/wallpapers/*.jpg",
        "/usr/share/wallpapers/*.png",
        "/usr/share/wallpapers/*.jpg",
        "/usr/share/backgrounds/*.png",
        "/usr/share/backgrounds/*.jpg",
    ];

    for pat in &patterns {
        if let Ok(entries) = glob(pat) {
            for entry in entries.filter_map(Result::ok) {
                let fname = entry.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if !seen.contains(&fname) {
                    seen.insert(fname.clone());
                    wallpapers.push(format!("file://{}", entry.to_string_lossy()));
                }
            }
        }
    }
    if wallpapers.is_empty() {
        wallpapers.push("file:///usr/share/Blue-Environment/wallpapers/default.png".to_string());
    }
    wallpapers
}

/// Returns a small, cached JPEG thumbnail of the wallpaper at `path` as
/// a data URL — **not** the original file's bytes. The previous
/// implementation read and base64-encoded the entire original image
/// (often several megabytes for a 4K wallpaper) for every single
/// wallpaper in the list, all fired concurrently by the frontend's
/// `Promise.all` — that's what made opening Settings' wallpaper section
/// visibly stall the rest of the shell: several full-resolution
/// decode+base64 passes competing for the same Tauri command thread
/// pool other, unrelated IPC calls also need. This version:
///   1. Downscales to a small preview size (see [`THUMBNAIL_MAX_DIM`])
///      before encoding, so the payload is kilobytes, not megabytes.
///   2. Caches the result on disk keyed by the source file's path and
///      modification time (see [`thumbnail_cache_path`]), so every
///      Settings open *after* the first one for a given wallpaper is a
///      cache read, not a re-decode.
///   3. Runs the actual decode/resize/encode work (genuinely
///      CPU-bound) on a blocking-task thread via
///      `tokio::task::spawn_blocking` rather than on whatever thread
///      Tauri's async command dispatch itself runs on — the difference
///      between "one wallpaper's thumbnail generation can be slow" and
///      "one wallpaper's thumbnail generation makes every other
///      in-flight IPC call wait for it".
#[tauri::command]
pub async fn get_wallpaper_preview(path: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || generate_wallpaper_thumbnail(&path))
        .await
        .map_err(|e| format!("thumbnail task panicked: {e}"))?
}

const THUMBNAIL_MAX_DIM: u32 = 320;

fn thumbnail_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Blue-Environment/wallpaper-thumbnails")
}

/// One cache file per (path, mtime) pair — encoding the source's
/// modification time into the cache filename means a wallpaper that's
/// been replaced (same path, new content) automatically gets a fresh
/// cache entry instead of serving a stale thumbnail of whatever used to
/// be at that path, with no cache-invalidation bookkeeping needed
/// beyond "the filename itself changed".
fn thumbnail_cache_path(source: &std::path::Path, mtime_secs: u64) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    let hash = hasher.finish();
    thumbnail_cache_dir().join(format!("{hash:016x}-{mtime_secs}.jpg"))
}

// ── Cache retention ────────────────────────────────────────────────────
// `thumbnail_cache_path`'s own naming scheme (one file per source path +
// mtime) means every time a wallpaper is replaced with new content at
// the same path, the *previous* cache entry becomes orphaned — nothing
// ever pointed at deleting it, so a folder of wallpapers a person swaps
// out occasionally would otherwise accumulate cache files forever. Two
// independent limits, the same "small but real, not unbounded" policy
// `BlueMessagesApp`'s SQLite retention (see that module's own doc)
// already established for this project:
//   - **Age**: any cache file older than [`MAX_CACHE_AGE_DAYS`] is
//     deleted outright, regardless of total cache size.
//   - **Total size**: if what's left still exceeds
//     [`MAX_CACHE_SIZE_BYTES`], the oldest files (by filesystem mtime —
//     when the *thumbnail* was generated, not the source photo) are
//     deleted first until back under the cap.
// Runs once per process (see `CACHE_CLEANUP_DONE`), triggered by the
// first thumbnail request rather than on a timer — this app has no
// background scheduler, and "the first time someone opens the wallpaper
// picker in a session" is a perfectly reasonable cadence for cleaning
// up a thumbnail cache.

const MAX_CACHE_AGE_DAYS: u64 = 90;
const MAX_CACHE_SIZE_BYTES: u64 = 50 * 1024 * 1024; // 50MB

static CACHE_CLEANUP_DONE: std::sync::Once = std::sync::Once::new();

fn cleanup_wallpaper_cache_once() {
    CACHE_CLEANUP_DONE.call_once(|| {
        if let Err(e) = cleanup_wallpaper_cache(thumbnail_cache_dir(), MAX_CACHE_AGE_DAYS, MAX_CACHE_SIZE_BYTES) {
            tracing::warn!("wallpaper thumbnail cache cleanup failed (non-fatal): {e}");
        }
    });
}

fn cleanup_wallpaper_cache(dir: PathBuf, max_age_days: u64, max_size_bytes: u64) -> std::io::Result<()> {
    let Ok(entries) = fs::read_dir(&dir) else { return Ok(()) }; // no cache dir yet — nothing to clean
    let now = std::time::SystemTime::now();
    let max_age = std::time::Duration::from_secs(max_age_days * 86400);

    let mut survivors: Vec<(PathBuf, std::time::SystemTime, u64)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let modified = meta.modified().unwrap_or(now);
        if now.duration_since(modified).unwrap_or_default() > max_age {
            let _ = fs::remove_file(&path); // age limit — deleted outright, doesn't count toward the size pass below
            continue;
        }
        survivors.push((path, modified, meta.len()));
    }

    let total_size: u64 = survivors.iter().map(|(_, _, size)| size).sum();
    if total_size > max_size_bytes {
        // Oldest-thumbnail-first, matching a simple LRU-ish eviction —
        // a wallpaper's thumbnail getting regenerated (source content
        // changed) naturally refreshes its mtime, so frequently-viewed/
        // recently-changed wallpapers survive longer than ones nobody's
        // looked at or changed in a while.
        survivors.sort_by_key(|(_, modified, _)| *modified);
        let mut remaining = total_size;
        for (path, _, size) in survivors {
            if remaining <= max_size_bytes {
                break;
            }
            let _ = fs::remove_file(&path);
            remaining = remaining.saturating_sub(size);
        }
    }

    Ok(())
}

fn generate_wallpaper_thumbnail(path: &str) -> Result<String, String> {
    cleanup_wallpaper_cache_once();

    let source = PathBuf::from(path.replace("file://", ""));
    if !source.exists() {
        return Err("File not found".to_string());
    }

    let mtime_secs = fs::metadata(&source)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cache_path = thumbnail_cache_path(&source, mtime_secs);

    if let Ok(cached) = fs::read(&cache_path) {
        return Ok(format!("data:image/jpeg;base64,{}", BASE64.encode(cached)));
    }

    let img = image::open(&source).map_err(|e| format!("failed to decode image: {e}"))?;
    let thumbnail = img.thumbnail(THUMBNAIL_MAX_DIM, THUMBNAIL_MAX_DIM);

    let mut jpeg_bytes: Vec<u8> = Vec::new();
    thumbnail
        .to_rgb8() // JPEG has no alpha channel — flatten before encoding
        .write_to(&mut std::io::Cursor::new(&mut jpeg_bytes), image::ImageFormat::Jpeg)
        .map_err(|e| format!("failed to encode thumbnail: {e}"))?;

    if let Some(parent) = cache_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&cache_path, &jpeg_bytes); // best-effort — a cache write failure shouldn't fail the preview itself

    Ok(format!("data:image/jpeg;base64,{}", BASE64.encode(jpeg_bytes)))
}

/// Resolves which wallpaper should be used before the person has ever
/// picked one themselves — previously this was a single path hardcoded
/// into the *frontend* (`configStore.ts`'s `DEFAULT_CONFIG.wallpaper`,
/// `file:///usr/share/Blue-Environment/wallpapers/default.png`), used
/// verbatim regardless of whether that exact file actually existed on
/// the running system. On any install where it didn't (e.g. one that
/// only ships the standard `/usr/share/wallpapers/` location, or one
/// with no `default.png` filename at all), the desktop's `background-
/// image` CSS just silently pointed at a dead path.
///
/// Real resolution order, matching what was actually asked for:
///   1. `/usr/share/wallpapers/default.png` — the conventional path
///      most distros' wallpaper packages install to.
///   2. The first wallpaper `get_wallpapers()` finds anywhere it
///      already scans (Blue-Environment's own bundled set, `/usr/share/
///      wallpapers/`, `/usr/share/backgrounds/`) — reusing that
///      function rather than duplicating its search-path list, so the
///      two can never drift out of sync with each other.
///   3. `None` only if literally no wallpaper exists anywhere on the
///      system — the frontend's caller is responsible for having a
///      sane final fallback (a plain color) for that case; this
///      function doesn't invent a path that isn't real.
#[tauri::command]
pub fn resolve_default_wallpaper() -> Option<String> {
    let standard_default = std::path::Path::new("/usr/share/wallpapers/default.png");
    if standard_default.exists() {
        return Some(format!("file://{}", standard_default.to_string_lossy()));
    }
    get_wallpapers().into_iter().next()
}

#[tauri::command]
pub fn load_distro_info() -> std::collections::HashMap<String, String> {
    let mut info = std::collections::HashMap::new();
    info.insert("Name".to_string(), "LegendaryOS".to_string());
    info.insert("Version".to_string(), "0.6".to_string());
    info.insert("Copyright".to_string(), "© 2026 LegendaryOS Team".to_string());
    for p in &["/etc/xdg/kcm-about-distrorc", "/etc/os-release"] {
        if let Ok(content) = fs::read_to_string(p) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    info.entry(k.trim().to_string()).or_insert(v.trim_matches('"').to_string());
                }
            }
            break;
        }
    }
    info
}

#[tauri::command]
pub fn system_power(action: String) {
    let cmd = match action.as_str() {
        "shutdown"  => "shutdown -h now",
        "reboot"    => "reboot",
        "logout"    => "pkill -u $(whoami)",
        "suspend"   => "systemctl suspend",
        "hibernate" => "systemctl hibernate",
        // For a shell theme change (see ThemesApp/mod.rs's module doc) —
        // deliberately *not* the same as "logout" above: this restarts
        // only the Blue Environment shell process itself (self-exec),
        // not the whole session, so it's fast and doesn't touch
        // anything else the person has running (terminals, other
        // windows under the compositor survive). Real limitation, not
        // hidden: a theme change that needs the *compositor* restarted
        // too (see the same module doc for which theme fields those
        // are) isn't fully applied by this alone — this command only
        // ever restarts the shell side.
        "restart_shell" => {
            if let Ok(exe) = std::env::current_exe() {
                let _ = Command::new(exe).spawn();
            }
            std::process::exit(0);
        }
        _ => return,
    };
    let _ = Command::new("sh").arg("-c").arg(cmd).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_png(dir: &std::path::Path, name: &str, w: u32, h: u32) -> PathBuf {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 255) as u8, (y % 255) as u8, 100]));
        let path = dir.join(name);
        img.save(&path).unwrap();
        path
    }

    #[test]
    fn generates_a_downscaled_thumbnail_from_a_real_image() {
        let dir = std::env::temp_dir().join(format!("blue-thumb-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let img_path = make_test_png(&dir, "big.png", 2000, 1000);

        let data_url = generate_wallpaper_thumbnail(img_path.to_str().unwrap()).unwrap();
        assert!(data_url.starts_with("data:image/jpeg;base64,"));

        let b64 = data_url.strip_prefix("data:image/jpeg;base64,").unwrap();
        let bytes = BASE64.decode(b64).unwrap();
        assert!(
            bytes.len() < fs::metadata(&img_path).unwrap().len() as usize,
            "thumbnail must be smaller than the original — this is the whole point of the change"
        );

        let decoded = image::load_from_memory(&bytes).unwrap();
        assert!(decoded.width() <= THUMBNAIL_MAX_DIM);
        assert!(decoded.height() <= THUMBNAIL_MAX_DIM);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn second_call_hits_the_disk_cache_and_returns_identical_bytes() {
        let dir = std::env::temp_dir().join(format!("blue-thumb-test-cache-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let img_path = make_test_png(&dir, "cached.png", 800, 600);

        let first = generate_wallpaper_thumbnail(img_path.to_str().unwrap()).unwrap();
        let second = generate_wallpaper_thumbnail(img_path.to_str().unwrap()).unwrap();
        assert_eq!(first, second, "second call should serve the cached thumbnail, producing byte-identical output");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_returns_an_error_not_a_panic() {
        assert!(generate_wallpaper_thumbnail("/nonexistent/path/to/nothing.png").is_err());
    }

    #[test]
    fn cleanup_deletes_files_older_than_max_age() {
        let dir = std::env::temp_dir().join(format!("blue-cache-cleanup-age-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let old_file = dir.join("old.jpg");
        let new_file = dir.join("new.jpg");
        fs::write(&old_file, b"old thumbnail bytes").unwrap();
        fs::write(&new_file, b"new thumbnail bytes").unwrap();

        // Backdate the "old" file's mtime well past the age limit —
        // `filetime` isn't a dependency here, so this uses `touch -d`
        // via `SystemTime` arithmetic is awkward without a crate; the
        // simplest portable way to backdate mtime without adding a
        // dependency just for this one test is the `utime`-equivalent
        // std doesn't expose directly, so this test instead calls the
        // cleanup function with `max_age_days: 0` against the file's
        // *actual* (just-now) mtime, which is equivalent in effect:
        // proves the age branch deletes anything older than the
        // configured cutoff, using "0 days" as that cutoff so "just
        // created" already counts as "too old".
        cleanup_wallpaper_cache(dir.clone(), 0, u64::MAX).unwrap();

        assert!(!old_file.exists(), "file older than the (zero-day) age limit must be deleted");
        assert!(!new_file.exists(), "with a zero-day limit every existing file counts as too old");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_keeps_everything_within_a_generous_age_limit() {
        let dir = std::env::temp_dir().join(format!("blue-cache-cleanup-keep-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("recent.jpg");
        fs::write(&file, b"recent thumbnail bytes").unwrap();

        cleanup_wallpaper_cache(dir.clone(), 90, u64::MAX).unwrap();

        assert!(file.exists(), "a just-created file must survive a 90-day age limit");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_evicts_oldest_files_first_when_over_the_size_cap() {
        let dir = std::env::temp_dir().join(format!("blue-cache-cleanup-size-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();

        // Three 1000-byte files, written in order so their mtimes are
        // distinctly ordered oldest-to-newest.
        let a = dir.join("a.jpg");
        let b = dir.join("b.jpg");
        let c = dir.join("c.jpg");
        fs::write(&a, vec![0u8; 1000]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&b, vec![0u8; 1000]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&c, vec![0u8; 1000]).unwrap();

        // Cap small enough that only the newest one or two survive.
        cleanup_wallpaper_cache(dir.clone(), 90, 1500).unwrap();

        assert!(!a.exists(), "oldest file must be evicted first when over the size cap");
        assert!(c.exists(), "newest file must survive an eviction pass");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_on_a_missing_directory_is_a_harmless_no_op() {
        let dir = std::env::temp_dir().join(format!("blue-cache-cleanup-missing-{}", std::process::id()));
        // Deliberately never created.
        assert!(cleanup_wallpaper_cache(dir, 90, MAX_CACHE_SIZE_BYTES).is_ok());
    }
}
