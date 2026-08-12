use crate::ai::PackageInfo;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::io::Read;

/// Runs `cmd` with a hard wall-clock timeout, returning its stdout (or an
/// empty string on any failure — missing binary, non-UTF8 output, or
/// timeout). This exists because several package-manager subcommands
/// touch the network (`dnf check-update`/`repoquery`, `flatpak search`,
/// `zypper` with its default auto-refresh) and, with no repos reachable,
/// can hang for a very long time with the plain `Command::output()` this
/// used to call directly — which previously had **no timeout at all**,
/// so the frontend's loading spinner would never resolve (observed:
/// stuck for over an hour). Every package-manager shell-out in this file
/// goes through this now, local-only commands included, since a stuck
/// package-manager lock (another `apt`/`dnf` process running) can hang
/// those too.
///
/// Implementation note: `std::process::Command` has no built-in timeout,
/// and pulling in a crate (e.g. `wait-timeout`) isn't worth it for this —
/// so this polls `Child::try_wait()` on a short interval and kills the
/// child if the deadline passes, rather than blocking forever on
/// `.output()`/`.wait()`.
fn run_timeout(cmd: &str, args: &[&str], timeout: Duration) -> String {
    let mut child = match Command::new(cmd).args(args)
        .stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    // IMPORTANT: stdout must be drained *while* we wait, not after. A
    // pipe's OS buffer is small (commonly 64KB on Linux) — any command
    // whose output exceeds that (e.g. `dpkg-query -W` listing every
    // installed package on a real system, or `apt-cache show` over a
    // couple dozen packages) will block on write() once the buffer
    // fills, and it can never be scheduled again if nothing is reading
    // the other end. Polling `try_wait()` without reading concurrently
    // deadlocks exactly that case: the child sits blocked forever, so
    // every call silently times out and returns empty — indistinguishable
    // from a real hang, but happening on a perfectly healthy system.
    // A dedicated reader thread avoids this: it keeps consuming stdout
    // the whole time, and the main thread just waits on a channel with
    // a timeout.
    let stdout = child.stdout.take();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut out) = stdout {
            let _ = out.read_to_string(&mut buf);
        }
        let _ = tx.send(buf.clone());
        buf
    });

    match rx.recv_timeout(timeout) {
        Ok(output) => {
            let _ = child.wait(); // reap, avoid a zombie
            output
        }
        Err(_) => {
            // Reader thread is still blocked reading; killing the child
            // closes its stdout, which unblocks the reader with EOF (and
            // whatever partial output had accumulated). Give it a brief
            // moment to notice before giving up entirely.
            let _ = child.kill();
            let _ = child.wait();
            reader.join().unwrap_or_default()
        }
    }
}

/// Default timeout for package-manager shell-outs. Local-database reads
/// (`rpm -qa`, `dpkg-query`, `pacman -Q`) normally return in well under a
/// second; 12s gives generous headroom for a slow/loaded machine while
/// still guaranteeing the UI is never stuck for more than that.
const RUN_TIMEOUT: Duration = Duration::from_secs(12);
/// Longer budget specifically for calls that legitimately talk to the
/// network when repos ARE reachable (`dnf check-update`/`repoquery`,
/// `flatpak search`) — long enough for a real metadata sync on a normal
/// connection, but still bounded instead of unlimited.
const RUN_TIMEOUT_NETWORK: Duration = Duration::from_secs(25);

fn run(cmd: &str, args: &[&str]) -> String {
    run_timeout(cmd, args, RUN_TIMEOUT)
}

fn run_check(cmd: &str) -> bool {
    Command::new("which").arg(cmd).output()
    .map(|o| o.status.success())
    .unwrap_or(false)
}

/// Resolves a package/app id to an icon via the shared FreeDesktop icon
/// theme resolver (linicon — see icon_resolver.rs), preferring Papirus.
/// Most distro packages ship a .desktop file whose Icon= matches the
/// package name closely enough for this to work directly; this is a
/// best-effort lookup and simply returns None when nothing matches
/// rather than fabricating a path, so the frontend correctly falls back
/// to its generic package icon.
fn icon_for_package(name: &str) -> Option<String> {
    let resolved = crate::icon_resolver::resolve_icon(name);
    if resolved.is_empty() { None } else { Some(resolved) }
}

/// Formats a size given in raw bytes (the unit rpm's %{SIZE} tag
/// reports) as a human-readable MB/GB string.
fn format_bytes(size_bytes: u64) -> Option<String> {
    if size_bytes == 0 { return None; }
    let mb = size_bytes as f64 / 1_048_576.0;
    Some(if mb >= 1024.0 {
        format!("{:.2} GB", mb / 1024.0)
    } else {
        format!("{:.1} MB", mb)
    })
}

/// A small set of popular GUI application names used to seed the
/// "Available" tab with real, live-queried dnf results. This is a list
/// of *search terms* only — every field actually shown for each result
/// (description, version, size...) comes straight from a live `dnf`
/// query against the system's real configured repos, never fabricated.
/// This mirrors how GNOME Software / Discover show "Featured/Popular"
/// sections: curated discovery, backed entirely by real package data.
const POPULAR_APPS: &[&str] = &[
    "firefox", "vlc", "gimp", "libreoffice-writer", "blender", "inkscape",
    "audacity", "thunderbird", "krita", "obs-studio", "code", "transmission-gtk",
    "kdenlive", "shotwell", "rhythmbox", "stellarium",
];

pub fn get_dnf_packages() -> Vec<PackageInfo> {
    if !run_check("rpm") { return Vec::new(); }

    let mut packages = Vec::new();
    let mut installed_names: std::collections::HashSet<String> = Default::default();

    // `rpm -qa` reads directly from the local RPM database and returns
    // instantly — much faster than `dnf list installed`, which has to
    // spin up dnf's Python startup and metadata machinery for the same
    // information.
    let raw = run("rpm", &["-qa", "--queryformat", "%{NAME}|%{VERSION}-%{RELEASE}|%{SUMMARY}|%{SIZE}\n"]);
    for line in raw.lines() {
        let p: Vec<&str> = line.splitn(4, '|').collect();
        if p.len() < 4 { continue; }
        let name = p[0].to_string();
        installed_names.insert(name.clone());
        let size_bytes: u64 = p[3].trim().parse().unwrap_or(0);
        packages.push(PackageInfo {
            id: name.clone(), name: name.clone(),
                      description: p[2].trim().to_string(),
                      version: p[1].trim().to_string(),
                      source: "dnf".to_string(), installed: true,
                      update_available: None, // filled in below
                      icon: icon_for_package(&name),
                      size: format_bytes(size_bytes),
        });
    }

    // Determine which installed packages have an update available.
    // `dnf check-update` exits with status 100 (not 0) when updates ARE
    // available, so we read its stdout directly via run() rather than
    // gating on exit status.
    let upg_raw = run_timeout("dnf", &["check-update", "--quiet"], RUN_TIMEOUT_NETWORK);
    let upgradeable: std::collections::HashSet<String> = upg_raw.lines()
    .filter_map(|l| l.split_whitespace().next())
    .filter(|s| !s.is_empty())
    .filter_map(|pkg_arch| pkg_arch.rsplit_once('.').map(|(n, _)| n.to_string()))
    .collect();
    for pkg in packages.iter_mut() {
        pkg.update_available = Some(upgradeable.contains(&pkg.id));
    }

    // "Available" tab content — a single `dnf repoquery` call covering
    // every popular-app search term at once (much faster than one
    // invocation per term, since each dnf process spin-up has real
    // overhead). Every returned field is live data from the system's
    // actual repos, not invented.
    if run_check("dnf") {
        let mut args = vec!["repoquery", "--quiet", "--qf", "%{name}|%{summary}|%{version}"];
        args.extend_from_slice(POPULAR_APPS);
        let repoquery_out = run_timeout("dnf", &args, RUN_TIMEOUT_NETWORK);
        let mut seen_available: std::collections::HashSet<String> = Default::default();
        for line in repoquery_out.lines() {
            let p: Vec<&str> = line.splitn(3, '|').collect();
            if p.len() < 3 { continue; }
            let name = p[0].trim().to_string();
            if name.is_empty() || installed_names.contains(&name) || seen_available.contains(&name) { continue; }
            seen_available.insert(name.clone());
            packages.push(PackageInfo {
                id: name.clone(), name: name.clone(),
                          description: p[1].trim().to_string(),
                          version: p[2].trim().to_string(),
                          source: "dnf".to_string(), installed: false,
                          update_available: None,
                          icon: icon_for_package(&name), size: None,
            });
        }
    }

    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages
}

pub fn install_dnf(pkg_id: &str) -> bool {
    Command::new("pkexec").args(["dnf","install","-y",pkg_id])
    .status().map(|s| s.success()).unwrap_or(false)
}
pub fn remove_dnf(pkg_id: &str) -> bool {
    Command::new("pkexec").args(["dnf","remove","-y",pkg_id])
    .status().map(|s| s.success()).unwrap_or(false)
}
pub fn update_dnf(pkg_id: &str) -> bool {
    Command::new("pkexec").args(["dnf","upgrade","-y",pkg_id])
    .status().map(|s| s.success()).unwrap_or(false)
}

pub fn get_flatpak_packages() -> Vec<PackageInfo> {
    if !run_check("flatpak") { return Vec::new(); }
    let mut packages = Vec::new();

    // Older flatpak versions (<1.4) don't recognize the "size" column and
    // would error out entirely, silently returning zero installed apps.
    // Try the richer query first and fall back to the basic one if it
    // doesn't look like it actually returned package rows.
    let mut raw = run("flatpak", &["list","--app","--columns=application,name,version,size"]);
    let has_size_col = raw.lines().skip(1).any(|l| l.split('\t').count() >= 4);
    if !has_size_col {
        raw = run("flatpak", &["list","--app","--columns=application,name,version"]);
    }

    let mut installed_ids: std::collections::HashSet<String> = Default::default();
    for line in raw.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 { continue; }
        let id = cols[0].trim().to_string();
        installed_ids.insert(id.clone());
        packages.push(PackageInfo {
            id: id.clone(), name: cols[1].trim().to_string(),
                      description: format!("Flatpak: {}", id),
                      version: cols[2].trim().to_string(),
                      source: "flatpak".to_string(), installed: true,
                      update_available: None,
                      // Flatpak exports each app's icon into the user's
                      // icon theme directories under its full reverse-DNS
                      // application id, so looking that id up directly
                      // through the shared theme resolver finds it.
                      icon: icon_for_package(&id),
                      size: cols.get(3).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        });
    }
    let search_raw = run_timeout("flatpak", &["search","--columns=application,name,description,version"], RUN_TIMEOUT_NETWORK);
    for line in search_raw.lines().skip(1).take(20) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 4 { continue; }
        let id = cols[0].trim().to_string();
        if installed_ids.contains(&id) { continue; }
        packages.push(PackageInfo {
            id: id.clone(), name: cols[1].trim().to_string(),
                      description: cols[2].trim().to_string(),
                      version: cols[3].trim().to_string(),
                      source: "flatpak".to_string(), installed: false,
                      update_available: None, icon: icon_for_package(&id), size: None,
        });
    }
    packages
}

pub fn install_flatpak(pkg_id: &str) -> bool {
    Command::new("flatpak").args(["install","--assumeyes","--user","flathub",pkg_id])
    .status().map(|s| s.success()).unwrap_or(false)
}
pub fn remove_flatpak(pkg_id: &str) -> bool {
    Command::new("flatpak").args(["uninstall","--assumeyes","--user",pkg_id])
    .status().map(|s| s.success()).unwrap_or(false)
}
pub fn update_flatpak(pkg_id: &str) -> bool {
    Command::new("flatpak").args(["update","--assumeyes","--user",pkg_id])
    .status().map(|s| s.success()).unwrap_or(false)
}

pub fn get_appimage_packages() -> Vec<PackageInfo> {
    let home = dirs::home_dir().unwrap_or_default();
    let app_dirs = [home.join("Applications"), home.join(".local/bin"), home.join("Downloads")];
    let mut packages = Vec::new();
    let mut seen: std::collections::HashSet<String> = Default::default();
    for dir in &app_dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue; };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if !name.to_lowercase().ends_with(".appimage") || seen.contains(&name) { continue; }
            seen.insert(name.clone());
            let display = name.trim_end_matches(".AppImage").trim_end_matches(".appimage").to_string();
            let size = std::fs::metadata(&path).map(|m| format!("{:.1} MB", m.len() as f64 / 1_048_576.0)).ok();
            packages.push(PackageInfo {
                id: path.to_string_lossy().to_string(), name: display.clone(),
                          description: format!("AppImage: {}", name),
                          version: "local".to_string(), source: "appimage".to_string(),
                          installed: true, update_available: None,
                          icon: icon_for_package(&display.to_lowercase()), size,
            });
        }
    }
    // AppImages have no centralized installable catalog the way apt /
    // flatpak / snap do (there is no "apt search"-equivalent query we can
    // run against a real index), so unlike the other three sources there
    // is no "Available" listing here — only what's genuinely found on
    // disk above. A previous version of this function filled that gap
    // with a hardcoded list of made-up app names and version numbers,
    // which is exactly the kind of placeholder data Blue Software must
    // never show.
    packages
}

pub fn install_appimage(pkg_id: &str) -> bool {
    if std::path::Path::new(pkg_id).exists() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(pkg_id, std::fs::Permissions::from_mode(0o755));
        return true;
    }
    let _ = Command::new("xdg-open")
    .arg(format!("https://appimage.github.io/apps/{}/", pkg_id)).spawn();
    false
}
pub fn remove_appimage(pkg_id: &str) -> bool {
    std::path::Path::new(pkg_id).exists() && std::fs::remove_file(pkg_id).is_ok()
}
pub fn update_appimage(_pkg_id: &str) -> bool { false }

// ─────────────────────────────────────────────────────────────────────────────
// APT backend (Debian / Ubuntu)
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_apt_packages() -> Vec<PackageInfo> {
    if !run_check("dpkg") { return Vec::new(); }
    let mut pkgs = Vec::new();
    let mut installed_names: std::collections::HashSet<String> = Default::default();

    let raw = run("dpkg-query", &["-W", "-f=${Package}|${Version}|${Status}|${Installed-Size}|${binary:Summary}\n"]);
    for line in raw.lines() {
        let p: Vec<&str> = line.splitn(5, '|').collect();
        if p.len() < 5 { continue; }
        let name = p[0].trim().to_string();
        let status = p[2].trim();
        if !status.contains("installed") { continue; }
        installed_names.insert(name.clone());
        let size_kb: u64 = p[3].trim().parse().unwrap_or(0);
        pkgs.push(PackageInfo {
            id: name.clone(), name: name.clone(),
            description: p[4].trim().to_string(),
            version: p[1].trim().to_string(),
            source: "apt".to_string(), installed: true,
            update_available: None, // filled in below
            icon: icon_for_package(&name),
            size: if size_kb > 0 { Some(format!("{:.1} MB", size_kb as f64 / 1024.0)) } else { None },
        });
    }

    // `apt list --upgradable` reads apt's *existing* local cache only — it
    // does NOT trigger a network refresh (that's `apt-get update`, which
    // this deliberately never calls automatically: it needs root and can
    // be slow, so it's left to the OS's own scheduled/manual update
    // mechanism). This just reports against whatever cache is already
    // there, same as `get_dnf_packages`'s `dnf check-update` above.
    if run_check("apt") {
        let upg_raw = run_timeout("apt", &["list", "--upgradable"], RUN_TIMEOUT_NETWORK);
        let upgradeable: std::collections::HashSet<String> = upg_raw.lines()
            .filter_map(|l| l.split('/').next())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "Listing...")
            .collect();
        for pkg in pkgs.iter_mut() {
            pkg.update_available = Some(upgradeable.contains(&pkg.id));
        }
    }

    // "Available" tab content — mirrors `get_dnf_packages`'s single
    // batched `repoquery` call: one `apt-cache show` covering every
    // popular-app term at once (rather than one apt-cache process per
    // term). Every field shown comes from the system's real configured
    // apt sources, nothing invented.
    if run_check("apt-cache") {
        let mut args = vec!["show"];
        args.extend_from_slice(POPULAR_APPS);
        let show_out = run("apt-cache", &args);
        // `apt-cache show` prints one RFC822-ish stanza per package,
        // separated by blank lines.
        let mut seen_available: std::collections::HashSet<String> = Default::default();
        for stanza in show_out.split("\n\n") {
            let mut name = String::new();
            let mut version = String::new();
            let mut description = String::new();
            for line in stanza.lines() {
                if let Some(v) = line.strip_prefix("Package: ") { name = v.trim().to_string(); }
                else if let Some(v) = line.strip_prefix("Version: ") { version = v.trim().to_string(); }
                else if let Some(v) = line.strip_prefix("Description: ").or_else(|| line.strip_prefix("Description-en: ")) {
                    if description.is_empty() { description = v.trim().to_string(); }
                }
            }
            if name.is_empty() || installed_names.contains(&name) || seen_available.contains(&name) { continue; }
            seen_available.insert(name.clone());
            pkgs.push(PackageInfo {
                id: name.clone(), name: name.clone(),
                description, version,
                source: "apt".to_string(), installed: false,
                update_available: None,
                icon: icon_for_package(&name), size: None,
            });
        }
    }

    pkgs
}

pub fn install_apt(pkg: &str) -> bool {
    Command::new("sh").arg("-c")
        .arg(format!("pkexec apt-get install -y {} 2>&1", shell_escape(pkg)))
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn remove_apt(pkg: &str) -> bool {
    Command::new("sh").arg("-c")
        .arg(format!("pkexec apt-get remove -y {} 2>&1", shell_escape(pkg)))
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn update_apt(pkg: &str) -> bool {
    Command::new("sh").arg("-c")
        .arg(format!("pkexec apt-get install --only-upgrade -y {} 2>&1", shell_escape(pkg)))
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn search_apt(query: &str) -> Vec<PackageInfo> {
    if !run_check("apt-cache") { return Vec::new(); }
    let raw = run("apt-cache", &["search", "--names-only", query]);
    raw.lines().take(50).filter_map(|line| {
        let mut parts = line.splitn(2, " - ");
        let name = parts.next()?.trim().to_string();
        let desc = parts.next().unwrap_or("").trim().to_string();
        Some(PackageInfo {
            id: name.clone(), name: name.clone(), description: desc,
            version: String::new(), source: "apt".to_string(),
            installed: false, update_available: None,
            icon: icon_for_package(&name), size: None,
        })
    }).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Pacman backend (Arch Linux)
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_pacman_packages() -> Vec<PackageInfo> {
    if !run_check("pacman") { return Vec::new(); }
    let mut pkgs = Vec::new();
    let mut installed_names: std::collections::HashSet<String> = Default::default();

    let raw = run("pacman", &["-Q", "--noconfirm"]);
    for line in raw.lines() {
        let mut p = line.splitn(2, ' ');
        let name    = p.next().unwrap_or("").trim().to_string();
        let version = p.next().unwrap_or("").trim().to_string();
        if name.is_empty() { continue; }
        installed_names.insert(name.clone());
        // Get description from pkginfo
        let desc = run("pacman", &["-Qi", &name])
            .lines()
            .find(|l| l.starts_with("Description"))
            .and_then(|l| l.split(':').nth(1))
            .unwrap_or("")
            .trim()
            .to_string();
        pkgs.push(PackageInfo {
            id: name.clone(), name: name.clone(), description: desc,
            version, source: "pacman".to_string(), installed: true,
            update_available: None, // filled in below
            icon: icon_for_package(&name), size: None,
        });
    }

    // `pacman -Qu` diffs installed packages against the *already synced*
    // local sync DB — it does not run `pacman -Sy` (which requires root
    // and would refresh over the network), same spirit as the apt/dnf
    // update checks above.
    let upg_raw = run("pacman", &["-Qu"]);
    let upgradeable: std::collections::HashSet<String> = upg_raw.lines()
        .filter_map(|l| l.split_whitespace().next())
        .map(|s| s.to_string())
        .collect();
    for pkg in pkgs.iter_mut() {
        pkg.update_available = Some(upgradeable.contains(&pkg.id));
    }

    // "Available" tab — same POPULAR_APPS seeding pattern as dnf/apt,
    // via `pacman -Si` (sync DB info, local, one process per lookup
    // since pacman -Si doesn't batch as cleanly as apt-cache/dnf, but
    // POPULAR_APPS is a short fixed list so this stays fast).
    let mut seen_available: std::collections::HashSet<String> = Default::default();
    for app in POPULAR_APPS {
        if installed_names.contains(*app) || seen_available.contains(*app) { continue; }
        let info_raw = run("pacman", &["-Si", app]);
        if info_raw.trim().is_empty() { continue; } // not found in any sync repo
        let mut version = String::new();
        let mut description = String::new();
        for line in info_raw.lines() {
            if let Some(v) = line.strip_prefix("Version") { version = v.trim_start_matches(|c: char| c == ':' || c.is_whitespace()).trim().to_string(); }
            else if let Some(v) = line.strip_prefix("Description") { description = v.trim_start_matches(|c: char| c == ':' || c.is_whitespace()).trim().to_string(); }
        }
        seen_available.insert(app.to_string());
        pkgs.push(PackageInfo {
            id: app.to_string(), name: app.to_string(), description, version,
            source: "pacman".to_string(), installed: false,
            update_available: None,
            icon: icon_for_package(app), size: None,
        });
    }

    pkgs
}

pub fn install_pacman(pkg: &str) -> bool {
    Command::new("sh").arg("-c")
        .arg(format!("pkexec pacman -S --noconfirm {} 2>&1", shell_escape(pkg)))
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn remove_pacman(pkg: &str) -> bool {
    Command::new("sh").arg("-c")
        .arg(format!("pkexec pacman -R --noconfirm {} 2>&1", shell_escape(pkg)))
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn update_pacman(pkg: &str) -> bool {
    install_pacman(pkg) // pacman -S upgrades existing
}

pub fn search_pacman(query: &str) -> Vec<PackageInfo> {
    if !run_check("pacman") { return Vec::new(); }
    let raw = run("pacman", &["-Ss", query]);
    let mut pkgs = Vec::new();
    let mut lines = raw.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with("  ") { continue; }
        let mut parts = line.splitn(2, '/');
        let _ = parts.next(); // repo name
        let rest = parts.next().unwrap_or("").trim();
        let mut name_ver = rest.splitn(2, ' ');
        let name = name_ver.next().unwrap_or("").trim().to_string();
        let ver  = name_ver.next().unwrap_or("").trim().to_string();
        let desc = lines.next().unwrap_or("").trim().to_string();
        if name.is_empty() { continue; }
        pkgs.push(PackageInfo {
            id: name.clone(), name: name.clone(), description: desc,
            version: ver, source: "pacman".to_string(),
            installed: false, update_available: None,
            icon: icon_for_package(&name), size: None,
        });
        if pkgs.len() >= 50 { break; }
    }
    pkgs
}

// ─────────────────────────────────────────────────────────────────────────────
// Zypper backend (openSUSE)
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_zypper_packages() -> Vec<PackageInfo> {
    if !run_check("zypper") { return Vec::new(); }
    let raw = run_timeout("zypper", &["--non-interactive", "--xmlout", "packages", "--installed-only"], RUN_TIMEOUT_NETWORK);
    let mut pkgs = Vec::new();
    let mut installed_names: std::collections::HashSet<String> = Default::default();
    for line in raw.lines() {
        if !line.contains("<package ") { continue; }
        let get = |attr: &str| -> String {
            let key = format!("{}=\"", attr);
            line.find(&key)
                .map(|i| {
                    let start = i + key.len();
                    let end = line[start..].find('"').map(|j| start + j).unwrap_or(start);
                    line[start..end].to_string()
                })
                .unwrap_or_default()
        };
        let name = get("name");
        if name.is_empty() { continue; }
        installed_names.insert(name.clone());
        pkgs.push(PackageInfo {
            id: name.clone(), name: name.clone(),
            description: get("summary"),
            version: get("edition"),
            source: "zypper".to_string(), installed: true,
            update_available: None, // filled in below
            icon: icon_for_package(&name), size: None,
        });
    }

    // `list-updates` with `--no-refresh` reads whatever repo metadata is
    // already cached — the `packages --installed-only` call just above
    // already talks to the network once (via RUN_TIMEOUT_NETWORK); this
    // deliberately doesn't add a *second* refresh on top of that, same
    // "don't force extra network round-trips" rule the apt/dnf/pacman
    // update checks already follow.
    let upd_raw = run_timeout("zypper", &["--non-interactive", "--no-refresh", "--xmlout", "list-updates"], RUN_TIMEOUT_NETWORK);
    let mut upgradeable: std::collections::HashSet<String> = Default::default();
    for line in upd_raw.lines() {
        if !line.contains("<update ") { continue; }
        if let Some(i) = line.find("name=\"") {
            let start = i + 6;
            if let Some(end) = line[start..].find('"') {
                upgradeable.insert(line[start..start + end].to_string());
            }
        }
    }
    for pkg in pkgs.iter_mut() {
        pkg.update_available = Some(upgradeable.contains(&pkg.id));
    }

    // "Available" tab — same POPULAR_APPS seeding pattern as apt/dnf,
    // via one batched `zypper search` call (zypper OR's multiple search
    // terms together) instead of one process per candidate app.
    let mut args = vec!["--non-interactive", "--no-refresh", "--xmlout", "search", "--type", "package"];
    args.extend_from_slice(POPULAR_APPS);
    let search_raw = run_timeout("zypper", &args, RUN_TIMEOUT_NETWORK);
    let mut seen_available: std::collections::HashSet<String> = Default::default();
    for line in search_raw.lines() {
        if !line.contains("<solvable ") || !line.contains("kind=\"package\"") { continue; }
        let get = |attr: &str| -> String {
            let key = format!("{}=\"", attr);
            line.find(&key)
                .map(|i| {
                    let start = i + key.len();
                    let end = line[start..].find('"').map(|j| start + j).unwrap_or(start);
                    line[start..end].to_string()
                })
                .unwrap_or_default()
        };
        let name = get("name");
        if name.is_empty() || installed_names.contains(&name) || seen_available.contains(&name) { continue; }
        // Only keep results that actually match a requested popular-app
        // name — zypper's search can return loosely-related packages
        // too (subpackages, -devel variants, etc.) which would otherwise
        // flood "Available" with noise the other backends don't have.
        if !POPULAR_APPS.iter().any(|p| *p == name) { continue; }
        seen_available.insert(name.clone());
        pkgs.push(PackageInfo {
            id: name.clone(), name: name.clone(),
            description: get("summary"), version: get("edition"),
            source: "zypper".to_string(), installed: false,
            update_available: None,
            icon: icon_for_package(&name), size: None,
        });
    }

    pkgs
}

pub fn install_zypper(pkg: &str) -> bool {
    Command::new("pkexec").args(["zypper", "--non-interactive", "install", pkg])
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn remove_zypper(pkg: &str) -> bool {
    Command::new("pkexec").args(["zypper", "--non-interactive", "remove", pkg])
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn update_zypper(pkg: &str) -> bool {
    Command::new("pkexec").args(["zypper", "--non-interactive", "update", pkg])
        .status().map(|s| s.success()).unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
// rpm-ostree backend (Fedora Silverblue / Universal Blue / bootc)
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_rpm_ostree_packages() -> Vec<PackageInfo> {
    if !run_check("rpm-ostree") { return Vec::new(); }
    // Get layered packages (user-installed on top of base image)
    let raw = run("rpm-ostree", &["status", "--json"]);
    let mut layered: Vec<String> = Vec::new();
    let mut update_pending = false;
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
        if let Some(deployments) = json["deployments"].as_array() {
            if let Some(booted) = deployments.iter().find(|d| d["booted"].as_bool() == Some(true)) {
                if let Some(pkgs) = booted["requested-local-packages"].as_array() {
                    layered = pkgs.iter().filter_map(|p| p.as_str().map(String::from)).collect();
                }
            }
            // An ostree deployment listed but not currently booted (and
            // not the previous rollback slot) means an update has
            // already been staged and is waiting for a reboot to apply
            // — `rpm-ostree upgrade` was run (or auto-staged), same
            // concept as `bootc`'s `update_pending` above. Unlike
            // apt/dnf/pacman, updates here are atomic and all-or-nothing:
            // there's no meaningful "this one package has an update"
            // distinction, so every package is flagged together.
            update_pending = deployments.iter().any(|d| d["staged"].as_bool() == Some(true));
        }
    }
    // Base image RPMs — a fully local `rpm -qa` query, deliberately NOT
    // `get_dnf_packages()` anymore. That used to run here, layering in
    // dnf's `check-update`/`repoquery` (each up to 25s) for an
    // "Available" tab and update badges — but on real rpm-ostree systems
    // (this was specifically reported broken on Bazzite) `dnf`'s repos
    // usually aren't configured for meaningful live queries at all,
    // since package management goes through `rpm-ostree`/Flatpak
    // instead. Those two sequential calls failing slowly rather than
    // failing fast could take up to 50s combined — blowing straight
    // through the frontend's 40s safety-net timeout and presenting as
    // "Blue Software is just broken" on exactly this kind of system.
    // `rpm -qa` is 100% local and near-instant regardless. The
    // "Available to install" use case is already covered by the
    // separate, independent Flatpak listing in the Promise.all this
    // feeds into — the actual way you install apps on an rpm-ostree
    // desktop like Bazzite.
    let qa_raw = run("rpm", &["-qa", "--qf", "%{NAME}|%{VERSION}-%{RELEASE}|%{SUMMARY}\n"]);
    let mut all: Vec<PackageInfo> = qa_raw.lines().filter_map(|line| {
        let p: Vec<&str> = line.splitn(3, '|').collect();
        if p.len() < 3 || p[0].is_empty() { return None; }
        Some(PackageInfo {
            id: p[0].to_string(), name: p[0].to_string(),
            description: p[2].to_string(), version: p[1].to_string(),
            source: "rpm-ostree".to_string(), installed: true,
            update_available: if update_pending { Some(true) } else { None },
            icon: icon_for_package(p[0]), size: None,
        })
    }).collect();
    for pkg_name in layered {
        if all.iter().any(|p: &PackageInfo| p.name == pkg_name) { continue; }
        // Layered packages aren't in the base image's rpmdb query above,
        // but they are installed system RPMs — `rpm -q` resolves their
        // real installed version instead of leaving it blank.
        let version = run("rpm", &["-q", "--qf", "%{VERSION}-%{RELEASE}", &pkg_name]);
        all.push(PackageInfo {
            id: pkg_name.clone(), name: pkg_name.clone(),
            description: "Layered package (installed on top of the base image)".to_string(),
            version: if version.starts_with("package") { String::new() } else { version },
            source: "rpm-ostree".to_string(),
            installed: true, update_available: Some(update_pending),
            icon: icon_for_package(&pkg_name), size: None,
        });
    }
    all
}

pub fn install_rpm_ostree(pkg: &str) -> bool {
    Command::new("pkexec").args(["rpm-ostree", "install", "--idempotent", pkg])
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn remove_rpm_ostree(pkg: &str) -> bool {
    Command::new("pkexec").args(["rpm-ostree", "uninstall", pkg])
        .status().map(|s| s.success()).unwrap_or(false)
}

/// Stages the next ostree deployment (applied on next reboot). This is
/// the rpm-ostree equivalent of `bootc_upgrade` — there's no per-package
/// upgrade command because updates are atomic, whole-image swaps.
pub fn rpm_ostree_upgrade() -> bool {
    Command::new("pkexec").args(["rpm-ostree", "upgrade"])
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn update_rpm_ostree(_pkg: &str) -> bool {
    // rpm-ostree updates happen system-wide; trigger an upgrade
    Command::new("pkexec").args(["rpm-ostree", "upgrade"])
        .status().map(|s| s.success()).unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
// bootc backend (image-based / OCI)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct BootcStatus {
    pub image:          String,
    pub version:        String,
    pub booted_digest:  String,
    pub staged_image:   Option<String>,
    pub update_pending: bool,
}

pub fn get_bootc_status() -> Option<BootcStatus> {
    if !run_check("bootc") { return None; }
    let raw = run("bootc", &["status", "--json"]);
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;

    let spec   = &json["spec"];
    let status = &json["status"]["booted"];
    Some(BootcStatus {
        image:          spec["image"]["image"].as_str().unwrap_or("unknown").to_string(),
        version:        status["image"]["version"].as_str().unwrap_or("").to_string(),
        booted_digest:  status["image"]["imageDigest"].as_str().unwrap_or("").to_string(),
        staged_image:   json["status"]["staged"]["image"]["image"].as_str().map(String::from),
        update_pending: json["status"]["staged"].is_object(),
    })
}

pub fn bootc_upgrade() -> bool {
    Command::new("pkexec").args(["bootc", "upgrade"])
        .status().map(|s| s.success()).unwrap_or(false)
}

pub fn bootc_switch(image: &str) -> bool {
    Command::new("pkexec").args(["bootc", "switch", image])
        .status().map(|s| s.success()).unwrap_or(false)
}

// ─────────────────────────────────────────────────────────────────────────────
// Auto-detect package manager
// ─────────────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum PkgBackend {
    Dnf, Apt, Pacman, Zypper, RpmOstree, Flatpak, AppImage, Unknown,
}

pub fn detect_backend() -> PkgBackend {
    // Check env var set by build.rb shell variant first
    if let Ok(pm) = std::env::var("BLUE_PKG_MANAGER") {
        return match pm.as_str() {
            "debian" | "apt"        => PkgBackend::Apt,
            "arch"   | "pacman"     => PkgBackend::Pacman,
            "opensuse"| "zypper"    => PkgBackend::Zypper,
            "rpm-ostree"            => PkgBackend::RpmOstree,
            "fedora" | "dnf"        => PkgBackend::Dnf,
            _                       => PkgBackend::Unknown,
        };
    }
    // Auto-detect from installed tools
    if run_check("rpm-ostree") { return PkgBackend::RpmOstree; }
    if run_check("apt-get")    { return PkgBackend::Apt; }
    if run_check("pacman")     { return PkgBackend::Pacman; }
    if run_check("zypper")     { return PkgBackend::Zypper; }
    if run_check("dnf") || run_check("rpm") { return PkgBackend::Dnf; }
    PkgBackend::Unknown
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
