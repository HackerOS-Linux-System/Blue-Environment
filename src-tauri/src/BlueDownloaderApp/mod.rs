use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    /// `resumable` reflects whether the server advertised
    /// `Accept-Ranges: bytes` on the original response — the frontend
    /// uses this to decide whether "Resume" would actually work or
    /// would just restart from zero (still offered either way, just
    /// labeled honestly).
    Paused { resumable: bool },
    Completed,
    Failed { error: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub destination_path: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub status: DownloadStatus,
    pub created_at: String,
    /// Bytes/second, computed over roughly the last second of transfer
    /// — `None` when not actively downloading. Not persisted (kept in
    /// the in-memory registry only): a speed reading from before the
    /// app last closed isn't meaningful to show again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_bytes_per_sec: Option<f64>,
}

/// Per-download control flags for a task currently running, keyed by
/// download id — lets `downloader_pause`/`downloader_cancel` signal an
/// in-flight `tokio::spawn`ed task without needing a channel per
/// download.
struct RunningControl {
    pause_requested: Arc<AtomicBool>,
    cancel_requested: Arc<AtomicBool>,
}

static RUNNING: Mutex<Option<HashMap<String, RunningControl>>> = Mutex::new(None);
fn with_running<T>(f: impl FnOnce(&mut HashMap<String, RunningControl>) -> T) -> T {
    let mut guard = RUNNING.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    f(guard.as_mut().unwrap())
}

fn downloader_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Blue-Environment").join("downloader")
}
fn history_path() -> PathBuf { downloader_dir().join("downloads.json") }

fn default_destination_dir() -> PathBuf {
    dirs::download_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")))
}

fn read_history() -> Vec<DownloadItem> {
    std::fs::read_to_string(history_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn write_history(items: &[DownloadItem]) {
    let _ = std::fs::create_dir_all(downloader_dir());
    if let Ok(json) = serde_json::to_string_pretty(items) {
        let _ = std::fs::write(history_path(), json);
    }
}
fn update_item(id: &str, f: impl FnOnce(&mut DownloadItem)) {
    let mut items = read_history();
    if let Some(item) = items.iter_mut().find(|d| d.id == id) {
        f(item);
        write_history(&items);
    }
}

/// Derives a filename from the URL's path, falling back to a generic
/// name if the URL has none (e.g. it points at a bare domain) —
/// refined later from the response's `Content-Disposition` header if
/// the server sends one (see `run_download`).
fn filename_from_url(url: &str) -> String {
    url.split('?').next().unwrap_or(url)
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("download")
        .to_string()
}

/// Picks a filename that doesn't already exist in `dir`, appending
/// " (1)", " (2)", ... before the extension — same convention as this
/// project's file manager's own copy-conflict naming, so two downloads
/// of the same filename don't silently clobber each other.
fn unique_path(dir: &std::path::Path, filename: &str) -> PathBuf {
    let candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match filename.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (filename.to_string(), String::new()),
    };
    for n in 1.. {
        let attempt = dir.join(format!("{stem} ({n}){ext}"));
        if !attempt.exists() {
            return attempt;
        }
    }
    unreachable!()
}

/// Starts a new download in the background and returns its (initial)
/// metadata immediately — the frontend tracks progress via
/// `blue-downloader://progress` events, not by polling.
#[tauri::command]
pub async fn downloader_add(app: AppHandle, url: String, destination_dir: Option<String>) -> Result<DownloadItem, String> {
    let dest_dir = destination_dir.map(PathBuf::from).unwrap_or_else(default_destination_dir);
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("Could not create destination folder: {e}"))?;
    let filename = filename_from_url(&url);
    let destination_path = unique_path(&dest_dir, &filename);

    let item = DownloadItem {
        id: uuid::Uuid::new_v4().to_string(),
        url: url.clone(),
        filename: destination_path.file_name().and_then(|n| n.to_str()).unwrap_or(&filename).to_string(),
        destination_path: destination_path.to_string_lossy().to_string(),
        total_bytes: None,
        downloaded_bytes: 0,
        status: DownloadStatus::Queued,
        created_at: chrono::Utc::now().to_rfc3339(),
        speed_bytes_per_sec: None,
    };

    let mut items = read_history();
    items.push(item.clone());
    write_history(&items);

    spawn_download(app, item.clone());
    Ok(item)
}

fn spawn_download(app: AppHandle, item: DownloadItem) {
    let pause_requested = Arc::new(AtomicBool::new(false));
    let cancel_requested = Arc::new(AtomicBool::new(false));
    with_running(|r| {
        r.insert(item.id.clone(), RunningControl { pause_requested: pause_requested.clone(), cancel_requested: cancel_requested.clone() });
    });
    tokio::spawn(run_download(app, item, 0, pause_requested, cancel_requested));
}

/// The actual transfer loop — streams the response body to disk in
/// chunks, checking `pause_requested`/`cancel_requested` between
/// chunks (not mid-chunk; a single chunk from `bytes_stream()` is at
/// most a network read's worth of bytes, so this is a small, bounded
/// delay before a pause/cancel actually takes effect, not an
/// unresponsive one).
async fn run_download(
    app: AppHandle,
    mut item: DownloadItem,
    resume_from: u64,
    pause_requested: Arc<AtomicBool>,
    cancel_requested: Arc<AtomicBool>,
) {
    update_item(&item.id, |d| d.status = DownloadStatus::Downloading);
    let _ = app.emit("blue-downloader://progress", &item);

    let client = reqwest::Client::new();
    let mut request = client.get(&item.url);
    if resume_from > 0 {
        request = request.header("Range", format!("bytes={resume_from}-"));
    }

    let response = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            fail_download(&app, &item.id, format!("Request failed: {e}"));
            return;
        }
    };
    if !response.status().is_success() && response.status().as_u16() != 206 {
        fail_download(&app, &item.id, format!("Server returned HTTP {}", response.status().as_u16()));
        return;
    }

    // Prefer the server's suggested filename (Content-Disposition) over
    // the URL-derived guess, if it sent one and we haven't started
    // writing yet (renaming mid-download would orphan already-written
    // bytes under the old name).
    if resume_from == 0 {
        if let Some(suggested) = response
            .headers()
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .and_then(parse_content_disposition_filename)
        {
            let dir = std::path::Path::new(&item.destination_path).parent().unwrap_or(std::path::Path::new("."));
            let new_path = unique_path(dir, &suggested);
            item.filename = new_path.file_name().and_then(|n| n.to_str()).unwrap_or(&suggested).to_string();
            item.destination_path = new_path.to_string_lossy().to_string();
            update_item(&item.id, |d| { d.filename = item.filename.clone(); d.destination_path = item.destination_path.clone(); });
        }
    }

    let total_bytes = response.content_length().map(|len| len + resume_from);
    let resumable = response.headers().get("accept-ranges").and_then(|v| v.to_str().ok()) == Some("bytes");
    update_item(&item.id, |d| d.total_bytes = total_bytes);
    item.total_bytes = total_bytes;

    let file_result = if resume_from > 0 {
        std::fs::OpenOptions::new().append(true).open(&item.destination_path)
    } else {
        std::fs::File::create(&item.destination_path)
    };
    let mut file = match file_result {
        Ok(f) => f,
        Err(e) => {
            fail_download(&app, &item.id, format!("Could not open destination file: {e}"));
            return;
        }
    };

    let mut downloaded = resume_from;
    let mut stream = response.bytes_stream();
    let mut last_emit = std::time::Instant::now();
    let mut bytes_since_last_emit = 0u64;

    use std::io::Write;
    loop {
        if cancel_requested.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&item.destination_path);
            update_item(&item.id, |d| d.status = DownloadStatus::Cancelled);
            with_running(|r| { r.remove(&item.id); });
            item.status = DownloadStatus::Cancelled;
            let _ = app.emit("blue-downloader://progress", &item);
            return;
        }
        if pause_requested.load(Ordering::Relaxed) {
            update_item(&item.id, |d| d.status = DownloadStatus::Paused { resumable });
            with_running(|r| { r.remove(&item.id); });
            item.status = DownloadStatus::Paused { resumable };
            item.downloaded_bytes = downloaded;
            let _ = app.emit("blue-downloader://progress", &item);
            return;
        }

        match stream.next().await {
            Some(Ok(chunk)) => {
                if let Err(e) = file.write_all(&chunk) {
                    fail_download(&app, &item.id, format!("Disk write failed: {e}"));
                    with_running(|r| { r.remove(&item.id); });
                    return;
                }
                downloaded += chunk.len() as u64;
                bytes_since_last_emit += chunk.len() as u64;

                // Throttled progress emission — see module doc for why
                // this isn't "emit on every chunk".
                if last_emit.elapsed() >= std::time::Duration::from_millis(200) {
                    let speed = bytes_since_last_emit as f64 / last_emit.elapsed().as_secs_f64().max(0.001);
                    item.downloaded_bytes = downloaded;
                    item.speed_bytes_per_sec = Some(speed);
                    let _ = app.emit("blue-downloader://progress", &item);
                    update_item(&item.id, |d| d.downloaded_bytes = downloaded);
                    last_emit = std::time::Instant::now();
                    bytes_since_last_emit = 0;
                }
            }
            Some(Err(e)) => {
                fail_download(&app, &item.id, format!("Connection error: {e}"));
                with_running(|r| { r.remove(&item.id); });
                return;
            }
            None => break, // stream ended — download complete
        }
    }

    let _ = file.flush();
    update_item(&item.id, |d| { d.status = DownloadStatus::Completed; d.downloaded_bytes = downloaded; });
    with_running(|r| { r.remove(&item.id); });
    item.status = DownloadStatus::Completed;
    item.downloaded_bytes = downloaded;
    item.speed_bytes_per_sec = None;
    let _ = app.emit("blue-downloader://progress", &item);
}

fn fail_download(app: &AppHandle, id: &str, error: String) {
    update_item(id, |d| d.status = DownloadStatus::Failed { error: error.clone() });
    with_running(|r| { r.remove(id); });
    if let Some(item) = read_history().into_iter().find(|d| d.id == id) {
        let _ = app.emit("blue-downloader://progress", &item);
    }
}

/// Extracts `filename="..."` (or the unquoted form) from a
/// `Content-Disposition` header value. Deliberately simple — doesn't
/// handle the RFC 5987 `filename*=UTF-8''...` extended form, which
/// covers non-ASCII filenames; those fall back to the URL-derived name
/// instead of this parser guessing wrong.
fn parse_content_disposition_filename(header: &str) -> Option<String> {
    for part in header.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename=") {
            let unquoted = rest.trim_matches('"');
            if !unquoted.is_empty() {
                return Some(unquoted.to_string());
            }
        }
    }
    None
}

#[tauri::command]
pub fn downloader_list() -> Vec<DownloadItem> {
    read_history()
}

#[tauri::command]
pub fn downloader_pause(id: String) -> Result<(), String> {
    with_running(|r| {
        r.get(&id).map(|c| c.pause_requested.store(true, Ordering::Relaxed))
    })
    .ok_or_else(|| "That download isn't currently running".to_string())
}

#[tauri::command]
pub fn downloader_cancel(id: String) -> Result<(), String> {
    // If it's currently running, signal the task to stop (which itself
    // deletes the partial file and updates status — see run_download).
    let was_running = with_running(|r| {
        if let Some(c) = r.get(&id) {
            c.cancel_requested.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    });
    if was_running {
        return Ok(());
    }
    // Not running (e.g. already Paused/Failed) — cancel directly here:
    // delete any partial file and mark Cancelled.
    let items = read_history();
    let Some(item) = items.iter().find(|d| d.id == id) else { return Err("Download not found".to_string()); };
    let _ = std::fs::remove_file(&item.destination_path);
    update_item(&id, |d| d.status = DownloadStatus::Cancelled);
    Ok(())
}

#[tauri::command]
pub async fn downloader_resume(app: AppHandle, id: String) -> Result<(), String> {
    let items = read_history();
    let item = items.into_iter().find(|d| d.id == id).ok_or("Download not found")?;
    let resume_from = if matches!(item.status, DownloadStatus::Paused { resumable: true }) {
        std::fs::metadata(&item.destination_path).map(|m| m.len()).unwrap_or(0)
    } else {
        // Not resumable (or was Failed/Cancelled) — restart from
        // scratch rather than appending onto a file the server may not
        // actually be able to continue from correctly.
        let _ = std::fs::remove_file(&item.destination_path);
        0
    };
    let pause_requested = Arc::new(AtomicBool::new(false));
    let cancel_requested = Arc::new(AtomicBool::new(false));
    with_running(|r| {
        r.insert(id.clone(), RunningControl { pause_requested: pause_requested.clone(), cancel_requested: cancel_requested.clone() });
    });
    tokio::spawn(run_download(app, item, resume_from, pause_requested, cancel_requested));
    Ok(())
}

/// Removes a download from history (and, if it isn't `Completed`,
/// deletes any partial file on disk — a completed download's actual
/// file is left alone, since "remove from this list" shouldn't delete
/// something the person successfully downloaded and may still want).
#[tauri::command]
pub fn downloader_remove(id: String) -> Result<(), String> {
    let mut items = read_history();
    let Some(pos) = items.iter().position(|d| d.id == id) else { return Ok(()); };
    let item = items.remove(pos);
    if !matches!(item.status, DownloadStatus::Completed) {
        let _ = std::fs::remove_file(&item.destination_path);
    }
    write_history(&items);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_from_url_extracts_last_path_segment() {
        assert_eq!(filename_from_url("https://example.com/files/report.pdf"), "report.pdf");
        assert_eq!(filename_from_url("https://example.com/files/report.pdf?x=1"), "report.pdf");
    }

    #[test]
    fn filename_from_url_falls_back_when_no_path() {
        assert_eq!(filename_from_url("https://example.com"), "download");
        assert_eq!(filename_from_url("https://example.com/"), "download");
    }

    #[test]
    fn parse_content_disposition_extracts_quoted_filename() {
        assert_eq!(
            parse_content_disposition_filename("attachment; filename=\"report (final).pdf\""),
            Some("report (final).pdf".to_string())
        );
    }

    #[test]
    fn parse_content_disposition_extracts_unquoted_filename() {
        assert_eq!(parse_content_disposition_filename("attachment; filename=report.pdf"), Some("report.pdf".to_string()));
    }

    #[test]
    fn parse_content_disposition_none_when_absent() {
        assert_eq!(parse_content_disposition_filename("inline"), None);
    }

    #[test]
    fn unique_path_appends_counter_on_collision() {
        let dir = std::env::temp_dir().join(format!("blue-downloader-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("file.txt"), b"x").unwrap();

        let p = unique_path(&dir, "file.txt");
        assert_eq!(p.file_name().unwrap().to_str().unwrap(), "file (1).txt");

        std::fs::write(&p, b"x").unwrap();
        let p2 = unique_path(&dir, "file.txt");
        assert_eq!(p2.file_name().unwrap().to_str().unwrap(), "file (2).txt");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unique_path_returns_original_when_no_collision() {
        let dir = std::env::temp_dir().join(format!("blue-downloader-test-nocollision-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = unique_path(&dir, "fresh.txt");
        assert_eq!(p.file_name().unwrap().to_str().unwrap(), "fresh.txt");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
