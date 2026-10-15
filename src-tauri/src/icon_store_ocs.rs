use crate::icon_store;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures_util::StreamExt;
use regex::Regex;
use reqwest::Url;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const OCS_BASE: &str = "https://api.kde-look.org/ocs/v1";
/// "Full Icon Themes" on the KDE Store.
const ICON_CATEGORY: &str = "132";
const DEFAULT_PAGE_SIZE: u32 = 24;
const MAX_PAGE_SIZE: u32 = 48;
const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
const MAX_THUMB_BYTES: usize = 4 * 1024 * 1024;
const MAX_DOWNLOAD_BYTES: u64 = 800 * 1024 * 1024; // keep in step with icon_store::MAX_ARCHIVE_BYTES
const PROGRESS_EVENT: &str = "icon-store-progress";

/// Hosts the *catalogue* and its *images* may come from.
const STORE_HOST_SUFFIXES: [&str; 5] = ["pling.com", "opendesktop.org", "kde-look.org", "kde.org", "gnome-look.org"];

// ── data model ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoreDownload {
    pub index: u32,
    pub name: String,
    pub url: String,
    pub size_kb: Option<u64>,
    /// Can Blue unpack it as an icon theme (by file name)?
    pub is_archive: bool,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoreIcon {
    pub id: String,
    pub name: String,
    pub author: String,
    pub summary: String,
    pub description: String,
    pub version: String,
    pub thumb: String,
    pub previews: Vec<String>,
    pub downloads_count: u64,
    /// 0–100 as the store reports it.
    pub score: u32,
    pub changed: String,
    pub detail_url: String,
    pub downloads: Vec<StoreDownload>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StorePage {
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
    pub items: Vec<StoreIcon>,
}

// ── pure helpers (unit-tested) ──────────────────────────────────────────

fn host_matches(host: &str, suffixes: &[&str]) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    suffixes.iter().any(|s| host == *s || host.ends_with(&format!(".{s}")))
}

/// `https` only; upgrades `http` since the store serves everything over TLS.
fn https_url(raw: &str) -> Option<Url> {
    let raw = raw.trim();
    let fixed = if let Some(rest) = raw.strip_prefix("http://") { format!("https://{rest}") } else { raw.to_string() };
    let url = Url::parse(&fixed).ok()?;
    (url.scheme() == "https" && url.host_str().is_some()).then_some(url)
}

fn is_store_url(raw: &str) -> Option<Url> {
    let u = https_url(raw)?;
    host_matches(u.host_str()?, &STORE_HOST_SUFFIXES).then_some(u)
}

/// Download links may legitimately point at a CDN or an external host, so they
/// are not restricted to store domains — but never to IP literals, localhost
/// or intranet-style names (the request is made from the user's machine).
fn is_public_https(url: &Url) -> bool {
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else { return false };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.parse::<IpAddr>().is_ok() || host.starts_with('[') {
        return false;
    }
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") || host.ends_with(".internal") || host.ends_with(".lan") {
        return false;
    }
    host.contains('.')
}

fn sort_mode(sort: &str) -> &'static str {
    match sort {
        "top" | "high" => "high",
        "downloads" | "down" => "down",
        "name" | "alpha" => "alpha",
        _ => "new",
    }
}

fn field(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn num(v: &Value, key: &str) -> u64 {
    field(v, key).parse::<f64>().ok().filter(|n| *n >= 0.0).map(|n| n as u64).unwrap_or(0)
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("static regex"))
}

/// Store descriptions are a mix of HTML and BBCode. The UI shows plain text.
pub(crate) fn plain_text(input: &str) -> String {
    static BR: OnceLock<Regex> = OnceLock::new();
    static TAGS: OnceLock<Regex> = OnceLock::new();
    static BB: OnceLock<Regex> = OnceLock::new();
    static BLANKS: OnceLock<Regex> = OnceLock::new();
    let s = re(&BR, r"(?i)<\s*br\s*/?\s*>|</\s*(p|div|li|h[1-6])\s*>").replace_all(input, "\n");
    let s = re(&TAGS, r"<[^>]*>").replace_all(&s, "");
    let s = re(&BB, r"(?i)\[/?(b|i|u|s|url|img|code|quote|list|size|color|center|\*)(=[^\]]*)?\]").replace_all(&s, "");
    let s = s
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&");
    let s = re(&BLANKS, r"\n{3,}").replace_all(s.trim(), "\n\n").to_string();
    s.chars().take(4000).collect()
}

fn file_name_from_url(url: &Url) -> Option<String> {
    for (k, v) in url.query_pairs() {
        if k == "file_name" && !v.is_empty() {
            return sanitize_file_name(&v);
        }
    }
    url.path_segments()?.next_back().filter(|s| !s.is_empty()).and_then(|s| sanitize_file_name(&urlencoding::decode(s).ok()?))
}

/// Base name only, no control characters, bounded length.
fn sanitize_file_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("").trim().trim_start_matches('.');
    let clean: String = base.chars().filter(|c| !c.is_control()).collect();
    (!clean.is_empty() && clean.chars().count() <= 200).then_some(clean)
}

fn filename_from_disposition(header: &str) -> Option<String> {
    // RFC 5987 form first: filename*=UTF-8''Foo%20Bar.tar.gz
    for part in header.split(';').map(str::trim) {
        if let Some(rest) = part.strip_prefix("filename*=") {
            let value = rest.rsplit('\'').next().unwrap_or(rest);
            if let Ok(dec) = urlencoding::decode(value) {
                if let Some(n) = sanitize_file_name(&dec) {
                    return Some(n);
                }
            }
        }
    }
    for part in header.split(';').map(str::trim) {
        if let Some(rest) = part.strip_prefix("filename=") {
            if let Some(n) = sanitize_file_name(rest.trim_matches('"')) {
                return Some(n);
            }
        }
    }
    None
}

/// Archive type from the first bytes — the file name on the store is often
/// wrong (`.tar.gz` that is really an `.xz`), and the unpacker picks its tool
/// by extension, so trust the content.
fn sniff_extension(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        Some(".zip")
    } else if head.starts_with(&[0x1f, 0x8b]) {
        Some(".tar.gz")
    } else if head.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00]) {
        Some(".tar.xz")
    } else if head.starts_with(b"BZh") {
        Some(".tar.bz2")
    } else if head.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        Some(".tar.zst")
    } else if head.len() >= 262 && &head[257..262] == b"ustar" {
        Some(".tar")
    } else {
        None
    }
}

fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else {
        None // SVG deliberately excluded
    }
}

/// `"Foo Icons.tar.gz"` → `"Foo Icons.tar.gz"`; replaces the extension with the sniffed one.
fn with_archive_extension(name: &str, sniffed: Option<&str>) -> String {
    let Some(ext) = sniffed else { return name.to_string() };
    let lower = name.to_lowercase();
    for known in [".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst", ".tgz", ".txz", ".tbz2", ".tbz", ".tzst", ".tar", ".zip"] {
        if lower.ends_with(known) {
            let stem = &name[..name.len() - known.len()];
            return format!("{stem}{ext}");
        }
    }
    format!("{name}{ext}")
}

fn parse_item(v: &Value) -> Option<StoreIcon> {
    let id = field(v, "id");
    let name = field(v, "name");
    if id.is_empty() || name.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut previews = Vec::new();
    let mut thumb = String::new();
    for i in 1..=8 {
        let big = field(v, &format!("previewpic{i}"));
        let small = field(v, &format!("smallpreviewpic{i}"));
        if let Some(u) = is_store_url(&big) {
            previews.push(u.to_string());
        }
        if thumb.is_empty() {
            if let Some(u) = is_store_url(&small).or_else(|| is_store_url(&big)) {
                thumb = u.to_string();
            }
        }
    }
    let mut downloads = Vec::new();
    for i in 1..=12u32 {
        let link = field(v, &format!("downloadlink{i}"));
        let Some(url) = https_url(&link).filter(is_public_https) else { continue };
        let name = Some(field(v, &format!("downloadname{i}")))
            .filter(|n| !n.is_empty())
            .and_then(|n| sanitize_file_name(&n))
            .or_else(|| file_name_from_url(&url))
            .unwrap_or_else(|| format!("download-{i}"));
        let size_kb = Some(num(v, &format!("downloadsize{i}"))).filter(|s| *s > 0);
        downloads.push(StoreDownload { index: i, is_archive: icon_store::is_supported_archive_name(&name), name, url: url.to_string(), size_kb });
    }
    // Real icon-theme archives first, then the rest, each in the store's order.
    downloads.sort_by_key(|d| !d.is_archive);
    let detail_url = is_store_url(&field(v, "detailpage")).map(|u| u.to_string()).unwrap_or_else(|| format!("https://store.kde.org/p/{id}"));
    Some(StoreIcon {
        author: field(v, "personid"),
        summary: plain_text(&field(v, "summary")),
        description: plain_text(&field(v, "description")),
        version: field(v, "version"),
        thumb,
        previews,
        downloads_count: num(v, "downloads"),
        score: num(v, "score").min(100) as u32,
        changed: field(v, "changed"),
        detail_url,
        downloads,
        id,
        name,
    })
}

/// OCS answers differ slightly between servers/versions: `data` is an array,
/// or `data.content` is an array/object, sometimes wrapped in `ocs`.
fn extract_items(root: &Value) -> (Vec<&Value>, Option<u64>) {
    let base = root.get("ocs").unwrap_or(root);
    let total = base
        .get("totalitems")
        .or_else(|| base.get("meta").and_then(|m| m.get("totalitems")))
        .and_then(|t| t.as_u64().or_else(|| t.as_str().and_then(|s| s.parse().ok())));
    let items = match base.get("data") {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(obj @ Value::Object(o)) => match o.get("content") {
            Some(Value::Array(a)) => a.iter().collect(),
            Some(c @ Value::Object(_)) => vec![c],
            _ => vec![obj],
        },
        _ => Vec::new(),
    };
    (items, total)
}

fn ocs_status_error(root: &Value) -> Option<String> {
    let base = root.get("ocs").unwrap_or(root);
    let status = base.get("status").or_else(|| base.get("meta").and_then(|m| m.get("status"))).and_then(|s| s.as_str())?;
    if status.eq_ignore_ascii_case("ok") {
        return None;
    }
    let msg = base.get("message").or_else(|| base.get("meta").and_then(|m| m.get("message"))).and_then(|m| m.as_str()).unwrap_or("");
    Some(format!("The KDE Store answered '{status}' {msg}").trim().to_string())
}

// ── network ─────────────────────────────────────────────────────────────

fn client(total_timeout: Duration, downloads: bool) -> Result<reqwest::Client, String> {
    let policy = reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= 8 {
            attempt.error("too many redirects")
        } else if downloads && !is_public_https(attempt.url()) {
            attempt.error("redirect to a disallowed location")
        } else if !downloads && !(attempt.url().scheme() == "https" && attempt.url().host_str().map(|h| host_matches(h, &STORE_HOST_SUFFIXES)).unwrap_or(false)) {
            attempt.error("redirect away from the KDE Store")
        } else {
            attempt.follow()
        }
    });
    reqwest::Client::builder()
        .user_agent(concat!("Blue-Environment/", env!("CARGO_PKG_VERSION"), " (blue-software; icon-store)"))
        .connect_timeout(Duration::from_secs(12))
        .timeout(total_timeout)
        .redirect(policy)
        .build()
        .map_err(|e| format!("Could not initialise the HTTP client: {e}"))
}

fn net_err(what: &str, e: reqwest::Error) -> String {
    if e.is_timeout() {
        format!("{what}: the server took too long to answer")
    } else if e.is_connect() {
        format!("{what}: could not connect — check your internet connection")
    } else {
        format!("{what}: {e}")
    }
}

async fn read_limited(resp: reqwest::Response, max: usize, what: &str) -> Result<Vec<u8>, String> {
    if resp.content_length().map(|l| l as usize > max).unwrap_or(false) {
        return Err(format!("{what}: the response is unreasonably large"));
    }
    let mut buf = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| net_err(what, e))?;
        if buf.len() + chunk.len() > max {
            return Err(format!("{what}: the response is unreasonably large"));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

async fn get_json(url: Url) -> Result<Value, String> {
    let resp = client(Duration::from_secs(30), false)?.get(url).header("Accept", "application/json").send().await.map_err(|e| net_err("KDE Store", e))?;
    if !resp.status().is_success() {
        return Err(format!("The KDE Store answered HTTP {}", resp.status().as_u16()));
    }
    let bytes = read_limited(resp, MAX_JSON_BYTES, "KDE Store").await?;
    let root: Value = serde_json::from_slice(&bytes).map_err(|_| "The KDE Store returned something that is not JSON".to_string())?;
    if let Some(err) = ocs_status_error(&root) {
        return Err(err);
    }
    Ok(root)
}

// ── commands: catalogue ─────────────────────────────────────────────────

/// One page of icon themes. `sort`: `new` | `top` | `downloads` | `name`.
#[tauri::command]
pub async fn icon_store_search(query: String, sort: String, page: u32, page_size: Option<u32>) -> Result<StorePage, String> {
    let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    let page = page.min(10_000);
    let mut url = Url::parse(&format!("{OCS_BASE}/content/data")).map_err(|e| e.to_string())?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("categories", ICON_CATEGORY);
        let search: String = query.trim().chars().take(100).collect();
        if !search.is_empty() {
            q.append_pair("search", &search);
        }
        q.append_pair("sortmode", sort_mode(&sort));
        q.append_pair("page", &page.to_string());
        q.append_pair("pagesize", &page_size.to_string());
        q.append_pair("format", "json");
    }
    let root = get_json(url).await?;
    let (raw, total) = extract_items(&root);
    let items: Vec<StoreIcon> = raw.into_iter().filter_map(parse_item).collect();
    let has_more = match total {
        Some(t) => (u64::from(page) + 1) * u64::from(page_size) < t,
        None => items.len() as u32 >= page_size,
    };
    Ok(StorePage { total: total.unwrap_or(items.len() as u64), page, page_size, has_more, items })
}

/// A single item with its full file list (list responses may carry fewer fields).
#[tauri::command]
pub async fn icon_store_details(id: String) -> Result<StoreIcon, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err("Invalid item id".into());
    }
    let mut url = Url::parse(&format!("{OCS_BASE}/content/data/{id}")).map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("format", "json");
    let root = get_json(url).await?;
    let (raw, _) = extract_items(&root);
    raw.into_iter().find_map(parse_item).ok_or_else(|| "The KDE Store has no such item".to_string())
}

// ── commands: thumbnails ────────────────────────────────────────────────

fn root_cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".cache")).join("Blue-Environment").join("icon-store")
}

fn thumb_cache_path(url: &Url) -> PathBuf {
    let digest = Sha256::digest(url.as_str().as_bytes());
    let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
    root_cache_dir().join("thumbs").join(format!("{hex}.img"))
}

fn to_data_url(bytes: &[u8]) -> Option<String> {
    let mime = sniff_image_mime(bytes)?;
    Some(format!("data:{mime};base64,{}", B64.encode(bytes)))
}

/// A preview image as a `data:` URL. Cached on disk, so scrolling back and
/// reopening the store costs no network.
#[tauri::command]
pub async fn icon_store_thumbnail(url: String) -> Result<String, String> {
    let url = is_store_url(&url).ok_or_else(|| "Image host not allowed".to_string())?;
    let cache = thumb_cache_path(&url);
    if let Ok(bytes) = fs::read(&cache) {
        if let Some(d) = to_data_url(&bytes) {
            return Ok(d);
        }
    }
    let resp = client(Duration::from_secs(30), false)?.get(url.clone()).send().await.map_err(|e| net_err("Image", e))?;
    if !resp.status().is_success() {
        return Err(format!("Image: HTTP {}", resp.status().as_u16()));
    }
    let bytes = read_limited(resp, MAX_THUMB_BYTES, "Image").await?;
    let data = to_data_url(&bytes).ok_or_else(|| "Unsupported image format".to_string())?;
    if let Some(parent) = cache.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&cache, &bytes);
    Ok(data)
}

// ── commands: download + install ────────────────────────────────────────

fn registry_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".config/Blue-Environment/icon-store-installed.json")
}

fn read_registry() -> HashMap<String, Vec<String>> {
    fs::read_to_string(registry_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write_registry(map: &HashMap<String, Vec<String>>) {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = fs::write(path, json);
    }
}

/// Store item id → theme folder names it installed. Entries whose folders are
/// gone (removed by hand, or via the Installed list) are dropped, so the ✓
/// badge never lies.
#[tauri::command(async)]
pub fn icon_store_installed_map() -> HashMap<String, Vec<String>> {
    let roots = icon_store::user_icon_roots();
    let mut map = read_registry();
    let before = map.len();
    map.retain(|_, themes| {
        themes.retain(|t| roots.iter().any(|r| r.join(t).join("index.theme").exists()));
        !themes.is_empty()
    });
    if map.len() != before {
        write_registry(&map);
    }
    map
}

fn emit_progress(app: &AppHandle, id: &str, phase: &str, received: u64, total: Option<u64>) {
    let _ = app.emit(PROGRESS_EVENT, serde_json::json!({ "id": id, "phase": phase, "received": received, "total": total }));
}

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn unique_name() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{}-{nanos}", std::process::id())
}

/// Downloads the chosen file from the store and installs it as an icon theme
/// for the current user (`~/.local/share/icons`). Emits `icon-store-progress`
/// events (`{ id, phase: "download" | "install", received, total }`).
/// Resolves to the installed theme folder names.
#[tauri::command]
pub async fn icon_store_install_remote(app: AppHandle, content_id: String, url: String, file_name: Option<String>) -> Result<Vec<String>, String> {
    if content_id.is_empty() || !content_id.chars().all(|c| c.is_ascii_digit()) {
        return Err("Invalid item id".into());
    }
    let url = https_url(&url).filter(is_public_https).ok_or_else(|| "The download link is not a safe https address".to_string())?;

    let resp = client(Duration::from_secs(1800), true)?.get(url.clone()).send().await.map_err(|e| net_err("Download", e))?;
    if !resp.status().is_success() {
        return Err(format!("Download: the server answered HTTP {}", resp.status().as_u16()));
    }
    if resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(|t| t.to_ascii_lowercase().starts_with("text/html")).unwrap_or(false) {
        return Err("The store sent a web page instead of the archive (the file may require a login or was removed)".into());
    }
    let total = resp.content_length().filter(|t| *t > 0);
    if total.map(|t| t > MAX_DOWNLOAD_BYTES).unwrap_or(false) {
        return Err("The archive is larger than 800 MB".into());
    }
    let suggested = resp
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(filename_from_disposition)
        .or_else(|| file_name.as_deref().and_then(sanitize_file_name))
        .or_else(|| file_name_from_url(resp.url()))
        .unwrap_or_else(|| "icon-theme".to_string());

    let tmp = TempDir(root_cache_dir().join("dl").join(unique_name()));
    fs::create_dir_all(&tmp.0).map_err(|e| format!("Could not create a temporary folder: {e}"))?;
    let part = tmp.0.join("download.part");
    {
        let mut file = fs::File::create(&part).map_err(|e| format!("Could not write the download: {e}"))?;
        let mut received: u64 = 0;
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        emit_progress(&app, &content_id, "download", 0, total);
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| net_err("Download", e))?;
            received += chunk.len() as u64;
            if received > MAX_DOWNLOAD_BYTES {
                return Err("The archive is larger than 800 MB".into());
            }
            file.write_all(&chunk).map_err(|e| format!("Could not write the download: {e}"))?;
            if last_emit.elapsed() >= Duration::from_millis(120) {
                last_emit = Instant::now();
                emit_progress(&app, &content_id, "download", received, total);
            }
        }
        file.flush().map_err(|e| e.to_string())?;
        emit_progress(&app, &content_id, "download", received, total.or(Some(received)));
    }

    // Trust the content over the name.
    let mut head = [0u8; 512];
    let n = {
        use std::io::Read;
        fs::File::open(&part).and_then(|mut f| f.read(&mut head)).map_err(|e| e.to_string())?
    };
    let sniffed = sniff_extension(&head[..n]);
    if sniffed.is_none() && !icon_store::is_supported_archive_name(&suggested) {
        return Err("This file is not an archive Blue can unpack (expected .tar.gz, .tar.xz, .tar.bz2, .zip)".into());
    }
    let archive = tmp.0.join(with_archive_extension(&suggested, sniffed));
    fs::rename(&part, &archive).map_err(|e| e.to_string())?;

    emit_progress(&app, &content_id, "install", 0, None);
    let archive_for_task = archive.clone();
    let names = tokio::task::spawn_blocking(move || icon_store::install_into_user_icons(&archive_for_task))
        .await
        .map_err(|e| e.to_string())??;

    let mut map = read_registry();
    map.insert(content_id.clone(), names.clone());
    write_registry(&map);
    emit_progress(&app, &content_id, "done", 0, None);
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_item() -> Value {
        json!({
            "id": "1234567", "name": "Fancy Icons", "personid": "someartist", "version": "2.1",
            "summary": "Flat <b>icons</b>", "description": "Line one<br/>Line two &amp; [b]bold[/b]",
            "downloads": "4210", "score": "87", "changed": "2024-05-01 10:00:00",
            "detailpage": "https://store.kde.org/p/1234567",
            "previewpic1": "https://images.pling.com/img/00/00/1/big.png",
            "smallpreviewpic1": "http://images.pling.com/img/00/00/1/small.png",
            "downloadlink1": "https://www.pling.com/p/1234567/startdownload?file_id=1&file_name=Fancy.tar.xz&file_type=application/x-xz&file_size=999",
            "downloadname1": "Fancy.tar.xz", "downloadsize1": "999",
            "downloadlink2": "https://dl.example.org/readme.pdf", "downloadname2": "readme.pdf"
        })
    }

    #[test]
    fn parses_an_ocs_item() {
        let it = parse_item(&sample_item()).unwrap();
        assert_eq!(it.id, "1234567");
        assert_eq!(it.name, "Fancy Icons");
        assert_eq!(it.author, "someartist");
        assert_eq!(it.downloads_count, 4210);
        assert_eq!(it.score, 87);
        assert_eq!(it.summary, "Flat icons");
        assert_eq!(it.description, "Line one\nLine two & bold");
        assert_eq!(it.thumb, "https://images.pling.com/img/00/00/1/small.png", "http upgraded to https");
        assert_eq!(it.downloads.len(), 2);
        assert!(it.downloads[0].is_archive && it.downloads[0].name == "Fancy.tar.xz" && it.downloads[0].size_kb == Some(999));
        assert!(!it.downloads[1].is_archive, "archives are listed first, the pdf last");
    }

    #[test]
    fn rejects_items_without_numeric_id_or_name() {
        assert!(parse_item(&json!({"name":"x"})).is_none());
        assert!(parse_item(&json!({"id":"../etc","name":"x"})).is_none());
        assert!(parse_item(&json!({"id":"5"})).is_none());
    }

    #[test]
    fn ignores_unsafe_download_links_and_foreign_images() {
        let it = parse_item(&json!({
            "id": "9", "name": "N",
            "previewpic1": "https://evil.example.net/x.png",
            "downloadlink1": "https://127.0.0.1/a.zip",
            "downloadlink2": "http://localhost/a.zip",
            "downloadlink3": "file:///etc/passwd",
            "downloadlink4": "https://intranet.local/a.zip",
            "downloadlink5": "https://cdn.example.org/a.zip"
        })).unwrap();
        assert!(it.thumb.is_empty() && it.previews.is_empty());
        assert_eq!(it.downloads.len(), 1);
        assert_eq!(it.downloads[0].url, "https://cdn.example.org/a.zip");
    }

    #[test]
    fn extracts_items_from_every_known_response_shape() {
        let a = json!({"status":"ok","totalitems":2,"data":[{"id":"1","name":"a"},{"id":"2","name":"b"}]});
        let (items, total) = extract_items(&a);
        assert_eq!((items.len(), total), (2, Some(2)));
        let b = json!({"ocs":{"meta":{"status":"ok","totalitems":"7"},"data":{"content":[{"id":"1","name":"a"}]}}});
        let (items, total) = extract_items(&b);
        assert_eq!((items.len(), total), (1, Some(7)));
        let c = json!({"status":"ok","data":{"content":{"id":"1","name":"a"}}});
        assert_eq!(extract_items(&c).0.len(), 1);
        let d = json!({"status":"ok","data":{"id":"1","name":"a"}});
        assert_eq!(extract_items(&d).0.len(), 1);
        assert!(extract_items(&json!({"status":"ok"})).0.is_empty());
    }

    #[test]
    fn reports_ocs_errors() {
        assert!(ocs_status_error(&json!({"status":"ok"})).is_none());
        assert!(ocs_status_error(&json!({"status":"failed","message":"nope"})).unwrap().contains("nope"));
    }

    #[test]
    fn sort_modes() {
        assert_eq!(sort_mode("top"), "high");
        assert_eq!(sort_mode("downloads"), "down");
        assert_eq!(sort_mode("name"), "alpha");
        assert_eq!(sort_mode("whatever"), "new");
    }

    #[test]
    fn host_rules() {
        assert!(is_store_url("https://images.pling.com/a.png").is_some());
        assert!(is_store_url("http://cdn.pling.com/a.png").is_some());
        assert!(is_store_url("https://store.kde.org/p/1").is_some());
        assert!(is_store_url("https://notpling.com/a.png").is_none());
        assert!(is_store_url("https://pling.com.evil.net/a.png").is_none());
        assert!(is_store_url("ftp://pling.com/a").is_none());
        for bad in ["https://10.0.0.1/a", "https://localhost/a", "https://[::1]/a", "https://nodots/a", "ftp://x.org/a"] {
            let ok = https_url(bad).map(|u| is_public_https(&u)).unwrap_or(false);
            assert!(!ok, "{bad}");
        }
        // plain http is upgraded, not refused
        assert_eq!(https_url("http://x.org/a").unwrap().scheme(), "https");
    }

    #[test]
    fn file_names() {
        assert_eq!(filename_from_disposition("attachment; filename=\"Foo Bar.tar.gz\"").as_deref(), Some("Foo Bar.tar.gz"));
        assert_eq!(filename_from_disposition("attachment; filename*=UTF-8''Foo%20Bar.zip").as_deref(), Some("Foo Bar.zip"));
        assert_eq!(filename_from_disposition("attachment; filename=\"../../etc/passwd\"").as_deref(), Some("passwd"));
        assert_eq!(filename_from_disposition("inline"), None);
        assert_eq!(sanitize_file_name("..hidden").as_deref(), Some("hidden"));
        let u = Url::parse("https://x.org/p/1/startdownload?file_id=3&file_name=Th%20eme.tar.xz").unwrap();
        assert_eq!(file_name_from_url(&u).as_deref(), Some("Th eme.tar.xz"));
        let u = Url::parse("https://x.org/dl/Theme.zip").unwrap();
        assert_eq!(file_name_from_url(&u).as_deref(), Some("Theme.zip"));
    }

    #[test]
    fn sniffs_archives_by_content() {
        assert_eq!(sniff_extension(b"PK\x03\x04rest"), Some(".zip"));
        assert_eq!(sniff_extension(&[0x1f, 0x8b, 8, 0]), Some(".tar.gz"));
        assert_eq!(sniff_extension(&[0xfd, b'7', b'z', b'X', b'Z', 0, 1]), Some(".tar.xz"));
        assert_eq!(sniff_extension(b"BZh91AY"), Some(".tar.bz2"));
        assert_eq!(sniff_extension(&[0x28, 0xb5, 0x2f, 0xfd, 0]), Some(".tar.zst"));
        let mut tar = vec![0u8; 512];
        tar[257..262].copy_from_slice(b"ustar");
        assert_eq!(sniff_extension(&tar), Some(".tar"));
        assert_eq!(sniff_extension(b"<!DOCTYPE html>"), None);
    }

    #[test]
    fn corrects_the_extension_to_match_content() {
        assert_eq!(with_archive_extension("Foo.tar.gz", Some(".tar.xz")), "Foo.tar.xz");
        assert_eq!(with_archive_extension("Foo.zip", Some(".tar.gz")), "Foo.tar.gz");
        assert_eq!(with_archive_extension("Foo", Some(".zip")), "Foo.zip");
        assert_eq!(with_archive_extension("Foo.tar.gz", None), "Foo.tar.gz");
    }

    #[test]
    fn images_are_recognised_but_svg_is_not() {
        assert_eq!(sniff_image_mime(&[0x89, b'P', b'N', b'G', 0]), Some("image/png"));
        assert_eq!(sniff_image_mime(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(sniff_image_mime(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"), None);
        assert!(to_data_url(&[0x89, b'P', b'N', b'G']).unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn plain_text_strips_markup() {
        assert_eq!(plain_text("<p>Hi</p><p>there</p>"), "Hi\nthere");
        assert_eq!(plain_text("[url=http://x]link[/url] &lt;ok&gt;"), "link <ok>");
        assert_eq!(plain_text("a\n\n\n\n\nb"), "a\n\nb");
        assert!(plain_text(&"x".repeat(10_000)).chars().count() <= 4000);
    }
}
