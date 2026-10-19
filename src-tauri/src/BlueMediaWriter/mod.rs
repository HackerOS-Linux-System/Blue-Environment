pub mod helper;
pub mod media;

use futures_util::StreamExt;
use media::*;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const EVENT: &str = "blue-media-writer://progress";
const BUNDLED_CATALOG: &str = include_str!("../../../config/stores/media-images.json");

static CANCEL: AtomicBool = AtomicBool::new(false);

pub use helper::run_priv_helper;

// ── Catalog ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub page_url: Option<String>,
    #[serde(default)]
    pub checksum_url: Option<String>,
}

#[derive(Deserialize)]
struct CatalogFile {
    #[serde(default)]
    images: Vec<CatalogEntry>,
}

fn user_catalog_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("Blue-Environment").join("media-images.json"))
}

/// Bundled catalog, overridden/extended by the user's own file (same `id`
/// replaces the bundled entry).
pub fn load_catalog() -> Vec<CatalogEntry> {
    let mut list: Vec<CatalogEntry> = serde_json::from_str::<CatalogFile>(BUNDLED_CATALOG).map(|f| f.images).unwrap_or_default();
    if let Some(p) = user_catalog_path() {
        if let Ok(txt) = std::fs::read_to_string(p) {
            if let Ok(f) = serde_json::from_str::<CatalogFile>(&txt) {
                for e in f.images {
                    match list.iter_mut().find(|x| x.id == e.id) {
                        Some(slot) => *slot = e,
                        None => list.push(e),
                    }
                }
            }
        }
    }
    list
}

#[tauri::command(async)]
pub fn mw_catalog() -> Vec<CatalogEntry> {
    load_catalog()
}

// ── Drives & images ────────────────────────────────────────────────────

#[tauri::command(async)]
pub fn mw_list_drives() -> Result<Vec<DriveInfo>, String> {
    let out = Command::new("lsblk")
        .args(["-J", "-b", "-o", "NAME,PATH,SIZE,RM,HOTPLUG,TRAN,MODEL,VENDOR,TYPE,RO,MOUNTPOINTS,LABEL"])
        .output()
        .map_err(|e| format!("Cannot run lsblk: {e}"))?;
    parse_lsblk(&String::from_utf8_lossy(&out.stdout), DEFAULT_MAX_TARGET_BYTES)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub compression: Compression,
    pub uncompressed_bytes: Option<u64>,
    pub supported: bool,
}

#[tauri::command(async)]
pub fn mw_image_info(path: String) -> Result<ImageInfo, String> {
    let meta = std::fs::metadata(&path).map_err(|e| format!("Cannot read {path}: {e}"))?;
    if !meta.is_file() {
        return Err("That is not a file".into());
    }
    let compression = detect_compression(&path);
    Ok(ImageInfo {
        name: PathBuf::from(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        size_bytes: meta.len(),
        uncompressed_bytes: uncompressed_size(&path, compression),
        supported: is_supported_image(&path),
        compression,
        path,
    })
}

// ── Cancel ─────────────────────────────────────────────────────────────

fn cancel_file() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("blue-media-writer.cancel")
}

fn reset_cancel() {
    CANCEL.store(false, Ordering::SeqCst);
    let _ = std::fs::remove_file(cancel_file());
}

/// Stops a running download / checksum and asks the root helper to stop writing.
#[tauri::command(async)]
pub fn mw_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
    let _ = std::fs::write(cancel_file(), b"1");
}

// ── Download (https only, resumable) ───────────────────────────────────

fn cache_dir() -> Result<PathBuf, String> {
    let d = dirs::cache_dir().ok_or("No cache directory")?.join("Blue-Environment").join("media-writer");
    std::fs::create_dir_all(&d).map_err(|e| format!("Cannot create {}: {e}", d.display()))?;
    Ok(d)
}

fn emit(app: &AppHandle, phase: &str, done: u64, total: Option<u64>, speed: f64) {
    let _ = app.emit(EVENT, serde_json::json!({"phase": phase, "done": done, "total": total, "speed": speed}));
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("Blue-Media-Writer/1.0")
        .build()
        .map_err(|e| e.to_string())
}

fn filename_from_disposition(v: &str) -> Option<String> {
    let idx = v.to_ascii_lowercase().find("filename=")?;
    let rest = &v[idx + "filename=".len()..];
    let name = rest.split(';').next()?.trim().trim_matches('"');
    if name.is_empty() { None } else { Some(sanitize_filename(name)) }
}

/// Downloads `url` into `~/.cache/Blue-Environment/media-writer/` and returns
/// the local path. An interrupted download is resumed (HTTP Range) the next
/// time the same URL is requested. Only `https://` is accepted, and a redirect
/// to plain http is refused.
#[tauri::command]
pub async fn mw_download(app: AppHandle, url: String) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err("Only https:// downloads are allowed".into());
    }
    reset_cancel();
    let dir = cache_dir()?;
    let key = {
        use sha2::{Digest, Sha256};
        hex(&Sha256::digest(url.as_bytes()))[..16].to_string()
    };
    let part = dir.join(format!("{key}.part"));
    let mut resume_from = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);

    let client = http_client()?;
    let mut req = client.get(&url);
    if resume_from > 0 {
        req = req.header("Range", format!("bytes={resume_from}-"));
    }
    let resp = req.send().await.map_err(|e| format!("Connection failed: {e}"))?;
    if resp.url().scheme() != "https" {
        return Err("The server redirected to an insecure (http) address — refusing".into());
    }
    let status = resp.status().as_u16();
    if status == 416 {
        // Our partial file is already complete (or the server disagrees about its size).
        let _ = std::fs::remove_file(&part);
        return Err("The partial download was out of date — please start the download again".into());
    }
    if !(status == 200 || status == 206) {
        return Err(format!("Server returned HTTP {status}"));
    }
    if status == 200 {
        resume_from = 0; // server ignored Range: start over
    }
    let name = resp
        .headers()
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .and_then(filename_from_disposition)
        .unwrap_or_else(|| sanitize_filename(resp.url().path()));
    let total = resp.content_length().map(|l| l + resume_from);

    let mut file = if resume_from > 0 {
        std::fs::OpenOptions::new().append(true).open(&part)
    } else {
        std::fs::File::create(&part)
    }
    .map_err(|e| format!("Cannot write to {}: {e}", part.display()))?;

    let mut done = resume_from;
    let mut stream = resp.bytes_stream();
    let (mut win, mut win_bytes) = (Instant::now(), 0u64);
    emit(&app, "download", done, total, 0.0);
    while let Some(chunk) = stream.next().await {
        if CANCEL.load(Ordering::SeqCst) {
            return Err("Cancelled".into()); // keep the .part file so it can be resumed
        }
        let chunk = chunk.map_err(|e| format!("Connection lost: {e} — start again to resume"))?;
        file.write_all(&chunk).map_err(|e| format!("Disk write failed: {e}"))?;
        done += chunk.len() as u64;
        win_bytes += chunk.len() as u64;
        if win.elapsed() >= Duration::from_millis(250) {
            emit(&app, "download", done, total, win_bytes as f64 / win.elapsed().as_secs_f64().max(0.001));
            win = Instant::now();
            win_bytes = 0;
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    if let Some(t) = total {
        if done != t {
            return Err("The download ended early — start again to resume".into());
        }
    }
    let final_path = dir.join(&name);
    std::fs::rename(&part, &final_path).map_err(|e| format!("Cannot finish the download: {e}"))?;
    emit(&app, "download", done, total, 0.0);
    Ok(final_path.to_string_lossy().to_string())
}

/// Fetches a `sha256sum`-style file and returns the digest for `filename`.
#[tauri::command]
pub async fn mw_fetch_checksum(url: String, filename: String) -> Result<Option<String>, String> {
    if !url.starts_with("https://") {
        return Err("Only https:// is allowed".into());
    }
    let resp = http_client()?.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server returned HTTP {}", resp.status().as_u16()));
    }
    if resp.content_length().unwrap_or(0) > 2 * 1024 * 1024 {
        return Err("Checksum file is unexpectedly large".into());
    }
    let text = resp.text().await.map_err(|e| e.to_string())?;
    Ok(find_checksum(&text, &filename))
}

/// SHA-256 of a local file (with progress), lower-case hex.
#[tauri::command]
pub async fn mw_hash(app: AppHandle, path: String) -> Result<String, String> {
    reset_cancel();
    tokio::task::spawn_blocking(move || {
        use sha2::{Digest, Sha256};
        let mut f = std::fs::File::open(&path).map_err(|e| format!("Cannot open {path}: {e}"))?;
        let total = f.metadata().ok().map(|m| m.len());
        let mut h = Sha256::new();
        let mut buf = vec![0u8; 1 << 20];
        let (mut done, mut win, mut win_bytes) = (0u64, Instant::now(), 0u64);
        loop {
            if CANCEL.load(Ordering::SeqCst) {
                return Err("Cancelled".to_string());
            }
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
            done += n as u64;
            win_bytes += n as u64;
            if win.elapsed() >= Duration::from_millis(250) {
                emit(&app, "hash", done, total, win_bytes as f64 / win.elapsed().as_secs_f64().max(0.001));
                win = Instant::now();
                win_bytes = 0;
            }
        }
        emit(&app, "hash", done, total, 0.0);
        Ok(hex(&h.finalize()))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Write / restore (root helper) ──────────────────────────────────────

fn run_helper(app: &AppHandle, mut req: serde_json::Value) -> Result<String, String> {
    reset_cancel();
    req["cancelFile"] = serde_json::json!(cancel_file().to_string_lossy());
    let payload = req.to_string();
    let mut final_result: Option<Result<String, String>> = None;
    let outcome = crate::privileged::run_privileged(
        &["--media-writer-priv"],
        &payload,
        |l| {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(l) else { return };
            match v.get("event").and_then(|e| e.as_str()) {
                Some("progress") => {
                    let phase = v.get("phase").and_then(|p| p.as_str()).unwrap_or("write");
                    let done = v.get("done").and_then(|p| p.as_u64()).unwrap_or(0);
                    let total = v.get("total").and_then(|p| p.as_u64());
                    let speed = v.get("speed").and_then(|p| p.as_f64()).unwrap_or(0.0);
                    emit(app, phase, done, total, speed);
                }
                Some("done") => {
                    final_result = Some(Ok(v.get("sha256").and_then(|s| s.as_str()).unwrap_or("").to_string()));
                }
                Some("error") => {
                    final_result = Some(Err(v.get("message").and_then(|s| s.as_str()).unwrap_or("Unknown error").to_string()));
                }
                _ => {}
            }
        },
        |_| {},
    )
    .map_err(|e| match e.hint {
        Some(h) => format!("{} — {h}", e.message),
        None => e.message,
    })?;
    if let Some(r) = final_result {
        return r;
    }
    let info = crate::privileged::explain_launch_failure(&outcome);
    Err(match info.hint {
        Some(h) => format!("{} — {h}", info.message),
        None => info.message,
    })
}

/// Writes `image` to `device` (asks for administrator rights). Returns the
/// SHA-256 of the data that was written.
#[tauri::command]
pub async fn mw_write(app: AppHandle, image: String, device: String, verify: bool) -> Result<String, String> {
    let req = serde_json::json!({"op": "write", "image": image, "device": device, "verify": verify});
    tokio::task::spawn_blocking(move || run_helper(&app, req)).await.map_err(|e| e.to_string())?
}

/// Erases `device` and creates a single empty FAT32 partition.
#[tauri::command]
pub async fn mw_restore(app: AppHandle, device: String) -> Result<(), String> {
    let req = serde_json::json!({"op": "restore", "device": device});
    tokio::task::spawn_blocking(move || run_helper(&app, req).map(|_| ())).await.map_err(|e| e.to_string())?
}
