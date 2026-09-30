pub mod error;
pub mod install;
pub mod manifest;
pub mod net;

use error::{StoreError, StoreResult};
use install::Receipt;
use manifest::{compare_versions, Manifest, PackageKind};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Emitter;

pub type ProgressFn = Arc<dyn Fn(u8, &str) + Send + Sync>;

fn parse_kind(kind: &str) -> StoreResult<PackageKind> {
    PackageKind::parse(kind).ok_or_else(|| StoreError::new("bad_kind", format!("Unknown package kind \"{kind}\"")))
}

// ── privileged job protocol ────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum PrivJob {
    Install { manifest: Manifest, archive: String, source_url: String },
    Uninstall { kind: PackageKind, id: String },
}

fn proto(line: serde_json::Value) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}{}", crate::privileged::LINE_PREFIX, line);
    let _ = out.flush();
}

/// Entry point of `blue-environment --store-priv` (runs as root). Reads one
/// [`PrivJob`] from stdin, talks back with protocol lines, returns the exit code.
pub fn run_priv_helper() -> i32 {
    let mut input = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).is_err() {
        proto(serde_json::json!({"event": "error", "error": StoreError::new("internal", "Could not read the job")}));
        return 2;
    }
    let job: PrivJob = match serde_json::from_str(input.trim()) {
        Ok(j) => j,
        Err(e) => {
            proto(serde_json::json!({"event": "error", "error": StoreError::new("internal", "Malformed job").detail(e.to_string())}));
            return 2;
        }
    };
    let progress = |pct: u8, msg: &str| proto(serde_json::json!({"event": "progress", "pct": pct, "message": msg}));
    let result: StoreResult<serde_json::Value> = match job {
        PrivJob::Install { manifest, archive, source_url } => install_job(&manifest, &archive, &source_url, &progress).map(|r| serde_json::json!(r)),
        PrivJob::Uninstall { kind, id } => install::uninstall(kind, &id).map(|_| serde_json::json!(null)),
    };
    match result {
        Ok(v) => {
            proto(serde_json::json!({"event": "done", "result": v}));
            0
        }
        Err(e) => {
            proto(serde_json::json!({"event": "error", "error": e}));
            1
        }
    }
}

/// Root-side install: the caller-supplied archive is first copied into a
/// root-owned private file, so nothing the (unprivileged) caller does to the
/// original after this point can influence what gets validated/installed.
fn install_job(manifest: &Manifest, archive: &str, source_url: &str, progress: &dyn Fn(u8, &str)) -> StoreResult<Receipt> {
    use std::os::unix::fs::PermissionsExt;
    let src = PathBuf::from(archive);
    let meta = std::fs::symlink_metadata(&src).map_err(|e| StoreError::io("opening the downloaded archive", e))?;
    if !meta.file_type().is_file() {
        return Err(StoreError::new("bad_path", "The archive path is not a regular file"));
    }
    if meta.len() > net::MAX_ARCHIVE_BYTES {
        return Err(StoreError::new("too_large", "The package is larger than 200 MiB"));
    }
    let root = install::install_root(manifest.kind);
    std::fs::create_dir_all(&root).map_err(|e| StoreError::io("creating the install directory", e))?;
    let private = root.join(format!(".dl-{}-{}.blue", manifest.id, std::process::id()));
    std::fs::copy(&src, &private).map_err(|e| StoreError::io("copying the archive", e))?;
    let _ = std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o600));
    let result = install::install_from_archive(manifest.kind, &private, manifest, source_url, progress);
    let _ = std::fs::remove_file(&private);
    result
}

/// Runs an install/uninstall job, elevating when needed.
fn run_job(job: PrivJob, progress: &dyn Fn(u8, &str)) -> StoreResult<serde_json::Value> {
    if install::is_root() {
        return match job {
            PrivJob::Install { manifest, archive, source_url } => install_job(&manifest, &archive, &source_url, progress).map(|r| serde_json::json!(r)),
            PrivJob::Uninstall { kind, id } => install::uninstall(kind, &id).map(|_| serde_json::json!(null)),
        };
    }
    let payload = serde_json::to_string(&job).map_err(|e| StoreError::new("internal", e.to_string()))?;
    let mut final_result: Option<StoreResult<serde_json::Value>> = None;
    let mut log_tail: Vec<String> = Vec::new();

    let outcome = crate::privileged::run_privileged(
        &["--store-priv"],
        &payload,
        |line| {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { return };
            match v.get("event").and_then(|e| e.as_str()) {
                Some("progress") => {
                    let pct = v.get("pct").and_then(|p| p.as_u64()).unwrap_or(0).min(100) as u8;
                    progress(pct, v.get("message").and_then(|m| m.as_str()).unwrap_or(""));
                }
                Some("done") => final_result = Some(Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null))),
                Some("error") => {
                    let err = serde_json::from_value::<StoreError>(v.get("error").cloned().unwrap_or_default())
                        .unwrap_or_else(|_| StoreError::new("internal", "The privileged helper reported an unknown error"));
                    final_result = Some(Err(err));
                }
                _ => {}
            }
        },
        |log| {
            log_tail.push(log.to_string());
            if log_tail.len() > 30 {
                log_tail.remove(0);
            }
        },
    )
    .map_err(|e| {
        let mut err = StoreError::new("privilege", e.message);
        if let Some(h) = e.hint {
            err = err.hint(h);
        }
        if let Some(d) = e.detail {
            err = err.detail(d);
        }
        err
    })?;

    match final_result {
        Some(r) => r,
        None => {
            let info = crate::privileged::explain_launch_failure(&outcome);
            let mut err = StoreError::new("privilege", info.message);
            if let Some(h) = info.hint {
                err = err.hint(h);
            }
            let detail = [outcome.stderr_tail.as_str(), &log_tail.join("\n")].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join("\n");
            if !detail.is_empty() {
                err = err.detail(detail);
            }
            Err(err)
        }
    }
}

// ── shared flow ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPackage {
    pub manifest: Manifest,
    pub manifest_url: String,
    pub installed_version: Option<String>,
    pub update_available: bool,
}

pub async fn resolve(url: &str) -> StoreResult<ResolvedPackage> {
    let (manifest, manifest_url) = net::fetch_manifest(url).await?;
    let installed = install::installed_receipt(manifest.kind, &manifest.id);
    let installed_version = installed.as_ref().map(|r| r.version.clone());
    let update_available = installed_version.as_deref().map(|v| compare_versions(&manifest.version, v).is_gt()).unwrap_or(false);
    Ok(ResolvedPackage { manifest, manifest_url: manifest_url.to_string(), installed_version, update_available })
}

/// Download (cached) + privileged install of the package behind `url`.
pub async fn install_package(url: &str, expected_kind: Option<PackageKind>, progress: ProgressFn) -> StoreResult<Receipt> {
    progress(1, "Reading blue.hk…");
    let (manifest, manifest_url) = net::fetch_manifest(url).await?;
    if let Some(k) = expected_kind {
        if k != manifest.kind {
            return Err(StoreError::new("kind_mismatch", format!("This is a {} package, but a {} was expected", manifest.kind.as_str(), k.as_str()))
                .hint("Install it from the matching section (Apps / Plugins / Themes)."));
        }
    }
    let archive_url = reqwest::Url::parse(manifest.archive.as_deref().unwrap_or("")).map_err(|_| StoreError::new("manifest_bad_archive", "blue.hk has no usable `archive`"))?;

    let cache = install::cache_dir().join(install::cache_file_name(manifest.kind, &manifest.id, &manifest.version));
    let reuse = match (&manifest.sha256, cache.is_file()) {
        (Some(want), true) => install::sha256_file(&cache).map(|h| h.eq_ignore_ascii_case(want)).unwrap_or(false),
        _ => false,
    };
    if reuse {
        progress(40, "Using cached download");
    } else {
        let p = progress.clone();
        // download is 5–45 % of the whole operation
        let scaled = move |pct: u8, msg: &str| p(5 + (pct as u16 * 40 / 100) as u8, msg);
        net::download_archive(&archive_url, &cache, &scaled).await?;
    }

    // Verify before spending an authentication prompt on it.
    progress(46, "Verifying checksum…");
    let sha = install::sha256_file(&cache).map_err(|e| StoreError::io("hashing the download", e))?;
    if let Some(want) = &manifest.sha256 {
        if !want.eq_ignore_ascii_case(&sha) {
            let _ = std::fs::remove_file(&cache);
            return Err(StoreError::new("sha256_mismatch", "The download does not match the checksum published in blue.hk")
                .hint("Nothing was installed. Try again; if it keeps failing the package is broken or has been tampered with.")
                .detail(format!("expected {want}, got {sha}")));
        }
    }

    progress(50, "Installing…");
    let job = PrivJob::Install { manifest: manifest.clone(), archive: cache.to_string_lossy().to_string(), source_url: manifest_url.to_string() };
    let p = progress.clone();
    let value = tokio::task::spawn_blocking(move || {
        let scaled = |pct: u8, msg: &str| p(50 + (pct as u16 * 50 / 100) as u8, msg);
        run_job(job, &scaled)
    })
    .await
    .map_err(|e| StoreError::new("internal", "Install task crashed").detail(e.to_string()))??;
    let receipt: Receipt = serde_json::from_value(value).map_err(|e| StoreError::new("internal", "Unexpected install result").detail(e.to_string()))?;
    progress(100, "Installed");
    Ok(receipt)
}

pub async fn uninstall_package(kind: PackageKind, id: String) -> StoreResult<()> {
    manifest::validate_id(&id)?;
    tokio::task::spawn_blocking(move || run_job(PrivJob::Uninstall { kind, id }, &|_, _| {}).map(|_| ()))
        .await
        .map_err(|e| StoreError::new("internal", "Uninstall task crashed").detail(e.to_string()))?
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub id: String,
    pub kind: PackageKind,
    pub installed_version: String,
    pub latest_version: String,
    pub manifest_url: String,
}

pub async fn check_updates(kind: PackageKind) -> Vec<UpdateInfo> {
    let installed = install::list_installed(kind);
    let mut tasks = Vec::new();
    for r in installed {
        tasks.push(tokio::spawn(async move {
            let (m, _) = net::fetch_manifest(&r.source_url).await.ok()?;
            if m.id == r.id && compare_versions(&m.version, &r.version).is_gt() {
                Some(UpdateInfo { id: r.id, kind: r.kind, installed_version: r.version, latest_version: m.version, manifest_url: r.source_url })
            } else {
                None
            }
        }));
    }
    let mut out = Vec::new();
    for t in tasks {
        if let Ok(Some(u)) = t.await {
            out.push(u);
        }
    }
    out
}

// ── Tauri commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn store_fetch_index(kind: String) -> Result<net::IndexResult, StoreError> {
    net::fetch_index(parse_kind(&kind)?).await
}

#[tauri::command]
pub async fn store_resolve(url: String) -> Result<ResolvedPackage, StoreError> {
    resolve(&url).await
}

#[tauri::command]
pub async fn store_install(app: tauri::AppHandle, url: String, expected_kind: Option<String>, op_id: String) -> Result<Receipt, StoreError> {
    let expected = match expected_kind {
        Some(k) => Some(parse_kind(&k)?),
        None => None,
    };
    let handle = app.clone();
    let op = op_id.clone();
    let progress: ProgressFn = Arc::new(move |pct, msg| {
        let _ = handle.emit("store-progress", serde_json::json!({"opId": op, "pct": pct, "message": msg}));
    });
    let result = install_package(&url, expected, progress).await;
    let _ = app.emit("store-changed", serde_json::json!({"opId": op_id, "ok": result.is_ok()}));
    result
}

#[tauri::command]
pub async fn store_uninstall(app: tauri::AppHandle, kind: String, id: String) -> Result<(), StoreError> {
    let result = uninstall_package(parse_kind(&kind)?, id).await;
    let _ = app.emit("store-changed", serde_json::json!({"ok": result.is_ok()}));
    result
}

#[tauri::command]
pub fn store_list_installed(kind: String) -> Result<Vec<Receipt>, StoreError> {
    Ok(install::list_installed(parse_kind(&kind)?))
}

#[tauri::command]
pub async fn store_check_updates(kind: String) -> Result<Vec<UpdateInfo>, StoreError> {
    Ok(check_updates(parse_kind(&kind)?).await)
}

/// UTF-8 text of a file inside an installed package (app entry bundle, style).
#[tauri::command]
pub fn store_read_file(kind: String, id: String, path: String) -> Result<String, StoreError> {
    let bytes = install::read_installed_file(parse_kind(&kind)?, &id, &path)?;
    String::from_utf8(bytes).map_err(|_| StoreError::new("bad_encoding", "The file is not valid UTF-8 text"))
}

/// An installed package's icon file as a `data:` URL (png / svg / jpg / webp).
#[tauri::command]
pub fn store_read_icon(kind: String, id: String, path: String) -> Result<String, StoreError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    let mime = match path.rsplit('.').next().map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => return Err(StoreError::new("bad_path", "Unsupported icon type")),
    };
    let bytes = install::read_installed_file(parse_kind(&kind)?, &id, &path)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(StoreError::new("too_large", "Icon is larger than 2 MiB"));
    }
    Ok(format!("data:{mime};base64,{}", B64.encode(bytes)))
}

// ── CLI: `blue-environment --store <install|remove|list|update> …` ─────────

/// Non-GUI entry used by the `blue` command (`blue apps install <url>`). When
/// started without root it elevates only the file-writing step, exactly like
/// the GUI does.
pub fn run_cli(args: &[String]) -> i32 {
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: could not start the async runtime: {e}");
            return 2;
        }
    };
    let progress: ProgressFn = Arc::new(|pct, msg| println!("[{pct:>3}%] {msg}"));
    let print_err = |e: &StoreError| {
        eprintln!("error: {}", e.message);
        if let Some(h) = &e.hint {
            eprintln!("  hint: {h}");
        }
        if let Some(d) = &e.detail {
            eprintln!("  detail: {d}");
        }
    };
    let sub = args.first().map(String::as_str).unwrap_or("");
    match sub {
        "install" => {
            let Some(url) = args.get(1) else {
                eprintln!("usage: --store install <URL of blue.hk>");
                return 2;
            };
            match rt.block_on(install_package(url, None, progress)) {
                Ok(r) => {
                    println!("Installed {} {} ({})", r.name, r.version, r.kind.as_str());
                    0
                }
                Err(e) => {
                    print_err(&e);
                    1
                }
            }
        }
        "remove" => {
            let (Some(kind), Some(id)) = (args.get(1), args.get(2)) else {
                eprintln!("usage: --store remove <app|plugin|theme> <id>");
                return 2;
            };
            let kind = match parse_kind(kind) {
                Ok(k) => k,
                Err(e) => {
                    print_err(&e);
                    return 2;
                }
            };
            match rt.block_on(uninstall_package(kind, id.clone())) {
                Ok(()) => {
                    println!("Removed {id}");
                    0
                }
                Err(e) => {
                    print_err(&e);
                    1
                }
            }
        }
        "list" => {
            let kinds: Vec<PackageKind> = match args.get(1).and_then(|k| PackageKind::parse(k)) {
                Some(k) => vec![k],
                None => vec![PackageKind::App, PackageKind::Plugin, PackageKind::Theme],
            };
            for k in kinds {
                for r in install::list_installed(k) {
                    println!("{}\t{}\t{}\t{}", k.as_str(), r.id, r.version, r.name);
                }
            }
            0
        }
        "update" => {
            let mut code = 0;
            for k in [PackageKind::App, PackageKind::Plugin, PackageKind::Theme] {
                for u in rt.block_on(check_updates(k)) {
                    println!("Updating {} {} -> {}", u.id, u.installed_version, u.latest_version);
                    if let Err(e) = rt.block_on(install_package(&u.manifest_url, Some(k), progress.clone())) {
                        print_err(&e);
                        code = 1;
                    }
                }
            }
            code
        }
        _ => {
            eprintln!("usage: blue-environment --store <install <URL>|remove <kind> <id>|list [kind]|update>");
            2
        }
    }
}
