pub mod error;
pub mod install;
pub mod manifest;
pub mod net;
pub mod permissions;
pub mod signature;
pub mod widgets;

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
    /// What the package asks for, riskiest first — shown before install.
    pub permissions: Vec<permissions::PermissionInfo>,
    pub risk: permissions::Risk,
    /// Declared permissions the installed version did not have (an update that widens access).
    pub new_permissions: Vec<String>,
    /// What the person granted earlier, if this is an update.
    pub previous_granted: Option<Vec<String>>,
    /// Verified against the manifest's published sha256 (re-verified against the real download at install).
    pub signature: signature::SignatureStatus,
    pub signature_policy: signature::Policy,
    pub manifest_url: String,
    pub installed_version: Option<String>,
    pub update_available: bool,
}

pub async fn resolve(url: &str) -> StoreResult<ResolvedPackage> {
    let (manifest, manifest_url) = net::fetch_manifest(url).await?;
    let installed = install::installed_receipt(manifest.kind, &manifest.id);
    let installed_version = installed.as_ref().map(|r| r.version.clone());
    let update_available = installed_version.as_deref().map(|v| compare_versions(&manifest.version, v).is_gt()).unwrap_or(false);
    let record = permissions::record_for(manifest.kind, &manifest.id);
    let new_permissions = match &record {
        Some(r) => permissions::added_permissions(&r.declared, &manifest.permissions),
        None if installed.is_some() => Vec::new(), // installed before consent existed
        None => manifest.permissions.clone(),
    };
    let signature = manifest.sha256.as_deref().map(|h| signature::verify_configured(&manifest, h)).unwrap_or_else(|| {
        if manifest.signature.is_some() {
            signature::SignatureStatus::Invalid { reason: "a signed package must publish its `sha256` in blue.hk".into() }
        } else {
            signature::SignatureStatus::Unsigned
        }
    });
    Ok(ResolvedPackage {
        permissions: permissions::describe(&manifest.permissions),
        risk: permissions::overall_risk(&manifest.permissions),
        new_permissions,
        previous_granted: record.map(|r| r.granted),
        signature,
        signature_policy: signature::load_policy(),
        manifest,
        manifest_url: manifest_url.to_string(),
        installed_version,
        update_available,
    })
}

/// Download (cached) + privileged install of the package behind `url`.
///
/// `granted` is the person's decision from the permission dialog; see
/// `permissions::resolve_grant` for when `None` is acceptable.
pub async fn install_package(url: &str, expected_kind: Option<PackageKind>, progress: ProgressFn, granted: Option<Vec<String>>) -> StoreResult<Receipt> {
    progress(1, "Reading blue.hk…");
    let (manifest, manifest_url) = net::fetch_manifest(url).await?;
    if let Some(k) = expected_kind {
        if k != manifest.kind {
            return Err(StoreError::new("kind_mismatch", format!("This is a {} package, but a {} was expected", manifest.kind.as_str(), k.as_str()))
                .hint("Install it from the matching section (Apps / Plugins / Themes)."));
        }
    }
    // Fail fast, before downloading anything, if a decision is still owed.
    let grant = permissions::resolve_grant(manifest.kind, &manifest.id, &manifest.permissions, granted)?;
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

    progress(47, "Checking signature…");
    let status = signature::verify_configured(&manifest, &sha);
    if let Err(e) = signature::enforce(&status, signature::load_policy()) {
        let _ = std::fs::remove_file(&cache);
        return Err(e);
    }
    if let signature::SignatureStatus::Trusted { name, .. } = &status {
        progress(48, &format!("Signed by {name}"));
    }

    // Record the decision first; restore the previous one if the install then fails,
    // so a failed update never leaves widened (or narrowed) permissions behind.
    let previous = permissions::record_for(manifest.kind, &manifest.id);
    permissions::store_consent(
        manifest.kind,
        &manifest.id,
        permissions::ConsentRecord {
            version: manifest.version.clone(),
            declared: manifest.permissions.clone(),
            granted: grant,
            granted_at: chrono::Utc::now().to_rfc3339(),
            signer: match &status {
                signature::SignatureStatus::Trusted { signer, .. } => Some(signer.clone()),
                _ => None,
            },
        },
    )?;
    let restore = |kind: PackageKind, id: &str, prev: &Option<permissions::ConsentRecord>| match prev {
        Some(r) => {
            let _ = permissions::store_consent(kind, id, r.clone());
        }
        None => permissions::forget(kind, id),
    };

    progress(50, "Installing…");
    let job = PrivJob::Install { manifest: manifest.clone(), archive: cache.to_string_lossy().to_string(), source_url: manifest_url.to_string() };
    let p = progress.clone();
    let value = tokio::task::spawn_blocking(move || {
        let scaled = |pct: u8, msg: &str| p(50 + (pct as u16 * 50 / 100) as u8, msg);
        run_job(job, &scaled)
    })
    .await
    .map_err(|e| StoreError::new("internal", "Install task crashed").detail(e.to_string()));
    let value = match value {
        Ok(Ok(v)) => v,
        Ok(Err(e)) | Err(e) => {
            restore(manifest.kind, &manifest.id, &previous);
            return Err(e);
        }
    };
    let receipt: Receipt = serde_json::from_value(value).map_err(|e| StoreError::new("internal", "Unexpected install result").detail(e.to_string()))?;
    progress(100, "Installed");
    Ok(receipt)
}

pub async fn uninstall_package(kind: PackageKind, id: String) -> StoreResult<()> {
    manifest::validate_id(&id)?;
    let forget_id = id.clone();
    let result = tokio::task::spawn_blocking(move || run_job(PrivJob::Uninstall { kind, id }, &|_, _| {}).map(|_| ()))
        .await
        .map_err(|e| StoreError::new("internal", "Uninstall task crashed").detail(e.to_string()))?;
    if result.is_ok() {
        permissions::forget(kind, &forget_id); // a reinstall must ask again
    }
    result
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

/// Widget Store catalogue (Blue Software → Widgets).
#[tauri::command]
pub async fn store_fetch_widgets() -> Result<widgets::WidgetIndexResult, StoreError> {
    net::fetch_widget_index().await
}

#[tauri::command]
pub async fn store_fetch_index(kind: String) -> Result<net::IndexResult, StoreError> {
    net::fetch_index(parse_kind(&kind)?).await
}

/// Fetches a store entry's icon/preview and returns it as a `data:` URL.
/// The webview's CSP (deliberately) does not allow remote `https:` images, so
/// the card grid can't just use `<img src="https://…">`. https only, images
/// only (png/jpeg/webp/gif/svg), ≤ 1.5 MB, short timeout.
#[tauri::command]
pub async fn store_fetch_image(url: String) -> Result<String, String> {
    use base64::Engine;
    let parsed = reqwest::Url::parse(&url).map_err(|e| e.to_string())?;
    if parsed.scheme() != "https" { return Err("only https images are allowed".to_string()); }
    // raw.githubusercontent.com serves the file; github.com/…/blob/… pages are HTML.
    let fixed = if parsed.host_str() == Some("github.com") && url.contains("/blob/") {
        url.replace("https://github.com/", "https://raw.githubusercontent.com/").replace("/blob/", "/")
    } else { url.clone() };
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().map_err(|e| e.to_string())?;
    let resp = client.get(&fixed).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(format!("HTTP {}", resp.status())); }
    let mime = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").trim().to_lowercase();
    let mime = if mime.starts_with("image/") { mime } else {
        let l = fixed.to_lowercase();
        if l.ends_with(".png") { "image/png".into() } else if l.ends_with(".jpg") || l.ends_with(".jpeg") { "image/jpeg".into() }
        else if l.ends_with(".webp") { "image/webp".into() } else if l.ends_with(".svg") { "image/svg+xml".into() }
        else if l.ends_with(".gif") { "image/gif".into() } else { return Err("not an image".to_string()); }
    };
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > 1_500_000 { return Err("image too large".to_string()); }
    Ok(format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

#[tauri::command]
pub async fn store_resolve(url: String) -> Result<ResolvedPackage, StoreError> {
    resolve(&url).await
}

#[tauri::command]
pub async fn store_install(app: tauri::AppHandle, url: String, expected_kind: Option<String>, op_id: String, granted_permissions: Option<Vec<String>>) -> Result<Receipt, StoreError> {
    let expected = match expected_kind {
        Some(k) => Some(parse_kind(&k)?),
        None => None,
    };
    let handle = app.clone();
    let op = op_id.clone();
    let progress: ProgressFn = Arc::new(move |pct, msg| {
        let _ = handle.emit("store-progress", serde_json::json!({"opId": op, "pct": pct, "message": msg}));
    });
    let result = install_package(&url, expected, progress, granted_permissions).await;
    let _ = app.emit("store-changed", serde_json::json!({"opId": op_id, "ok": result.is_ok()}));
    result
}

#[tauri::command]
pub async fn store_uninstall(app: tauri::AppHandle, kind: String, id: String) -> Result<(), StoreError> {
    let result = uninstall_package(parse_kind(&kind)?, id).await;
    let _ = app.emit("store-changed", serde_json::json!({"ok": result.is_ok()}));
    result
}

#[tauri::command(async)]
pub fn store_list_installed(kind: String) -> Result<Vec<Receipt>, StoreError> {
    // `permissions` is what the person *granted* (≤ what the package declared):
    // the runtimes gate every API call on it.
    Ok(install::list_installed(parse_kind(&kind)?)
        .into_iter()
        .map(|mut r| {
            r.permissions = permissions::effective(r.kind, &r.id, &r.permissions);
            r
        })
        .collect())
}

#[tauri::command]
pub async fn store_check_updates(kind: String) -> Result<Vec<UpdateInfo>, StoreError> {
    Ok(check_updates(parse_kind(&kind)?).await)
}

/// UTF-8 text of a file inside an installed package (app entry bundle, style).
#[tauri::command(async)]
pub fn store_read_file(kind: String, id: String, path: String) -> Result<String, StoreError> {
    let bytes = install::read_installed_file(parse_kind(&kind)?, &id, &path)?;
    String::from_utf8(bytes).map_err(|_| StoreError::new("bad_encoding", "The file is not valid UTF-8 text"))
}

/// An installed package's icon file as a `data:` URL (png / svg / jpg / webp).
#[tauri::command(async)]
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
                eprintln!("usage: --store install <URL of blue.hk> [--grant-all | --grant <a,b> | --no-permissions]");
                return 2;
            };
            let flags = &args[2..];
            let granted = match cli_grant(&rt, url, flags) {
                Ok(g) => g,
                Err(e) => {
                    print_err(&e);
                    return 1;
                }
            };
            match rt.block_on(install_package(url, None, progress, granted)) {
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
                    if let Err(e) = rt.block_on(install_package(&u.manifest_url, Some(k), progress.clone(), None)) {
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


/// Permission decision for the command line: explicit flags win; otherwise the
/// permission list is printed and, on a terminal, confirmed with y/N. Without a
/// terminal and without flags nothing is granted silently (`None` → the install
/// fails with `consent_required` when the package needs permissions).
fn cli_grant(rt: &tokio::runtime::Runtime, url: &str, flags: &[String]) -> StoreResult<Option<Vec<String>>> {
    use std::io::{BufRead, IsTerminal, Write};
    let resolved = rt.block_on(resolve(url))?;
    let declared = &resolved.manifest.permissions;
    if flags.iter().any(|f| f == "--no-permissions") {
        return Ok(Some(Vec::new()));
    }
    if flags.iter().any(|f| f == "--grant-all") {
        return Ok(Some(declared.clone()));
    }
    if let Some(i) = flags.iter().position(|f| f == "--grant") {
        let list = flags.get(i + 1).map(|s| s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()).unwrap_or_default();
        return Ok(Some(list));
    }
    if declared.is_empty() {
        return Ok(None);
    }
    println!("{} {} asks for permissions:", resolved.manifest.name, resolved.manifest.version);
    for p in &resolved.permissions {
        println!("  [{:?}] {} — {}", p.risk, p.title, p.description);
    }
    match &resolved.signature {
        signature::SignatureStatus::Trusted { name, .. } => println!("Signed by {name}."),
        signature::SignatureStatus::Unsigned => println!("WARNING: this package is not signed."),
        signature::SignatureStatus::UnknownSigner { signer } => println!("WARNING: signed by \"{signer}\", who is not in your trusted publishers."),
        signature::SignatureStatus::Invalid { reason } => println!("WARNING: invalid signature: {reason}"),
    }
    if !std::io::stdin().is_terminal() {
        return Ok(None);
    }
    print!("Grant these permissions? [y/N] ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    Ok(Some(if line.trim().eq_ignore_ascii_case("y") { declared.clone() } else { Vec::new() }))
}
