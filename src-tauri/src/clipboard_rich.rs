use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::session::{self, SessionType};

/// Larger images are ignored (a 4K screenshot is ~8 MB as PNG).
const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const MAX_ITEMS: usize = 30;
const MAX_FILES_PER_ITEM: usize = 200;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RichItem {
    pub id: String,
    /// "image" | "files"
    pub kind: String,
    pub timestamp: u64,
    /// image: stored file name (inside the clipboard dir); files: empty
    #[serde(default)]
    pub file_name: String,
    /// image: MIME type
    #[serde(default)]
    pub mime: String,
    #[serde(default)]
    pub size: u64,
    /// files: absolute paths
    #[serde(default)]
    pub paths: Vec<String>,
    /// content hash used for de-duplication
    #[serde(default)]
    pub hash: String,
}

#[derive(Serialize)]
pub struct PollResult {
    /// "text" | "image" | "files" | "empty"
    pub kind: String,
    /// plain text (only when kind == "text")
    pub text: Option<String>,
    /// newly stored (or moved to top) item for image/files
    pub item: Option<RichItem>,
}

// ── pure helpers (unit-tested) ───────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind {
    Files,
    Image,
    Text,
    Empty,
}

/// Decides what the clipboard holds from its advertised MIME types. Files win over images
/// (a file manager also offers a thumbnail), images win over text (browsers offer the
/// alt text next to the picture).
pub fn pick_kind(types: &[String]) -> Kind {
    let has = |pred: &dyn Fn(&str) -> bool| types.iter().any(|t| pred(t.trim()));
    if has(&|t| t == "text/uri-list" || t == "x-special/gnome-copied-files" || t == "application/x-kde-cutselection") {
        return Kind::Files;
    }
    if has(&|t| t.starts_with("image/")) {
        return Kind::Image;
    }
    if has(&|t| t.starts_with("text/") || t == "UTF8_STRING" || t == "STRING" || t == "TEXT") {
        return Kind::Text;
    }
    Kind::Empty
}

/// Best image MIME among the advertised types (PNG preferred).
pub fn pick_image_mime(types: &[String]) -> Option<String> {
    let imgs: Vec<&String> = types.iter().filter(|t| t.trim().starts_with("image/")).collect();
    for pref in ["image/png", "image/jpeg", "image/webp", "image/bmp", "image/gif"] {
        if let Some(t) = imgs.iter().find(|t| t.trim() == pref) {
            return Some(t.trim().to_string());
        }
    }
    imgs.first().map(|t| t.trim().to_string())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = ((b[i + 1] as char).to_digit(16), (b[i + 2] as char).to_digit(16)) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parses `text/uri-list` / `x-special/gnome-copied-files` into absolute local paths.
/// Ignores comments (`#…`), the leading `copy`/`cut` verb of the GNOME format and non-`file://` URIs.
pub fn parse_uri_list(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && *l != "copy" && *l != "cut")
        .filter_map(|l| l.strip_prefix("file://"))
        .map(|rest| {
            // file://host/path → drop the (usually empty) host
            let path = match rest.find('/') {
                Some(0) => rest,
                Some(i) => &rest[i..],
                None => rest,
            };
            percent_decode(path)
        })
        .filter(|p| p.starts_with('/'))
        .take(MAX_FILES_PER_ITEM)
        .collect()
}

/// Builds the `text/uri-list` body (CRLF per RFC 2483) for pasting files back.
pub fn build_uri_list(paths: &[String]) -> String {
    paths
        .iter()
        .map(|p| format!("file://{}", urlencoding::encode(p).replace("%2F", "/")))
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n"
}

fn ext_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/gif" => "gif",
        _ => "png",
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    bytes.len().hash(&mut h);
    format!("{:016x}", h.finish())
}

// ── storage ──────────────────────────────────────────────────────────────────

fn base_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or(PathBuf::from("/tmp"));
    let dir = home.join(".config/Blue-Environment/clipboard");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn index_path() -> PathBuf {
    base_dir().join("clipboard_rich.json")
}

fn load() -> Vec<RichItem> {
    fs::read_to_string(index_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(items: &[RichItem]) {
    if let Ok(s) = serde_json::to_string(items) {
        let _ = fs::write(index_path(), s);
    }
}

fn now_ms() -> u64 {
    chrono::Utc::now().timestamp_millis() as u64
}

/// Inserts `item` at the top; an entry with the same hash is moved instead of duplicated.
/// Evicts the oldest entries (deleting their image files) beyond `MAX_ITEMS`.
fn push(mut items: Vec<RichItem>, item: RichItem) -> Vec<RichItem> {
    items.retain(|i| !(i.kind == item.kind && i.hash == item.hash));
    items.insert(0, item);
    while items.len() > MAX_ITEMS {
        if let Some(old) = items.pop() {
            if !old.file_name.is_empty() {
                let _ = fs::remove_file(base_dir().join(&old.file_name));
            }
        }
    }
    items
}

// ── clipboard access ─────────────────────────────────────────────────────────

fn wayland() -> bool {
    session::detect_session() == SessionType::WaylandClient
}

fn list_types() -> Vec<String> {
    let out = if wayland() {
        Command::new("wl-paste").arg("--list-types").stderr(Stdio::null()).output()
    } else {
        Command::new("xclip").args(["-selection", "clipboard", "-t", "TARGETS", "-o"]).stderr(Stdio::null()).output()
    };
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(),
        _ => Vec::new(),
    }
}

fn read_type(mime: &str) -> Option<Vec<u8>> {
    let out = if wayland() {
        Command::new("wl-paste").args(["--no-newline", "--type", mime]).stderr(Stdio::null()).output()
    } else {
        Command::new("xclip").args(["-selection", "clipboard", "-t", mime, "-o"]).stderr(Stdio::null()).output()
    };
    match out {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => Some(o.stdout),
        _ => None,
    }
}

fn write_type(mime: &str, bytes: &[u8]) -> Result<(), String> {
    let mut cmd = if wayland() {
        let mut c = Command::new("wl-copy");
        c.args(["--type", mime]);
        c
    } else {
        let mut c = Command::new("xclip");
        c.args(["-selection", "clipboard", "-t", mime, "-i"]);
        c
    };
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(bytes).map_err(|e| e.to_string())?;
    }
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
}

// ── commands ─────────────────────────────────────────────────────────────────

/// Called by the clipboard panel's poller. Stores new images/file lists itself; for text it just
/// returns the text so the existing text-history path stays the single source of truth.
#[tauri::command(async)]
pub fn clipboard_poll_rich() -> Result<PollResult, String> {
    let types = list_types();
    match pick_kind(&types) {
        Kind::Empty => Ok(PollResult { kind: "empty".into(), text: None, item: None }),
        Kind::Text => {
            let text = read_type(if wayland() { "text/plain" } else { "UTF8_STRING" })
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .filter(|t| !t.trim().is_empty());
            Ok(PollResult { kind: "text".into(), text, item: None })
        }
        Kind::Files => {
            let mime = types.iter().find(|t| *t == "text/uri-list").cloned()
                .or_else(|| types.iter().find(|t| *t == "x-special/gnome-copied-files").cloned())
                .unwrap_or_else(|| "text/uri-list".into());
            let paths = read_type(&mime).map(|b| parse_uri_list(&String::from_utf8_lossy(&b))).unwrap_or_default();
            if paths.is_empty() {
                return Ok(PollResult { kind: "empty".into(), text: None, item: None });
            }
            let hash = hash_bytes(paths.join("\n").as_bytes());
            let mut items = load();
            // unchanged since last poll → nothing to do
            if items.first().map(|i| i.kind == "files" && i.hash == hash).unwrap_or(false) {
                return Ok(PollResult { kind: "files".into(), text: None, item: None });
            }
            let size = paths.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum();
            let item = RichItem { id: now_ms().to_string(), kind: "files".into(), timestamp: now_ms(), file_name: String::new(), mime: String::new(), size, paths, hash };
            items = push(items, item.clone());
            save(&items);
            Ok(PollResult { kind: "files".into(), text: None, item: Some(item) })
        }
        Kind::Image => {
            let Some(mime) = pick_image_mime(&types) else {
                return Ok(PollResult { kind: "empty".into(), text: None, item: None });
            };
            let Some(bytes) = read_type(&mime) else {
                return Ok(PollResult { kind: "empty".into(), text: None, item: None });
            };
            if bytes.len() > MAX_IMAGE_BYTES {
                return Ok(PollResult { kind: "empty".into(), text: None, item: None });
            }
            let hash = hash_bytes(&bytes);
            let mut items = load();
            if items.first().map(|i| i.kind == "image" && i.hash == hash).unwrap_or(false) {
                return Ok(PollResult { kind: "image".into(), text: None, item: None });
            }
            let file_name = format!("{hash}.{}", ext_for_mime(&mime));
            let path = base_dir().join(&file_name);
            if !path.exists() {
                fs::write(&path, &bytes).map_err(|e| e.to_string())?;
            }
            let item = RichItem { id: now_ms().to_string(), kind: "image".into(), timestamp: now_ms(), file_name, mime, size: bytes.len() as u64, paths: Vec::new(), hash };
            items = push(items, item.clone());
            save(&items);
            Ok(PollResult { kind: "image".into(), text: None, item: Some(item) })
        }
    }
}

#[tauri::command(async)]
pub fn clipboard_rich_history() -> Vec<RichItem> {
    load()
}

/// Data URL of a stored image (for thumbnails/preview in the panel).
#[tauri::command(async)]
pub fn clipboard_rich_image(id: String) -> Result<String, String> {
    let item = load().into_iter().find(|i| i.id == id && i.kind == "image").ok_or("not found")?;
    let bytes = fs::read(base_dir().join(&item.file_name)).map_err(|e| e.to_string())?;
    Ok(format!("data:{};base64,{}", item.mime, B64.encode(bytes)))
}

/// Puts an item back on the system clipboard (image bytes, or the file list as `text/uri-list`).
#[tauri::command(async)]
pub fn clipboard_rich_copy(id: String) -> Result<(), String> {
    let item = load().into_iter().find(|i| i.id == id).ok_or("not found")?;
    match item.kind.as_str() {
        "image" => {
            let bytes = fs::read(base_dir().join(&item.file_name)).map_err(|e| e.to_string())?;
            write_type(&item.mime, &bytes)
        }
        "files" => {
            let existing: Vec<String> = item.paths.iter().filter(|p| std::path::Path::new(p).exists()).cloned().collect();
            if existing.is_empty() {
                return Err("the files no longer exist".into());
            }
            write_type("text/uri-list", build_uri_list(&existing).as_bytes())
        }
        _ => Err("unsupported item".into()),
    }
}

#[tauri::command(async)]
pub fn clipboard_rich_delete(id: String) -> Result<(), String> {
    let mut items = load();
    if let Some(pos) = items.iter().position(|i| i.id == id) {
        let old = items.remove(pos);
        if !old.file_name.is_empty() {
            let _ = fs::remove_file(base_dir().join(&old.file_name));
        }
        save(&items);
    }
    Ok(())
}

#[tauri::command(async)]
pub fn clipboard_rich_clear() -> Result<(), String> {
    for i in load() {
        if !i.file_name.is_empty() {
            let _ = fs::remove_file(base_dir().join(&i.file_name));
        }
    }
    save(&[]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn kind_priority_files_over_image_over_text() {
        assert_eq!(pick_kind(&s(&["text/uri-list", "image/png", "text/plain"])), Kind::Files);
        assert_eq!(pick_kind(&s(&["image/png", "text/html", "text/plain"])), Kind::Image);
        assert_eq!(pick_kind(&s(&["text/plain;charset=utf-8", "UTF8_STRING"])), Kind::Text);
        assert_eq!(pick_kind(&s(&["x-special/gnome-copied-files"])), Kind::Files);
        assert_eq!(pick_kind(&[]), Kind::Empty);
    }

    #[test]
    fn prefers_png() {
        assert_eq!(pick_image_mime(&s(&["image/jpeg", "image/png"])), Some("image/png".into()));
        assert_eq!(pick_image_mime(&s(&["image/x-foo"])), Some("image/x-foo".into()));
        assert_eq!(pick_image_mime(&s(&["text/plain"])), None);
    }

    #[test]
    fn parses_uri_lists() {
        let l = parse_uri_list("# comment\r\nfile:///home/u/My%20Doc.txt\r\nfile://localhost/tmp/a.png\r\nhttps://x.y/z\r\n");
        assert_eq!(l, vec!["/home/u/My Doc.txt".to_string(), "/tmp/a.png".to_string()]);
    }

    #[test]
    fn parses_gnome_copied_files() {
        let l = parse_uri_list("copy\nfile:///a/b.txt\nfile:///c%C3%B3.txt");
        assert_eq!(l, vec!["/a/b.txt".to_string(), "/có.txt".to_string()]);
    }

    #[test]
    fn uri_list_round_trips() {
        let paths = vec!["/home/u/My Doc.txt".to_string(), "/tmp/ó.png".to_string()];
        let body = build_uri_list(&paths);
        assert!(body.contains("file:///home/u/My%20Doc.txt\r\n"));
        assert_eq!(parse_uri_list(&body), paths);
    }

    #[test]
    fn percent_decode_edge_cases() {
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%41"), "A");
    }
}
