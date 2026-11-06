#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::{interface, Connection, ObjectServer};

pub const BUS_NAME: &str = "org.freedesktop.impl.portal.desktop.blue";
pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

/// Portal response codes (`org.freedesktop.impl.portal.Request`).
pub const RESPONSE_OK: u32 = 0;
pub const RESPONSE_CANCELLED: u32 = 1;
pub const RESPONSE_OTHER: u32 = 2;

// ── UI abstraction ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Filter {
    pub name: String,
    /// File extensions without dot (`png`); empty = any file.
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChooserMode {
    Open,
    Save,
    /// Pick a folder; the caller then places the named files inside it.
    SaveFiles,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // `app_id` / `accept_label` are part of the request for UI implementations that show them
pub struct ChooserRequest {
    pub mode: ChooserMode,
    pub app_id: String,
    pub title: String,
    pub multiple: bool,
    pub directory: bool,
    pub filters: Vec<Filter>,
    pub current_name: Option<String>,
    pub current_folder: Option<PathBuf>,
    pub accept_label: Option<String>,
}

/// What the portal needs from the shell's UI. Both calls block until the person answers.
pub trait PortalUi: Send + Sync + 'static {
    /// `None` = cancelled.
    fn choose(&self, req: &ChooserRequest) -> Option<Vec<PathBuf>>;
    /// Yes/no question, e.g. "Allow Firefox to take a screenshot?".
    fn confirm(&self, title: &str, message: &str) -> bool;
}

// ── pure helpers (unit-tested) ─────────────────────────────────────────────

/// `/home/me/My File #1.png` → `file:///home/me/My%20File%20%231.png`.
pub fn path_to_uri(p: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut out = String::from("file://");
    for &b in p.as_os_str().as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let b = rest.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = rest.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(out)))
}

/// `*.png` → `png`; patterns that are not a plain extension glob (`Makefile`, `*.tar.*`) give `None`.
pub fn glob_extension(pattern: &str) -> Option<String> {
    let ext = pattern.strip_prefix("*.")?;
    (!ext.is_empty() && ext.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_' || b == b'+')).then(|| ext.to_ascii_lowercase())
}

/// The handful of MIME types portals actually send for file pickers.
pub fn mime_extensions(mime: &str) -> &'static [&'static str] {
    match mime {
        "image/png" => &["png"],
        "image/jpeg" => &["jpg", "jpeg"],
        "image/gif" => &["gif"],
        "image/webp" => &["webp"],
        "image/svg+xml" => &["svg"],
        "image/*" => &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"],
        "application/pdf" => &["pdf"],
        "text/plain" => &["txt", "md", "log"],
        "text/html" => &["html", "htm"],
        "text/csv" => &["csv"],
        "application/json" => &["json"],
        "application/zip" => &["zip"],
        "audio/*" => &["mp3", "flac", "ogg", "wav", "m4a", "opus"],
        "video/*" => &["mp4", "mkv", "webm", "mov", "avi"],
        "video/mp4" => &["mp4"],
        "audio/mpeg" => &["mp3"],
        _ => &[],
    }
}

/// Portal filter spec `a(sa(us))`: `(name, [(kind, pattern)])`, kind 0 = glob, 1 = MIME.
/// A filter containing something we cannot express as extensions becomes "any file"
/// rather than silently hiding the files the app wants.
pub fn convert_filters(raw: &[(String, Vec<(u32, String)>)]) -> Vec<Filter> {
    raw.iter()
        .map(|(name, pats)| {
            let mut exts: Vec<String> = Vec::new();
            let mut any = pats.is_empty();
            for (kind, pat) in pats {
                let found: Vec<String> = match kind {
                    0 => glob_extension(pat).into_iter().collect(),
                    1 => mime_extensions(pat).iter().map(|s| s.to_string()).collect(),
                    _ => Vec::new(),
                };
                if found.is_empty() {
                    any = true;
                }
                exts.extend(found);
            }
            exts.sort();
            exts.dedup();
            Filter { name: name.clone(), extensions: if any { Vec::new() } else { exts } }
        })
        .collect()
}

/// Safe single file name from the app-supplied `current_name` (no directories).
pub fn sanitize_file_name(name: &str) -> Option<String> {
    let base = name.rsplit('/').next()?.trim();
    (!base.is_empty() && base != "." && base != ".." && !base.contains('\0')).then(|| base.to_string())
}

/// Where portal screenshots go.
pub fn screenshot_dir() -> PathBuf {
    let base = std::env::var_os("XDG_PICTURES_DIR").map(PathBuf::from).or_else(|| dirs::picture_dir()).unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Pictures"));
    base.join("Screenshots")
}

pub fn screenshot_file_name(now: chrono::DateTime<chrono::Local>) -> String {
    format!("Screenshot from {}.png", now.format("%Y-%m-%d %H-%M-%S"))
}

fn str_opt(o: &HashMap<String, OwnedValue>, k: &str) -> Option<String> {
    o.get(k).and_then(|v| v.try_clone().ok()).and_then(|v| String::try_from(v).ok())
}
fn bool_opt(o: &HashMap<String, OwnedValue>, k: &str) -> bool {
    o.get(k).and_then(|v| v.try_clone().ok()).and_then(|v| bool::try_from(v).ok()).unwrap_or(false)
}
/// `current_folder` is a NUL-terminated byte string (`ay`).
fn folder_opt(o: &HashMap<String, OwnedValue>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let bytes = o.get("current_folder").and_then(|v| v.try_clone().ok()).and_then(|v| Vec::<u8>::try_from(v).ok())?;
    let trimmed: Vec<u8> = bytes.into_iter().take_while(|&b| b != 0).collect();
    (!trimmed.is_empty()).then(|| PathBuf::from(std::ffi::OsString::from_vec(trimmed)))
}
fn filters_opt(o: &HashMap<String, OwnedValue>) -> Vec<Filter> {
    let raw: Option<Vec<(String, Vec<(u32, String)>)>> = o.get("filters").and_then(|v| v.try_clone().ok()).and_then(|v| v.try_into().ok());
    convert_filters(&raw.unwrap_or_default())
}

fn results_with_uris(uris: Vec<String>, extra: Vec<(&str, OwnedValue)>) -> HashMap<String, OwnedValue> {
    let mut m: HashMap<String, OwnedValue> = HashMap::new();
    if let Ok(v) = OwnedValue::try_from(Value::from(uris)) {
        m.insert("uris".into(), v);
    }
    for (k, v) in extra {
        m.insert(k.to_string(), v);
    }
    m
}

// ── Request object (so the frontend can cancel) ────────────────────────────

struct Request;

#[interface(name = "org.freedesktop.impl.portal.Request")]
impl Request {
    /// The shell's chooser cannot be torn down from here; the answer is simply discarded.
    async fn close(&self) {}
}

async fn with_request<T>(server: &ObjectServer, handle: &ObjectPath<'_>, fut: impl std::future::Future<Output = T>) -> T {
    let _ = server.at(handle.as_str(), Request).await;
    let out = fut.await;
    let _ = server.remove::<Request, _>(handle.as_str()).await;
    out
}

// ── FileChooser ────────────────────────────────────────────────────────────

struct FileChooser {
    ui: Arc<dyn PortalUi>,
}

impl FileChooser {
    async fn run(&self, req: ChooserRequest) -> (u32, HashMap<String, OwnedValue>) {
        let ui = self.ui.clone();
        let picked = tokio::task::spawn_blocking(move || ui.choose(&req)).await.ok().flatten();
        match picked {
            Some(paths) if !paths.is_empty() => (RESPONSE_OK, results_with_uris(paths.iter().map(|p| path_to_uri(p)).collect(), vec![])),
            _ => (RESPONSE_CANCELLED, HashMap::new()),
        }
    }
}

#[interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    async fn open_file(
        &self,
        handle: ObjectPath<'_>,
        app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, HashMap<String, OwnedValue>) {
        let req = ChooserRequest {
            mode: ChooserMode::Open,
            app_id,
            title,
            multiple: bool_opt(&options, "multiple"),
            directory: bool_opt(&options, "directory"),
            filters: filters_opt(&options),
            current_name: None,
            current_folder: folder_opt(&options),
            accept_label: str_opt(&options, "accept_label"),
        };
        with_request(server, &handle, self.run(req)).await
    }

    async fn save_file(
        &self,
        handle: ObjectPath<'_>,
        app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, HashMap<String, OwnedValue>) {
        let req = ChooserRequest {
            mode: ChooserMode::Save,
            app_id,
            title,
            multiple: false,
            directory: false,
            filters: filters_opt(&options),
            current_name: str_opt(&options, "current_name").and_then(|n| sanitize_file_name(&n)),
            current_folder: folder_opt(&options),
            accept_label: str_opt(&options, "accept_label"),
        };
        with_request(server, &handle, self.run(req)).await
    }

    /// Save several files: the person picks a folder, the files go inside it.
    async fn save_files(
        &self,
        handle: ObjectPath<'_>,
        app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, HashMap<String, OwnedValue>) {
        let names: Vec<String> = options
            .get("files")
            .and_then(|v| v.try_clone().ok())
            .and_then(|v| Vec::<Vec<u8>>::try_from(v).ok())
            .map(|files| files.into_iter().filter_map(|b| String::from_utf8(b.into_iter().take_while(|&c| c != 0).collect()).ok()).filter_map(|n| sanitize_file_name(&n)).collect())
            .unwrap_or_default();
        let req = ChooserRequest {
            mode: ChooserMode::SaveFiles,
            app_id,
            title,
            multiple: false,
            directory: true,
            filters: Vec::new(),
            current_name: None,
            current_folder: folder_opt(&options),
            accept_label: str_opt(&options, "accept_label"),
        };
        with_request(server, &handle, async {
            let ui = self.ui.clone();
            let picked = tokio::task::spawn_blocking(move || ui.choose(&req)).await.ok().flatten().and_then(|mut v| v.pop());
            match picked {
                Some(dir) => (RESPONSE_OK, results_with_uris(names.iter().map(|n| path_to_uri(&dir.join(n))).collect(), vec![])),
                None => (RESPONSE_CANCELLED, HashMap::new()),
            }
        })
        .await
    }

    #[zbus(property, name = "version")]
    async fn version(&self) -> u32 {
        3
    }
}

// ── Screenshot ─────────────────────────────────────────────────────────────

struct Screenshot {
    ui: Arc<dyn PortalUi>,
    /// Command used to capture: `grim`; replaceable for tests.
    grim: String,
}

impl Screenshot {
    fn capture(&self, interactive: bool, dest: &Path) -> Result<(), String> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut cmd = std::process::Command::new(&self.grim);
        if interactive {
            // region picker; cancelling it (non-zero exit / empty output) is a cancel
            let region = std::process::Command::new("slurp").output().map_err(|e| format!("slurp unavailable: {e}"))?;
            let geometry = String::from_utf8_lossy(&region.stdout).trim().to_string();
            if !region.status.success() || geometry.is_empty() {
                return Err("cancelled".into());
            }
            cmd.arg("-g").arg(geometry);
        }
        let status = cmd.arg(dest).status().map_err(|e| format!("{} unavailable: {e}", self.grim))?;
        if status.success() && dest.is_file() {
            Ok(())
        } else {
            Err(format!("{} failed", self.grim))
        }
    }
}

#[interface(name = "org.freedesktop.impl.portal.Screenshot")]
impl Screenshot {
    async fn screenshot(
        &self,
        handle: ObjectPath<'_>,
        app_id: String,
        _parent_window: String,
        options: HashMap<String, OwnedValue>,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, HashMap<String, OwnedValue>) {
        let interactive = bool_opt(&options, "interactive");
        let ui = self.ui.clone();
        let who = if app_id.is_empty() { "An application".to_string() } else { app_id };
        with_request(server, &handle, async {
            let q = who.clone();
            let allowed = tokio::task::spawn_blocking(move || ui.confirm("Screenshot request", &format!("{q} wants to take a screenshot of your screen.\n\nAllow it?"))).await.unwrap_or(false);
            if !allowed {
                return (RESPONSE_CANCELLED, HashMap::new());
            }
            let dest = screenshot_dir().join(screenshot_file_name(chrono::Local::now()));
            let dest2 = dest.clone();
            let grim = Screenshot { ui: self.ui.clone(), grim: self.grim.clone() };
            match tokio::task::spawn_blocking(move || grim.capture(interactive, &dest2)).await {
                Ok(Ok(())) => {
                    let mut m = HashMap::new();
                    if let Ok(v) = OwnedValue::try_from(Value::from(path_to_uri(&dest))) {
                        m.insert("uri".to_string(), v);
                    }
                    (RESPONSE_OK, m)
                }
                Ok(Err(e)) if e == "cancelled" => (RESPONSE_CANCELLED, HashMap::new()),
                Ok(Err(e)) => {
                    tracing::warn!("portal screenshot failed: {e}");
                    (RESPONSE_OTHER, HashMap::new())
                }
                Err(_) => (RESPONSE_OTHER, HashMap::new()),
            }
        })
        .await
    }

    /// Not supported: answering "other" is honest; a made-up colour is not.
    async fn pick_color(&self, _handle: ObjectPath<'_>, _app_id: String, _parent_window: String, _options: HashMap<String, OwnedValue>) -> (u32, HashMap<String, OwnedValue>) {
        (RESPONSE_OTHER, HashMap::new())
    }

    #[zbus(property, name = "version")]
    async fn version(&self) -> u32 {
        2
    }
}

// ── entry point ────────────────────────────────────────────────────────────

/// Claims `org.freedesktop.impl.portal.desktop.blue` on the session bus.
/// `Ok(None)` when the name is taken (a second shell instance).
pub async fn run(ui: Arc<dyn PortalUi>) -> zbus::Result<Option<Connection>> {
    let built = zbus::connection::Builder::session()?
        .name(BUS_NAME)?
        .serve_at(PORTAL_PATH, FileChooser { ui: ui.clone() })?
        .serve_at(PORTAL_PATH, Screenshot { ui, grim: "grim".to_string() })?
        .build()
        .await;
    match built {
        Ok(c) => {
            tracing::info!("portal backend: serving {BUS_NAME}");
            Ok(Some(c))
        }
        Err(zbus::Error::NameTaken) => {
            tracing::warn!("portal backend: {BUS_NAME} already owned");
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

// ── status (Diagnostics page) ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PortalStatus {
    /// `org.freedesktop.portal.Desktop` is on the bus.
    pub frontend_running: bool,
    /// Our backend owns its bus name.
    pub backend_running: bool,
    /// `blue.portal` and `blue-portals.conf` are installed.
    pub config_installed: bool,
    /// xdg-desktop-portal-wlr (screen sharing) is installed.
    pub wlr_installed: bool,
    /// A PipeWire daemon is running (needed for screen sharing).
    pub pipewire_running: bool,
    pub grim_installed: bool,
    /// Human-readable fixes, empty when everything is in place.
    pub problems: Vec<String>,
}

fn on_path(bin: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file())).unwrap_or(false)
}

fn any_exists(paths: &[&str]) -> bool {
    paths.iter().any(|p| Path::new(p).exists())
}

/// Pure: turns the probed facts into the list of things to fix.
pub fn diagnose(mut s: PortalStatus) -> PortalStatus {
    let mut p = Vec::new();
    if !s.frontend_running {
        p.push("xdg-desktop-portal is not running — install it (package xdg-desktop-portal) and start your session through the Blue launcher.".to_string());
    }
    if !s.config_installed {
        p.push("Blue's portal configuration is missing: install packaging/portals/blue.portal and blue-portals.conf to /usr/share/xdg-desktop-portal/.".to_string());
    }
    if !s.backend_running {
        p.push("The Blue portal backend is not on the session bus (restart the shell).".to_string());
    }
    if !s.wlr_installed {
        p.push("Screen sharing needs xdg-desktop-portal-wlr.".to_string());
    }
    if !s.pipewire_running {
        p.push("Screen sharing needs PipeWire (pipewire + wireplumber) to be running.".to_string());
    }
    if !s.grim_installed {
        p.push("Screenshots through the portal need grim (and slurp for region selection).".to_string());
    }
    s.problems = p;
    s
}

pub async fn status() -> PortalStatus {
    let mut s = PortalStatus {
        config_installed: any_exists(&["/usr/share/xdg-desktop-portal/portals/blue.portal", "/etc/xdg-desktop-portal/portals/blue.portal"])
            && any_exists(&["/usr/share/xdg-desktop-portal/blue-portals.conf", "/etc/xdg-desktop-portal/blue-portals.conf"]),
        wlr_installed: any_exists(&["/usr/libexec/xdg-desktop-portal-wlr", "/usr/lib/xdg-desktop-portal-wlr", "/usr/lib/x86_64-linux-gnu/xdg-desktop-portal-wlr", "/usr/lib/xdg-desktop-portal/xdg-desktop-portal-wlr"]) || on_path("xdg-desktop-portal-wlr"),
        grim_installed: on_path("grim"),
        ..Default::default()
    };
    if let Ok(conn) = Connection::session().await {
        if let Ok(proxy) = zbus::fdo::DBusProxy::new(&conn).await {
            async fn has(proxy: &zbus::fdo::DBusProxy<'_>, n: &str) -> bool {
                match zbus::names::BusName::try_from(n.to_string()) {
                    Ok(name) => proxy.name_has_owner(name).await.unwrap_or(false),
                    Err(_) => false,
                }
            }
            s.frontend_running = has(&proxy, "org.freedesktop.portal.Desktop").await;
            s.backend_running = has(&proxy, BUS_NAME).await;
            s.pipewire_running = std::process::Command::new("pgrep").args(["-x", "pipewire"]).status().map(|st| st.success()).unwrap_or(false);
        }
    }
    diagnose(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_round_trip_including_awkward_names() {
        for p in ["/home/me/plain.txt", "/home/me/My File #1 (copy).png", "/tmp/zażółć gęślą.txt", "/a/b%c/d"] {
            let uri = path_to_uri(Path::new(p));
            assert!(uri.starts_with("file:///"));
            assert!(!uri.contains(' ') && !uri.contains('#'), "{uri}");
            assert_eq!(uri_to_path(&uri).unwrap(), PathBuf::from(p));
        }
        assert_eq!(uri_to_path("https://x/y"), None);
        assert_eq!(uri_to_path("file:///bad%zz"), None);
        assert_eq!(uri_to_path("file://localhost/etc/hosts").unwrap(), PathBuf::from("/etc/hosts"));
    }

    #[test]
    fn filters_become_extensions_or_any_file_never_a_silent_empty_list() {
        let raw = vec![
            ("Images".to_string(), vec![(0u32, "*.PNG".to_string()), (0, "*.jpg".to_string()), (1, "image/gif".to_string())]),
            ("Documents".to_string(), vec![(1, "application/pdf".to_string()), (0, "Makefile".to_string())]),
            ("All".to_string(), vec![]),
            ("Weird".to_string(), vec![(1, "application/x-unknown".to_string())]),
        ];
        let f = convert_filters(&raw);
        assert_eq!(f[0], Filter { name: "Images".into(), extensions: vec!["gif".into(), "jpg".into(), "png".into()] });
        assert!(f[1].extensions.is_empty(), "an un-expressible pattern widens to any file");
        assert!(f[2].extensions.is_empty());
        assert!(f[3].extensions.is_empty());
    }

    #[test]
    fn glob_extension_accepts_only_plain_extension_globs() {
        assert_eq!(glob_extension("*.rs").as_deref(), Some("rs"));
        assert_eq!(glob_extension("*.tar.gz").as_deref(), Some("tar.gz"));
        assert_eq!(glob_extension("*.[ch]"), None);
        assert_eq!(glob_extension("README"), None);
        assert_eq!(glob_extension("*."), None);
    }

    #[test]
    fn app_supplied_file_names_cannot_escape_the_chosen_folder() {
        assert_eq!(sanitize_file_name("report.pdf").as_deref(), Some("report.pdf"));
        assert_eq!(sanitize_file_name("../../etc/passwd").as_deref(), Some("passwd"));
        assert_eq!(sanitize_file_name("/abs/path/x.txt").as_deref(), Some("x.txt"));
        assert_eq!(sanitize_file_name(".."), None);
        assert_eq!(sanitize_file_name("dir/"), None);
        assert_eq!(sanitize_file_name(""), None);
    }

    #[test]
    fn screenshot_names_are_stable_and_filesystem_safe() {
        let t = chrono::TimeZone::with_ymd_and_hms(&chrono::Local, 2026, 11, 1, 13, 5, 9).unwrap();
        assert_eq!(screenshot_file_name(t), "Screenshot from 2026-11-01 13-05-09.png");
        assert!(screenshot_dir().ends_with("Screenshots"));
    }

    #[test]
    fn diagnose_lists_exactly_what_is_missing() {
        let all_ok = diagnose(PortalStatus { frontend_running: true, backend_running: true, config_installed: true, wlr_installed: true, pipewire_running: true, grim_installed: true, problems: vec![] });
        assert!(all_ok.problems.is_empty());
        let bare = diagnose(PortalStatus::default());
        assert_eq!(bare.problems.len(), 6);
        let no_wlr = diagnose(PortalStatus { frontend_running: true, backend_running: true, config_installed: true, wlr_installed: false, pipewire_running: true, grim_installed: true, problems: vec![] });
        assert_eq!(no_wlr.problems.len(), 1);
        assert!(no_wlr.problems[0].contains("wlr"));
    }

    #[test]
    fn shipped_portal_files_are_consistent() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packaging/portals");
        let portal = std::fs::read_to_string(dir.join("blue.portal")).unwrap();
        assert!(portal.contains(&format!("DBusName={BUS_NAME}")));
        assert!(portal.contains("org.freedesktop.impl.portal.FileChooser") && portal.contains("org.freedesktop.impl.portal.Screenshot"));
        assert!(!portal.contains("ScreenCast"), "ScreenCast is delegated to wlr, not claimed here");
        let conf = std::fs::read_to_string(dir.join("blue-portals.conf")).unwrap();
        assert!(conf.contains("org.freedesktop.impl.portal.FileChooser=blue"));
        assert!(conf.contains("org.freedesktop.impl.portal.ScreenCast=wlr"));
    }
}
