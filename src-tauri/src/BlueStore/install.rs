use super::error::{StoreError, StoreResult};
use super::manifest::{is_safe_relative_path, parse_manifest, Manifest, PackageKind, MANIFEST_FILE, RECEIPT_FILE};
use hk_parser::{parse_hk, HkValue};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_MEMBERS: usize = 5_000;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
/// Largest file the frontend may read back (app entry bundles etc.).
pub const MAX_READ_BYTES: u64 = 16 * 1024 * 1024;

pub const SYSTEM_APPS_DIR: &str = "/usr/share/Blue-Environment/apps";
pub const SYSTEM_PLUGINS_DIR: &str = "/usr/share/Blue-Environment/plugins";
pub const SYSTEM_THEMES_DIR: &str = "/usr/share/themes";
pub const SYSTEM_CACHE_DIR: &str = "/var/cache/blue-environment";

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Directory a kind is installed into. `BLUE_*_DIR` overrides exist for
/// development and tests, but are honoured ONLY when not running as root —
/// a privileged helper must never let the calling user redirect where it
/// writes.
pub fn install_root(kind: PackageKind) -> PathBuf {
    let (var, default) = match kind {
        PackageKind::App => ("BLUE_APPS_DIR", SYSTEM_APPS_DIR),
        PackageKind::Plugin => ("BLUE_PLUGINS_DIR", SYSTEM_PLUGINS_DIR),
        PackageKind::Theme => ("BLUE_THEMES_DIR", SYSTEM_THEMES_DIR),
    };
    if !is_root() {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return PathBuf::from(v);
            }
        }
    }
    PathBuf::from(default)
}

/// `/var/cache/blue-environment` when the current user can write there (the
/// HackerOS image creates it world-writable+sticky via tmpfiles.d), else
/// `~/.cache/blue-environment`.
pub fn cache_dir() -> PathBuf {
    if !is_root() {
        if let Ok(v) = std::env::var("BLUE_CACHE_DIR") {
            if !v.is_empty() {
                let _ = fs::create_dir_all(&v);
                return PathBuf::from(v);
            }
        }
    }
    let system = PathBuf::from(SYSTEM_CACHE_DIR);
    if fs::create_dir_all(&system).is_ok() && dir_is_writable(&system) {
        return system;
    }
    let user = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("blue-environment");
    let _ = fs::create_dir_all(&user);
    user
}

fn dir_is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".w-{}", std::process::id()));
    let ok = fs::write(&probe, b"").is_ok();
    let _ = fs::remove_file(&probe);
    ok
}

pub fn cache_file_name(kind: PackageKind, id: &str, version: &str) -> String {
    format!("{}-{}-{}.blue", kind.as_str(), id, version)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub id: String,
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub entry: String,
    pub style: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub min_width: Option<u32>,
    pub min_height: Option<u32>,
    pub plugin_kind: Option<String>,
    pub permissions: Vec<String>,
    pub source_url: String,
    pub sha256: String,
    pub installed_at: String,
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

// ── tar helpers ────────────────────────────────────────────────────────────

fn tar_command() -> Command {
    let mut c = Command::new("tar");
    c.env("LC_ALL", "C");
    c
}

fn tar_failure(what: &str, out: &std::process::Output) -> StoreError {
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let lower = stderr.to_lowercase();
    let mut e = StoreError::new("archive_unreadable", format!("Could not {what} the .blue archive")).detail(stderr.clone());
    if lower.contains("zstd") && (lower.contains("not found") || lower.contains("cannot exec") || lower.contains("can't exec")) {
        e = StoreError::new("zstd_missing", "The `zstd` tool is required to unpack .blue packages but was not found")
            .hint("Install it with: sudo apt install zstd")
            .detail(stderr);
    } else if lower.contains("not in gzip") || lower.contains("not in zstd") || lower.contains("zstd: error") || lower.contains("unknown frame") {
        e = e.hint("The file is not a valid tar+zstd archive. Build it with `blue-dev pack`.");
    }
    e
}

/// Validates the member list without extracting anything.
pub fn validate_archive_members(archive: &Path) -> StoreResult<Vec<String>> {
    let names = tar_command()
        .args(["--zstd", "-tf"])
        .arg(archive)
        .output()
        .map_err(|e| StoreError::new("tar_missing", "Could not run `tar`").hint("Install tar (sudo apt install tar zstd).").detail(e.to_string()))?;
    if !names.status.success() {
        return Err(tar_failure("read", &names));
    }
    let verbose = tar_command()
        .args(["--zstd", "-tvf"])
        .arg(archive)
        .output()
        .map_err(|e| StoreError::io("running tar", e))?;
    if !verbose.status.success() {
        return Err(tar_failure("read", &verbose));
    }
    let names = String::from_utf8_lossy(&names.stdout).to_string();
    let verbose = String::from_utf8_lossy(&verbose.stdout).to_string();
    let name_lines: Vec<&str> = names.lines().collect();
    let type_lines: Vec<&str> = verbose.lines().collect();
    if name_lines.len() != type_lines.len() {
        return Err(StoreError::new("archive_unsafe", "Archive contains members with unusual names (embedded newlines?)"));
    }
    if name_lines.is_empty() {
        return Err(StoreError::new("archive_empty", "The .blue archive is empty"));
    }
    if name_lines.len() > MAX_MEMBERS {
        return Err(StoreError::new("archive_unsafe", format!("Archive has too many members ({} > {MAX_MEMBERS})", name_lines.len())));
    }

    let mut cleaned = Vec::with_capacity(name_lines.len());
    for (name, tline) in name_lines.iter().zip(type_lines.iter()) {
        let kind_char = tline.chars().next().unwrap_or('?');
        if kind_char != '-' && kind_char != 'd' {
            return Err(StoreError::new("archive_unsafe", format!("Archive member \"{name}\" is not a regular file or directory (type '{kind_char}')"))
                .hint("Symlinks, hard links and device nodes are not allowed in .blue packages."));
        }
        let trimmed = name.trim_start_matches("./").trim_end_matches('/');
        if trimmed.is_empty() || trimmed == "." {
            continue;
        }
        if !is_safe_relative_path(trimmed) {
            return Err(StoreError::new("archive_unsafe", format!("Archive member has an unsafe path: \"{name}\"")));
        }
        cleaned.push(trimmed.to_string());
    }
    if !cleaned.iter().any(|n| n == MANIFEST_FILE) {
        return Err(StoreError::new("archive_no_manifest", "The .blue archive has no blue.hk at its root")
            .hint("Run `blue-dev pack` from the project directory — it puts blue.hk at the archive root."));
    }
    Ok(cleaned)
}

fn extract_archive(archive: &Path, dest: &Path) -> StoreResult<()> {
    let mut cmd = tar_command();
    cmd.args(["--zstd", "-xf"])
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .args(["--no-same-owner", "--no-same-permissions", "--no-overwrite-dir", "-m"]);
    // Cap the size of any single extracted file (decompression-bomb guard).
    unsafe {
        cmd.pre_exec(|| {
            let lim = libc::rlimit { rlim_cur: MAX_FILE_BYTES as libc::rlim_t, rlim_max: MAX_FILE_BYTES as libc::rlim_t };
            libc::setrlimit(libc::RLIMIT_FSIZE, &lim);
            Ok(())
        });
    }
    let out = cmd.output().map_err(|e| StoreError::io("running tar", e))?;
    if !out.status.success() {
        return Err(tar_failure("extract", &out));
    }
    Ok(())
}

/// Re-checks the extracted tree and normalises modes (dirs 0755, files 0644).
fn sanitize_tree(root: &Path) -> StoreResult<u64> {
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).map_err(|e| StoreError::io("chmod", e))?;
        for entry in fs::read_dir(&dir).map_err(|e| StoreError::io("reading staging dir", e))? {
            let entry = entry.map_err(|e| StoreError::io("reading staging dir", e))?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| StoreError::io("stat", e))?;
            let ft = meta.file_type();
            if ft.is_symlink() {
                return Err(StoreError::new("archive_unsafe", format!("Extracted symlink rejected: {}", path.display())));
            }
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                total += meta.len();
                if total > MAX_TOTAL_BYTES {
                    return Err(StoreError::new("archive_too_large", "The package is larger than 512 MiB once unpacked"));
                }
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).map_err(|e| StoreError::io("chmod", e))?;
            } else {
                return Err(StoreError::new("archive_unsafe", format!("Unexpected file type: {}", path.display())));
            }
        }
    }
    Ok(total)
}

fn hk_quote(s: &str) -> String {
    let cleaned: String = s.chars().map(|c| if c == '"' { '\'' } else if c == '\n' || c == '\r' { ' ' } else { c }).collect();
    format!("\"{cleaned}\"")
}

/// Themes are discovered by `themes.rs` through `<dir>/config.hk` +
/// `<dir>/styles.css`. A `.blue` theme doesn't have to ship a `config.hk`:
/// its `[package]` metadata and optional `[effects]` section in `blue.hk`
/// are converted into one.
fn ensure_theme_config(dir: &Path, m: &Manifest, manifest_text: &str) -> StoreResult<()> {
    if !dir.join("styles.css").is_file() {
        return Err(StoreError::new("theme_no_styles", "A theme package must contain styles.css at its root"));
    }
    if dir.join("config.hk").is_file() {
        return Ok(());
    }
    let mut out = String::new();
    out.push_str("! Generated by Blue Store from blue.hk\n[metadata]\n");
    out.push_str(&format!("-> name => {}\n-> author => {}\n-> version => {}\n-> description => {}\n", hk_quote(&m.name), hk_quote(&m.author), hk_quote(&m.version), hk_quote(&m.description)));
    if let Ok(cfg) = parse_hk(manifest_text) {
        if let Some(HkValue::Map(effects)) = cfg.get("effects") {
            out.push_str("\n[effects]\n");
            for (k, v) in effects {
                match v {
                    HkValue::Bool(b) => out.push_str(&format!("-> {k} => {b}\n")),
                    HkValue::String(s) => out.push_str(&format!("-> {k} => {}\n", hk_quote(s))),
                    HkValue::Number(n) => out.push_str(&format!("-> {k} => {n}\n")),
                    _ => {}
                }
            }
        }
    }
    fs::write(dir.join("config.hk"), out).map_err(|e| StoreError::io("writing config.hk", e))
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Installs the (already downloaded and hash-checked) archive.
///
/// `expected` is the manifest fetched from the store URL; the archive's own
/// `blue.hk` has to agree with it on id, version and type.
pub fn install_from_archive(
    kind: PackageKind,
    archive: &Path,
    expected: &Manifest,
    source_url: &str,
    progress: &dyn Fn(u8, &str),
) -> StoreResult<Receipt> {
    if expected.kind != kind {
        return Err(StoreError::new("kind_mismatch", format!("This package is a {}, not a {}", expected.kind.as_str(), kind.as_str())));
    }
    progress(5, "Validating archive…");
    let members = validate_archive_members(archive)?;

    let sha = sha256_file(archive).map_err(|e| StoreError::io("hashing archive", e))?;
    if let Some(want) = &expected.sha256 {
        if !want.eq_ignore_ascii_case(&sha) {
            return Err(StoreError::new("sha256_mismatch", "The archive does not match the checksum published in blue.hk")
                .hint("The download may be corrupted or tampered with. Nothing was installed.")
                .detail(format!("expected {want}, got {sha}")));
        }
    }

    let root = install_root(kind);
    fs::create_dir_all(&root).map_err(|e| {
        let mut err = StoreError::io(&format!("creating {}", root.display()), e);
        if !is_root() {
            err = err.hint("Installing system-wide needs administrator rights.");
        }
        err
    })?;
    let _ = fs::set_permissions(&root, fs::Permissions::from_mode(0o755));

    let staging = root.join(format!(".staging-{}-{}", expected.id, std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| StoreError::io("creating staging directory", e))?;

    // Everything from here on must clean the staging dir up on failure.
    let result = (|| -> StoreResult<Receipt> {
        progress(20, "Unpacking…");
        extract_archive(archive, &staging)?;
        progress(55, "Checking contents…");
        sanitize_tree(&staging)?;

        let manifest_text = fs::read_to_string(staging.join(MANIFEST_FILE))
            .map_err(|e| StoreError::new("archive_no_manifest", "blue.hk inside the archive could not be read").detail(e.to_string()))?;
        let inner = parse_manifest(&manifest_text, None)?;
        if inner.id != expected.id || inner.version != expected.version || inner.kind != expected.kind {
            return Err(StoreError::new("manifest_mismatch", "blue.hk inside the archive doesn't match the published blue.hk")
                .hint("Republish the package so both manifests agree on id, version and type.")
                .detail(format!(
                    "published: {} {} ({}), archive: {} {} ({})",
                    expected.id, expected.version, expected.kind.as_str(), inner.id, inner.version, inner.kind.as_str()
                )));
        }

        match kind {
            PackageKind::App | PackageKind::Plugin => {
                if !members.iter().any(|m| m == &inner.entry) {
                    return Err(StoreError::new("entry_missing", format!("The entry file \"{}\" is not in the archive", inner.entry))
                        .hint("Bundle your code to a single .js file and set `entry` in blue.hk accordingly."));
                }
                if let Some(style) = &inner.style {
                    if !members.iter().any(|m| m == style) {
                        return Err(StoreError::new("entry_missing", format!("The style file \"{style}\" is not in the archive")));
                    }
                }
            }
            PackageKind::Theme => ensure_theme_config(&staging, &inner, &manifest_text)?,
        }
        if let Some(icon) = &inner.icon {
            if icon.contains('.') && !members.iter().any(|m| m == icon) {
                return Err(StoreError::new("icon_missing", format!("The icon file \"{icon}\" is not in the archive")));
            }
        }

        progress(75, "Installing…");
        let receipt = Receipt {
            id: inner.id.clone(),
            kind,
            name: inner.name.clone(),
            version: inner.version.clone(),
            author: inner.author.clone(),
            description: inner.description.clone(),
            icon: inner.icon.clone(),
            category: inner.category.clone(),
            entry: inner.entry.clone(),
            style: inner.style.clone(),
            width: inner.width,
            height: inner.height,
            min_width: inner.min_width,
            min_height: inner.min_height,
            plugin_kind: inner.plugin_kind.clone(),
            permissions: inner.permissions.clone(),
            source_url: source_url.to_string(),
            sha256: sha.clone(),
            installed_at: now_rfc3339(),
        };
        let json = serde_json::to_string_pretty(&receipt).map_err(|e| StoreError::new("internal", e.to_string()))?;
        fs::write(staging.join(RECEIPT_FILE), json).map_err(|e| StoreError::io("writing install receipt", e))?;
        fs::set_permissions(staging.join(RECEIPT_FILE), fs::Permissions::from_mode(0o644)).ok();

        // Swap into place. An existing Blue-Store-managed install is
        // replaced; a directory we did NOT create (no receipt) is never
        // touched — that's how a GTK theme in /usr/share/themes stays safe.
        let final_dir = root.join(&inner.id);
        let backup = root.join(format!(".old-{}-{}", inner.id, std::process::id()));
        let had_old = final_dir.exists();
        if had_old {
            if !final_dir.join(RECEIPT_FILE).is_file() {
                return Err(StoreError::new("target_not_managed", format!("{} already exists and was not installed by Blue Store", final_dir.display()))
                    .hint("Choose a different package id or remove that directory manually."));
            }
            fs::rename(&final_dir, &backup).map_err(|e| StoreError::io("replacing the previous version", e))?;
        }
        if let Err(e) = fs::rename(&staging, &final_dir) {
            if had_old {
                let _ = fs::rename(&backup, &final_dir);
            }
            return Err(StoreError::io("moving the package into place", e));
        }
        if had_old {
            let _ = fs::remove_dir_all(&backup);
        }
        progress(100, "Installed");
        Ok(receipt)
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

pub fn uninstall(kind: PackageKind, id: &str) -> StoreResult<()> {
    super::manifest::validate_id(id)?;
    let dir = install_root(kind).join(id);
    if !dir.exists() {
        return Err(StoreError::new("not_installed", format!("\"{id}\" is not installed")));
    }
    let receipt = read_receipt(&dir).ok_or_else(|| {
        StoreError::new("target_not_managed", format!("{} was not installed by Blue Store — refusing to delete it", dir.display()))
    })?;
    if receipt.kind != kind || receipt.id != id {
        return Err(StoreError::new("target_not_managed", "Install receipt doesn't match — refusing to delete"));
    }
    fs::remove_dir_all(&dir).map_err(|e| StoreError::io("removing the package", e))
}

fn read_receipt(dir: &Path) -> Option<Receipt> {
    let text = fs::read_to_string(dir.join(RECEIPT_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Every Blue-Store-managed package of `kind`. Directories without a valid
/// receipt (system GTK themes, hand-made folders) are simply not listed.
pub fn list_installed(kind: PackageKind) -> Vec<Receipt> {
    let root = install_root(kind);
    let Ok(rd) = fs::read_dir(&root) else { return Vec::new() };
    let mut out: Vec<Receipt> = rd
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| read_receipt(&e.path()))
        .filter(|r| r.kind == kind)
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Reads one file of an installed package (app entry bundle, stylesheet,
/// icon …). The path is confined to the package directory.
pub fn read_installed_file(kind: PackageKind, id: &str, rel: &str) -> StoreResult<Vec<u8>> {
    super::manifest::validate_id(id)?;
    if !is_safe_relative_path(rel) {
        return Err(StoreError::new("bad_path", "Unsafe path"));
    }
    let dir = install_root(kind).join(id);
    if read_receipt(&dir).is_none() {
        return Err(StoreError::new("not_installed", format!("\"{id}\" is not installed")));
    }
    let path = dir.join(rel);
    // Resolve symlinks and confirm we are still inside the package dir.
    let canon = fs::canonicalize(&path).map_err(|e| StoreError::io("opening package file", e))?;
    let canon_dir = fs::canonicalize(&dir).map_err(|e| StoreError::io("opening package", e))?;
    if !canon.starts_with(&canon_dir) {
        return Err(StoreError::new("bad_path", "Path escapes the package directory"));
    }
    let meta = fs::metadata(&canon).map_err(|e| StoreError::io("stat", e))?;
    if !meta.is_file() || meta.len() > MAX_READ_BYTES {
        return Err(StoreError::new("bad_path", "Not a readable file (or larger than 16 MiB)"));
    }
    fs::read(&canon).map_err(|e| StoreError::io("reading package file", e))
}

pub fn installed_receipt(kind: PackageKind, id: &str) -> Option<Receipt> {
    if super::manifest::validate_id(id).is_err() {
        return None;
    }
    read_receipt(&install_root(kind).join(id))
}
