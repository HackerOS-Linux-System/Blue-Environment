use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub mod trash;
pub mod transfer;

// ── FileEntry ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: String,
    pub mime_type: String,
    pub modified: Option<String>,
}

fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png"                                             => "image/png",
        "jpg"|"jpeg"                                       => "image/jpeg",
        "gif"                                               => "image/gif",
        "webp"                                              => "image/webp",
        "svg"                                               => "image/svg+xml",
        "avif"                                              => "image/avif",
        "bmp"                                               => "image/bmp",
        "mp4"|"m4v"                                         => "video/mp4",
        "mkv"                                               => "video/x-matroska",
        "webm"                                              => "video/webm",
        "avi"                                               => "video/x-msvideo",
        "mov"                                               => "video/quicktime",
        "flv"                                               => "video/x-flv",
        "wmv"                                               => "video/x-ms-wmv",
        "mp3"                                               => "audio/mpeg",
        "wav"                                               => "audio/wav",
        "ogg"                                               => "audio/ogg",
        "flac"                                              => "audio/flac",
        "aac"                                               => "audio/aac",
        "opus"                                              => "audio/opus",
        "pdf"                                               => "application/pdf",
        "json"                                              => "application/json",
        "xml"                                               => "application/xml",
        "zip"                                               => "application/zip",
        "tar"                                               => "application/x-tar",
        "gz"|"tgz"                                          => "application/gzip",
        "txt"|"md"|"rs"|"ts"|"js"|"tsx"|"jsx"|"py"|"rb"|
        "go"|"toml"|"yaml"|"yml"|"sh"|"css"|
        "html"|"ini"|"conf"|"log"                           => "text/plain",
        _                                                   => "application/octet-stream",
    }
}

// ── Path resolution ─────────────────────────────────────────────────────────

/// Resolves paths that start with the `HOME` sentinel used throughout
/// the frontend's default config (e.g. `"HOME/Desktop"`) or `~`.
pub fn resolve_path(path: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    if path == "HOME" {
        home
    } else if let Some(rest) = path.strip_prefix("HOME/") {
        home.join(rest)
    } else if path == "~" {
        home
    } else if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(path)
    }
}

// ── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command(async)]
pub fn list_files(path: String) -> Result<Vec<FileEntry>, String> {
    let target = resolve_path(&path);

    // Distinguish "genuinely empty folder" from "couldn't be read" — a bare
    // empty Vec looked identical to a real error in the UI, which made the
    // whole Explorer main pane appear permanently blank (only the static
    // Places sidebar was ever visible) whenever a folder didn't exist yet
    // or wasn't readable.
    if !target.exists() {
        return Err(format!("Path does not exist: {}", target.display()));
    }
    if !target.is_dir() {
        return Err(format!("Not a directory: {}", target.display()));
    }

    let rd = fs::read_dir(&target)
        .map_err(|e| format!("Cannot read {}: {}", target.display(), e))?;

    let mut entries = Vec::new();
    for res in rd {
        // Skip individual entries that fail to stat (e.g. broken symlinks)
        // instead of dropping the whole listing.
        let entry = match res { Ok(e) => e, Err(_) => continue };

        // `DirEntry::metadata()` deliberately does NOT follow symlinks (it's
        // equivalent to `symlink_metadata`) — so on any filesystem layout
        // where top-level dirs are symlinks (ostree-based systems like
        // Fedora Silverblue/HackerOS symlink /bin, /lib, /lib64, /sbin,
        // /home, /media, /mnt, /opt, /root, /srv into /usr or /var), every
        // one of those showed up here as a plain 0-byte "file" instead of a
        // folder — roughly half the entries in a typical root listing,
        // which is exactly the bug: they couldn't be opened/navigated into
        // from the Files app. Resolve through the symlink first
        // (`fs::metadata` follows it) and only fall back to the raw
        // (unresolved) metadata for genuinely broken symlinks, so a
        // dangling link still shows up as *something* rather than
        // vanishing from the listing entirely.
        let resolved = fs::metadata(entry.path());
        let meta = match resolved.or_else(|_| entry.metadata()) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = meta.is_dir();
        let size = if is_dir { "DIR".to_string() } else {
            format!("{:.1} KB", meta.len() as f64 / 1024.0)
        };
        let ext = entry.path().extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let mime_type = if is_dir { "inode/directory".to_string() } else { mime_for_ext(&ext).to_string() };
        let modified = meta.modified().ok().map(|t| {
            chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M").to_string()
        });
        entries.push(FileEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir, size, mime_type, modified,
        });
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

/// BUGFIX (reported: opening/editing files inside a real project folder
/// "doesn't work"): this used to return a plain `String` and, on any I/O
/// error, silently returned `format!("Error: {}", e)` AS IF it were the
/// file's content — so Blue Code (and Notepad) couldn't tell a failure
/// from a real file, opened the tab anyway, and would happily overwrite
/// the real file with that literal error text on the next save.
///
/// A flat folder of plain `.txt` files never hits this, but a real
/// project directory almost always contains at least one file this old
/// code path choked on:
///   - non-UTF-8 / binary files (images, compiled `.node`/`.so`, lockfile
///     quirks, `.git/objects/**`) — `read_to_string` errors on these by
///     design, since they aren't text;
///   - broken or permission-restricted symlinks, common under
///     `node_modules/.bin` and pnpm/monorepo layouts;
///   - files owned by another user / read-only mounts.
/// Every one of those now surfaces as a REAL error the frontend can
/// distinguish from content, instead of corrupting the file on save.
/// Also resolves the `HOME/...`/`~` sentinel like every other command
/// here (`list_files`, `create_folder`, ...) — `read_text_file` and
/// `write_text_file` were the only two file commands that skipped
/// `resolve_path`, so a path built from that sentinel elsewhere (e.g. a
/// future caller mirroring Explorer's own convention) would have failed
/// even though the exact same string works for every other command.
#[tauri::command(async)]
pub fn read_text_file(path: String) -> Result<String, String> {
    let target = resolve_path(&path);
    if !target.exists() {
        return Err(format!("File not found: {}", target.display()));
    }
    if target.is_dir() {
        return Err(format!("\"{}\" is a folder, not a file", target.display()));
    }
    fs::read_to_string(&target).map_err(|e| {
        if e.kind() == std::io::ErrorKind::InvalidData {
            format!(
                "\"{}\" is not a text file (it contains binary data) — Blue Code and Notepad can only open plain text.",
                target.display()
            )
        } else {
            format!("Could not read \"{}\": {}", target.display(), e)
        }
    })
}

#[tauri::command(async)]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    let target = resolve_path(&path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create \"{}\": {}", parent.display(), e))?;
    }
    fs::write(&target, content).map_err(|e| format!("Could not save \"{}\": {}", target.display(), e))
}

#[tauri::command(async)]
pub fn create_folder(path: String, name: String) -> Result<(), String> {
    fs::create_dir_all(resolve_path(&path).join(name)).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn delete_file(path: String) -> Result<(), String> {
    let p = resolve_path(&path);
    if p.is_dir() { fs::remove_dir_all(p) } else { fs::remove_file(p) }
    .map_err(|e| e.to_string())
}

/// `name (copy)`, `name (copy 2)`… — never silently overwrite an existing file
/// (what happens when you copy a file and paste it into the same folder).
fn unique_dest(dest: &std::path::Path) -> PathBuf {
    if !dest.exists() { return dest.to_path_buf(); }
    let parent = dest.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let stem = dest.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = dest.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    for i in 1..1000 {
        let tag = if i == 1 { " (copy)".to_string() } else { format!(" (copy {})", i) };
        let cand = parent.join(format!("{}{}{}", stem, tag, ext));
        if !cand.exists() { return cand; }
    }
    dest.to_path_buf()
}

fn copy_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        let target = fs::read_link(src)?;
        std::os::unix::fs::symlink(target, dst)
    } else if meta.is_dir() {
        fs::create_dir_all(dst)?;
        for e in fs::read_dir(src)? {
            let e = e?;
            copy_recursive(&e.path(), &dst.join(e.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(src, dst).map(|_| ())
    }
}

#[tauri::command(async)]
pub fn copy_file(src: String, dest: String) -> Result<(), String> {
    // Previously `fs::copy`, which fails outright on folders ("Copy" on a
    // directory in Explorer always errored) and overwrote existing files.
    let s = resolve_path(&src);
    let d = unique_dest(&resolve_path(&dest));
    if d.starts_with(&s) && s.is_dir() { return Err("Cannot copy a folder into itself".to_string()); }
    copy_recursive(&s, &d).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn move_file(src: String, dest: String) -> Result<(), String> {
    let s = resolve_path(&src);
    let mut d = resolve_path(&dest);
    if s == d { return Ok(()); }
    // Never move a folder into itself / one of its own sub-folders.
    if s.is_dir() && d.starts_with(&s) { return Err("Cannot move a folder into itself".to_string()); }
    // `fs::rename` silently REPLACES an existing file at the destination — that
    // lost data when dropping icons onto a folder that already held a file of
    // the same name. Pick a free name instead ("a (1).txt"), like `copy_file`.
    if d.exists() { d = unique_dest(&d); }
    match fs::rename(&s, &d) {
        Ok(()) => Ok(()),
        // `rename` can't cross filesystems (e.g. Home → USB stick) — copy, then remove.
        Err(e) if e.raw_os_error() == Some(18) => {
            copy_recursive(&s, &d).map_err(|e| e.to_string())?;
            if s.is_dir() { fs::remove_dir_all(&s) } else { fs::remove_file(&s) }.map_err(|e| e.to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

// ── Trash / compress / extract / open-with / details ────────────────────────

fn shq(s: &str) -> String { format!("'{}'", s.replace('\'', "'\\''")) }

// Trash (move / list / restore / delete / empty) lives in trash.rs — Blue's own
// trash under ~/.cache/Blue-Environment/trash/.

/// Create an archive next to the first item. `format`: "zip" | "tar.gz" | "tar.xz" | "tar.zst".
/// Returns the created archive's path.
#[tauri::command(async)]
pub fn compress_files(paths: Vec<String>, format: String, name: Option<String>) -> Result<String, String> {
    if paths.is_empty() { return Err("Nothing to compress".to_string()); }
    let resolved: Vec<PathBuf> = paths.iter().map(|p| resolve_path(p)).collect();
    let dir = resolved[0].parent().map(|p| p.to_path_buf()).ok_or("bad path")?;
    let names: Vec<String> = resolved.iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).collect();
    let stem = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
        if names.len() == 1 { names[0].clone() } else { "Archive".to_string() }
    });
    let ext = match format.as_str() { "zip" | "tar.gz" | "tar.xz" | "tar.zst" => format.as_str(), _ => return Err("Unsupported archive format".to_string()) };
    let out = unique_dest(&dir.join(format!("{}.{}", stem, ext)));
    let out_name = out.file_name().unwrap().to_string_lossy().to_string();

    let status = if ext == "zip" {
        let mut c = Command::new("zip");
        c.current_dir(&dir).arg("-rq").arg(&out_name).args(&names);
        c.status()
    } else {
        let flag = match ext { "tar.gz" => "-czf", "tar.xz" => "-cJf", _ => "--zstd -cf" };
        let mut c = Command::new("tar");
        c.current_dir(&dir);
        for f in flag.split(' ') { c.arg(f); }
        c.arg(&out_name).arg("--").args(&names);
        c.status()
    };
    match status {
        Ok(s) if s.success() => Ok(out.to_string_lossy().to_string()),
        Ok(_) => { let _ = fs::remove_file(&out); Err(format!("Compression failed ({})", ext)) }
        Err(e) => Err(format!("Could not run {}: {}", if ext == "zip" { "zip" } else { "tar" }, e)),
    }
}

/// Extract an archive into a folder next to it (named after the archive).
#[tauri::command(async)]
pub fn extract_archive_here(path: String) -> Result<String, String> {
    let p = resolve_path(&path);
    let dir = p.parent().map(|x| x.to_path_buf()).ok_or("bad path")?;
    let fname = p.file_name().unwrap().to_string_lossy().to_string();
    let lower = fname.to_lowercase();
    let stem = ["tar.gz", "tar.xz", "tar.zst", "tar.bz2", "tgz", "zip", "7z", "rar", "tar"].iter()
        .find_map(|e| lower.strip_suffix(&format!(".{}", e)).map(|_| fname[..fname.len() - e.len() - 1].to_string()))
        .unwrap_or_else(|| format!("{}-extracted", fname));
    let dest = unique_dest(&dir.join(stem));
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let ps = p.to_string_lossy().to_string();
    let ds = dest.to_string_lossy().to_string();
    let ok = if lower.ends_with(".zip") {
        Command::new("unzip").args(["-q", &ps, "-d", &ds]).status().map(|s| s.success()).unwrap_or(false)
    } else if lower.ends_with(".7z") || lower.ends_with(".rar") {
        Command::new("7z").args(["x", &format!("-o{}", ds), "-y", &ps]).status().map(|s| s.success()).unwrap_or(false)
    } else {
        Command::new("tar").args(["-xf", &ps, "-C", &ds]).status().map(|s| s.success()).unwrap_or(false)
    };
    if ok { Ok(ds) } else { let _ = fs::remove_dir(&dest); Err("Extraction failed (is the needed tool installed?)".to_string()) }
}

#[derive(Serialize, Clone)]
pub struct OpenWithApp {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub exec: String,
    pub recommended: bool,
}

fn desktop_entry_dirs() -> Vec<PathBuf> {
    let mut v = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
    ];
    if let Some(d) = dirs::data_dir() {
        v.push(d.join("applications"));
        v.push(d.join("flatpak/exports/share/applications"));
    }
    v
}

/// Applications that can open `mime` (from the .desktop `MimeType=` keys),
/// "recommended" first. With `all = true` every visible application is
/// included too — the "Other application…" list from KDE's Open With.
#[tauri::command(async)]
pub fn get_open_with_apps(mime: String, all: bool) -> Vec<OpenWithApp> {
    let major = mime.split('/').next().unwrap_or("").to_string();
    let mut out: Vec<OpenWithApp> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for d in desktop_entry_dirs() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            if path.extension().map(|x| x != "desktop").unwrap_or(true) { continue; }
            let id = path.file_name().unwrap().to_string_lossy().to_string();
            if !seen.insert(id.clone()) { continue; }
            let Ok(text) = fs::read_to_string(&path) else { continue };
            let (mut name, mut exec, mut icon, mut mimes) = (String::new(), String::new(), None, String::new());
            let (mut hidden, mut kind_ok, mut in_entry) = (false, true, false);
            for line in text.lines() {
                let line = line.trim();
                if line.starts_with('[') { in_entry = line == "[Desktop Entry]"; continue; }
                if !in_entry { continue; }
                let Some((k, v)) = line.split_once('=') else { continue };
                match k {
                    "Name" => name = v.to_string(),
                    "Exec" => exec = v.to_string(),
                    "Icon" => icon = Some(v.to_string()),
                    "MimeType" => mimes = v.to_string(),
                    "NoDisplay" | "Hidden" => if v.eq_ignore_ascii_case("true") { hidden = true },
                    "Type" => kind_ok = v == "Application",
                    _ => {}
                }
            }
            if hidden || !kind_ok || name.is_empty() || exec.is_empty() { continue; }
            let recommended = mimes.split(';').any(|m| !m.is_empty() && (m == mime || m == format!("{}/*", major)));
            if recommended || all {
                out.push(OpenWithApp { id, name, icon, exec, recommended });
            }
        }
    }
    out.sort_by(|a, b| b.recommended.cmp(&a.recommended).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

/// Launch a .desktop `Exec=` line with `path` as the file argument.
#[tauri::command(async)]
pub fn open_with_app(exec: String, path: String) -> Result<(), String> {
    let target = resolve_path(&path).to_string_lossy().to_string();
    let q = shq(&target);
    let mut cmd = String::new();
    let mut used = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' { cmd.push(c); continue; }
        match chars.next() {
            Some('f') | Some('F') | Some('u') | Some('U') => { cmd.push_str(&q); used = true; }
            Some('%') => cmd.push('%'),
            _ => {} // %i %c %k %d … are dropped
        }
    }
    if !used { cmd.push(' '); cmd.push_str(&q); }
    Command::new("sh").arg("-c").arg(format!("nohup {} >/dev/null 2>&1 &", cmd))
        .status().map(|_| ()).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct FileDetails {
    pub permissions: String,
    pub owner: String,
    pub group: String,
    pub size_bytes: u64,
    pub accessed: String,
    pub modified: String,
    pub symlink_target: Option<String>,
}

#[tauri::command(async)]
pub fn get_file_details(path: String) -> Result<FileDetails, String> {
    let p = resolve_path(&path);
    let ps = p.to_string_lossy().to_string();
    let out = Command::new("stat").args(["-c", "%A|%U|%G|%s|%x|%y", &ps]).output().map_err(|e| e.to_string())?;
    if !out.status.success() { return Err("stat failed".to_string()); }
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let f: Vec<&str> = line.splitn(6, '|').collect();
    if f.len() < 6 { return Err("unexpected stat output".to_string()); }
    let mut size: u64 = f[3].parse().unwrap_or(0);
    if p.is_dir() {
        // Real folder size (bounded: a huge tree must not hang the panel).
        if let Ok(o) = Command::new("timeout").args(["3", "du", "-sb", &ps]).output() {
            if let Some(n) = String::from_utf8_lossy(&o.stdout).split_whitespace().next().and_then(|n| n.parse().ok()) { size = n; }
        }
    }
    let short = |s: &str| s.split('.').next().unwrap_or(s).to_string();
    Ok(FileDetails {
        permissions: f[0].to_string(), owner: f[1].to_string(), group: f[2].to_string(),
        size_bytes: size, accessed: short(f[4]), modified: short(f[5]),
        symlink_target: fs::read_link(&p).ok().map(|t| t.to_string_lossy().to_string()),
    })
}

#[tauri::command(async)]
pub fn create_text_file(path: String, name: String, content: String) -> Result<(), String> {
    let p = resolve_path(&path).join(name);
    if let Some(parent) = p.parent() { fs::create_dir_all(parent).ok(); }
    fs::write(p, content).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn read_file_as_data_url(path: String) -> Result<String, String> {
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let ext = PathBuf::from(&path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mime = match ext.as_str() {
        "jpg"|"jpeg" => "image/jpeg",
        "png"        => "image/png",
        "gif"        => "image/gif",
        "svg"        => "image/svg+xml",
        "webp"       => "image/webp",
        _            => "application/octet-stream",
    };
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    Ok(format!("data:{};base64,{}", mime, STANDARD.encode(bytes)))
}

#[tauri::command(async)]
pub fn get_default_desktop_path() -> String {
    if let Ok(o) = Command::new("xdg-user-dir").arg("DESKTOP").output() {
        if o.status.success() {
            let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !p.is_empty() && p != "/" { return p; }
        }
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    for dir in &["Desktop", "Pulpit"] {
        let d = home.join(dir);
        if d.exists() { return d.to_string_lossy().to_string(); }
    }
    let d = home.join("Desktop");
    let _ = fs::create_dir_all(&d);
    d.to_string_lossy().to_string()
}

#[tauri::command(async)]
pub fn get_home_path() -> String {
    dirs::home_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "/root".to_string())
}

#[tauri::command(async)]
pub fn get_username() -> String {
    if let Ok(u) = std::env::var("USER") { if !u.is_empty() { return u; } }
    if let Ok(u) = std::env::var("LOGNAME") { if !u.is_empty() { return u; } }
    Command::new("whoami").output()
        .ok()
        .and_then(|o| {
            let u = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if u.is_empty() { None } else { Some(u) }
        })
        .unwrap_or_else(|| "user".to_string())
}

#[tauri::command(async)]
pub fn get_hostname() -> String {
    if let Ok(o) = Command::new("hostname").output() {
        let h = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !h.is_empty() { return h; }
    }
    if let Ok(h) = fs::read_to_string("/etc/hostname") {
        let h = h.trim().to_string();
        if !h.is_empty() { return h; }
    }
    std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string())
}

#[tauri::command(async)]
pub fn pick_file(filters: Option<String>) -> Option<String> {
    // Fallback backend picker (Tauri's plugin-dialog is preferred on the
    // frontend; this is only reached when the JS plugin isn't available).
    let _ = filters;
    None
}

#[tauri::command(async)]
pub fn pick_directory() -> Option<String> {
    None
}

#[tauri::command(async)]
pub fn git_status(path: String) -> Vec<String> {
    Command::new("git")
        .args(["-C", &path, "status", "--short"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.to_string()).collect())
        .unwrap_or_default()
}
