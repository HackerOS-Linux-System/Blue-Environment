use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const MAX_ARCHIVE_BYTES: u64 = 800 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 3 * 1024 * 1024 * 1024;
const MAX_MEMBERS: usize = 400_000;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Kind { Zip, Tar }

fn archive_kind(file_name: &str) -> Option<Kind> {
    let n = file_name.to_lowercase();
    if n.ends_with(".zip") { return Some(Kind::Zip); }
    for ext in [".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tbz", ".tar.xz", ".txz", ".tar.zst", ".tzst"] {
        if n.ends_with(ext) { return Some(Kind::Tar); }
    }
    None
}

/// A single archive member path is acceptable only if it stays inside the extraction root.
fn is_safe_member(name: &str) -> bool {
    if name.is_empty() || name.contains('\0') { return false; }
    // zips made on Windows use backslashes — treat them as separators too
    let unified = name.replace('\\', "/");
    if unified.starts_with('/') { return false; }
    // `C:foo` style drive prefixes
    if unified.len() >= 2 && unified.as_bytes()[1] == b':' && unified.as_bytes()[0].is_ascii_alphabetic() { return false; }
    !unified.split('/').any(|c| c == "..")
}

/// A theme directory name that is safe to create under the icons dir.
fn valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && !name.starts_with('.')
        && !name.contains('/') && !name.contains('\\') && !name.contains('\0')
        && name != "." && name != ".."
}

/// Lexical path normalisation (no filesystem access): resolves `.` and `..`.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                // `/..` is `/` (POSIX); only a relative path keeps leading `..`
                let at_root = out.has_root() && out.components().count() == 1;
                if !at_root && !out.pop() { out.push(".."); }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn run_ok(cmd: &mut Command, what: &str) -> Result<std::process::Output, String> {
    let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() { Ok(out) } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("{what} failed: {}", err.lines().next().unwrap_or("unknown error")))
    }
}

fn list_members(archive: &Path, kind: Kind) -> Result<Vec<String>, String> {
    let out = match kind {
        Kind::Tar => run_ok(Command::new("tar").arg("-tf").arg(archive), "tar")?,
        Kind::Zip => run_ok(Command::new("unzip").arg("-Z1").arg(archive), "unzip (is it installed?)")?,
    };
    Ok(String::from_utf8_lossy(&out.stdout).lines().map(|s| s.to_string()).collect())
}

fn extract(archive: &Path, kind: Kind, dest: &Path) -> Result<(), String> {
    match kind {
        Kind::Tar => run_ok(
            Command::new("tar").arg("-xf").arg(archive).arg("-C").arg(dest).arg("--no-same-owner").arg("--no-same-permissions"),
            "tar",
        ).map(|_| ()),
        Kind::Zip => run_ok(Command::new("unzip").arg("-qq").arg("-o").arg(archive).arg("-d").arg(dest), "unzip").map(|_| ()),
    }
}

/// Removes everything dangerous from an extracted tree and returns its total file size.
fn sanitize_tree(root: &Path) -> Result<u64, String> {
    let canon_root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let mut total: u64 = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) { Ok(e) => e, Err(_) => continue };
        for entry in entries.flatten() {
            let p = entry.path();
            let Ok(meta) = fs::symlink_metadata(&p) else { continue };
            let ft = meta.file_type();
            if ft.is_symlink() {
                let keep = match fs::read_link(&p) {
                    Err(_) => false,
                    Ok(target) => {
                        if target.is_absolute() { false } else {
                            let lexical = normalize(&dir.join(&target));
                            let lexical_ok = lexical.starts_with(&normalize(root));
                            // Resolve for real too: a chain such as `x -> .` + `l -> x/..` is
                            // inside lexically but escapes once the filesystem resolves it.
                            let real_ok = match fs::canonicalize(&p) {
                                Ok(real) => real.starts_with(&canon_root),
                                Err(e) => e.kind() == std::io::ErrorKind::NotFound, // dangling: harmless
                            };
                            lexical_ok && real_ok
                        }
                    }
                };
                if !keep { let _ = fs::remove_file(&p); }
            } else if ft.is_dir() {
                stack.push(p);
            } else if ft.is_file() {
                total = total.saturating_add(meta.len());
                if total > MAX_EXTRACTED_BYTES {
                    return Err("Archive is too large once extracted".to_string());
                }
            } else {
                // FIFOs, sockets, device nodes — never part of an icon theme
                let _ = fs::remove_file(&p);
            }
        }
    }
    Ok(total)
}

/// True when `dir` is a real icon theme: `index.theme` with `[Icon Theme]` and a
/// `Directories=` list. Cursor-only themes (also `index.theme`, but `cursors/` and no
/// `Directories=`) are excluded.
pub fn is_icon_theme_dir(dir: &Path) -> bool {
    let Ok(text) = fs::read_to_string(dir.join("index.theme")) else { return false };
    if !text.contains("[Icon Theme]") { return false; }
    text.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("Directories=") || l.starts_with("Directories =")
    })
}

/// Theme directories inside `staging` (depth 1–2; a wrapper folder is common).
fn find_theme_dirs(staging: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(top) = fs::read_dir(staging) else { return found };
    for a in top.flatten() {
        let pa = a.path();
        if !fs::symlink_metadata(&pa).map(|m| m.is_dir()).unwrap_or(false) { continue; }
        if is_icon_theme_dir(&pa) { found.push(pa); continue; }
        if let Ok(inner) = fs::read_dir(&pa) {
            for b in inner.flatten() {
                let pb = b.path();
                if fs::symlink_metadata(&pb).map(|m| m.is_dir()).unwrap_or(false) && is_icon_theme_dir(&pb) {
                    found.push(pb);
                }
            }
        }
    }
    found.sort();
    found
}

fn unique_suffix() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{}-{}", std::process::id(), nanos)
}

/// Core installer — pure with respect to `icons_root`, so it is unit-testable.
/// Returns the names of the installed themes.
pub fn install_archive_into(archive: &Path, icons_root: &Path) -> Result<Vec<String>, String> {
    let meta = fs::metadata(archive).map_err(|e| format!("Cannot read the file: {e}"))?;
    if !meta.is_file() { return Err("Not a file".to_string()); }
    if meta.len() > MAX_ARCHIVE_BYTES { return Err("Archive is too large (over 800 MB)".to_string()); }
    let name = archive.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let kind = archive_kind(&name).ok_or_else(|| "Unsupported archive type (use .tar.gz, .tar.xz, .tar.bz2 or .zip)".to_string())?;

    // 1) validate every member before writing anything
    let members = list_members(archive, kind)?;
    if members.is_empty() { return Err("The archive is empty".to_string()); }
    if members.len() > MAX_MEMBERS { return Err("The archive has too many files".to_string()); }
    if let Some(bad) = members.iter().find(|m| !is_safe_member(m)) {
        return Err(format!("Unsafe path in archive: {bad}"));
    }

    // 2) extract into a private staging dir on the same filesystem as the target
    fs::create_dir_all(icons_root).map_err(|e| e.to_string())?;
    let staging = icons_root.join(format!(".blue-install-{}", unique_suffix()));
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let result = (|| -> Result<Vec<String>, String> {
        extract(archive, kind, &staging)?;
        sanitize_tree(&staging)?;
        let themes = find_theme_dirs(&staging);
        if themes.is_empty() {
            return Err("This archive does not contain an icon theme (no index.theme with icon directories)".to_string());
        }
        let mut installed = Vec::new();
        for src in themes {
            let theme_name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if !valid_theme_name(&theme_name) { continue; }
            let dest = icons_root.join(&theme_name);
            // never write through a symlink, and never replace something that is not a directory
            if let Ok(m) = fs::symlink_metadata(&dest) {
                if !m.is_dir() { return Err(format!("{theme_name} exists and is not a folder")); }
                let old = icons_root.join(format!(".blue-old-{}", unique_suffix()));
                fs::rename(&dest, &old).map_err(|e| e.to_string())?;
                if let Err(e) = fs::rename(&src, &dest) {
                    let _ = fs::rename(&old, &dest); // put the previous version back
                    return Err(e.to_string());
                }
                let _ = fs::remove_dir_all(&old);
            } else {
                fs::rename(&src, &dest).map_err(|e| e.to_string())?;
            }
            installed.push(theme_name);
        }
        if installed.is_empty() { return Err("No installable icon theme found".to_string()); }
        Ok(installed)
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

// ── Tauri commands ──────────────────────────────────────────────────────

fn user_icon_roots() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    vec![home.join(".local/share/icons"), home.join(".icons")]
}

fn refresh_icon_caches() {
    // App icons are resolved once and cached; a new theme must re-resolve them.
    crate::cache::invalidate_app_cache();
    crate::apps::clear_memory_cache();
}

#[derive(Serialize)]
pub struct IconThemeInfo {
    pub name: String,
    pub path: String,
    /// Lives under the user's home → can be removed from Blue Software.
    pub removable: bool,
    /// Currently selected as Blue Environment's icon theme.
    pub active: bool,
}

/// Installs an icon-theme archive (as downloaded from the KDE Store) for the current user.
#[tauri::command(async)]
pub fn icon_store_install(path: String) -> Result<Vec<String>, String> {
    let root = user_icon_roots().remove(0);
    let installed = install_archive_into(Path::new(&path), &root)?;
    refresh_icon_caches();
    Ok(installed)
}

/// Every installed icon theme (system + user), with whether it can be removed.
#[tauri::command(async)]
pub fn icon_store_list() -> Vec<IconThemeInfo> {
    let home = dirs::home_dir().unwrap_or_default();
    let active = crate::icon_resolver::get_icon_theme();
    let roots = [
        PathBuf::from("/usr/share/icons"),
        home.join(".local/share/icons"),
        home.join(".icons"),
    ];
    let mut out: Vec<IconThemeInfo> = Vec::new();
    for root in &roots {
        let Ok(entries) = fs::read_dir(root) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            let Some(name) = p.file_name().map(|n| n.to_string_lossy().to_string()) else { continue };
            if name.starts_with('.') || !p.is_dir() || !is_icon_theme_dir(&p) { continue; }
            if out.iter().any(|t| t.name == name) { continue; }
            out.push(IconThemeInfo {
                removable: p.starts_with(&home),
                active: active.as_deref() == Some(name.as_str()),
                name,
                path: p.to_string_lossy().to_string(),
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Removes a user-installed icon theme. System themes can never be removed here.
#[tauri::command(async)]
pub fn icon_store_remove(name: String) -> Result<(), String> {
    if !valid_theme_name(&name) { return Err("Invalid theme name".to_string()); }
    for root in user_icon_roots() {
        let dir = root.join(&name);
        let Ok(meta) = fs::symlink_metadata(&dir) else { continue };
        if meta.file_type().is_symlink() || !meta.is_dir() { return Err("Refusing to remove a non-folder".to_string()); }
        if !dir.join("index.theme").exists() { return Err("Not an icon theme".to_string()); }
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        if crate::icon_resolver::get_icon_theme().as_deref() == Some(name.as_str()) {
            crate::icon_resolver::set_icon_theme(None);
        }
        refresh_icon_caches();
        return Ok(());
    }
    Err("Only icon themes installed in your home folder can be removed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("blue-icons-{}-{}-{}", name, std::process::id(), unique_suffix()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn have(cmd: &str) -> bool { Command::new(cmd).arg("--help").output().is_ok() }

    fn make_theme(root: &Path, name: &str) -> PathBuf {
        let t = root.join(name);
        fs::create_dir_all(t.join("48x48/apps")).unwrap();
        fs::write(t.join("index.theme"), "[Icon Theme]\nName=Test\nDirectories=48x48/apps\n\n[48x48/apps]\nSize=48\nType=Fixed\n").unwrap();
        fs::write(t.join("48x48/apps/firefox.png"), b"\x89PNG").unwrap();
        t
    }

    #[test]
    fn member_paths() {
        for ok in ["Theme/index.theme", "a/b/c.png", "./a", "Theme/48x48/apps/x.svg"] { assert!(is_safe_member(ok), "{ok}"); }
        for bad in ["", "/etc/passwd", "../x", "a/../../x", "a/..", "..\\x", "C:\\x", "C:x", "a\0b"] { assert!(!is_safe_member(bad), "{bad:?}"); }
    }

    #[test]
    fn theme_names() {
        assert!(valid_theme_name("Papirus-Dark"));
        for bad in ["", ".hidden", "..", "a/b", "a\\b", &"x".repeat(101)] { assert!(!valid_theme_name(bad), "{bad}"); }
    }

    #[test]
    fn archive_kinds() {
        assert_eq!(archive_kind("Foo.tar.gz"), Some(Kind::Tar));
        assert_eq!(archive_kind("FOO.TAR.XZ"), Some(Kind::Tar));
        assert_eq!(archive_kind("a.zip"), Some(Kind::Zip));
        assert_eq!(archive_kind("a.7z"), None);
        assert_eq!(archive_kind("a.png"), None);
    }

    #[test]
    fn normalizes_lexically() {
        assert_eq!(normalize(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
        assert_eq!(normalize(Path::new("/a/../../b")), PathBuf::from("/b"));
    }

    #[test]
    fn classifies_icon_vs_cursor_themes() {
        let root = tmp("classify");
        let t = make_theme(&root, "Good");
        assert!(is_icon_theme_dir(&t));
        let c = root.join("Cursors");
        fs::create_dir_all(c.join("cursors")).unwrap();
        fs::write(c.join("index.theme"), "[Icon Theme]\nName=Cursors\n").unwrap();
        assert!(!is_icon_theme_dir(&c));
        assert!(!is_icon_theme_dir(&root.join("missing")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn installs_a_tar_theme_and_prunes_escaping_symlinks() {
        if !have("tar") { return; }
        let work = tmp("src"); let icons = tmp("dest");
        let t = make_theme(&work, "MyIcons");
        #[cfg(unix)] {
            std::os::unix::fs::symlink("firefox.png", t.join("48x48/apps/web.png")).unwrap();          // inside → kept
            std::os::unix::fs::symlink("../../../../outside", t.join("48x48/apps/evil")).unwrap();     // escapes → removed
            std::os::unix::fs::symlink("/etc/passwd", t.join("48x48/apps/abs")).unwrap();              // absolute → removed
        }
        let archive = work.join("MyIcons.tar.gz");
        assert!(Command::new("tar").arg("-czf").arg(&archive).arg("-C").arg(&work).arg("MyIcons").status().unwrap().success());

        let names = install_archive_into(&archive, &icons).unwrap();
        assert_eq!(names, vec!["MyIcons"]);
        let dest = icons.join("MyIcons");
        assert!(dest.join("index.theme").exists());
        assert!(dest.join("48x48/apps/firefox.png").exists());
        #[cfg(unix)] {
            assert!(fs::symlink_metadata(dest.join("48x48/apps/web.png")).is_ok(), "safe symlink must survive");
            assert!(fs::symlink_metadata(dest.join("48x48/apps/evil")).is_err(), "escaping symlink must be removed");
            assert!(fs::symlink_metadata(dest.join("48x48/apps/abs")).is_err(), "absolute symlink must be removed");
        }
        // staging cleaned up: only the theme remains
        let left: Vec<_> = fs::read_dir(&icons).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(left, vec!["MyIcons"]);
        let _ = fs::remove_dir_all(&work); let _ = fs::remove_dir_all(&icons);
    }

    #[test]
    fn reinstall_replaces_the_previous_version() {
        if !have("tar") { return; }
        let work = tmp("re-src"); let icons = tmp("re-dest");
        let t = make_theme(&work, "Again");
        let archive = work.join("Again.tar");
        assert!(Command::new("tar").arg("-cf").arg(&archive).arg("-C").arg(&work).arg("Again").status().unwrap().success());
        install_archive_into(&archive, &icons).unwrap();
        fs::write(icons.join("Again/stale.txt"), "old").unwrap();
        fs::write(t.join("new.txt"), "new").unwrap();
        assert!(Command::new("tar").arg("-cf").arg(&archive).arg("-C").arg(&work).arg("Again").status().unwrap().success());
        install_archive_into(&archive, &icons).unwrap();
        assert!(icons.join("Again/new.txt").exists());
        assert!(!icons.join("Again/stale.txt").exists(), "old version fully replaced");
        let _ = fs::remove_dir_all(&work); let _ = fs::remove_dir_all(&icons);
    }

    #[test]
    fn accepts_a_wrapper_folder_and_multiple_themes() {
        if !have("tar") { return; }
        let work = tmp("wrap-src"); let icons = tmp("wrap-dest");
        let wrapper = work.join("pack");
        fs::create_dir_all(&wrapper).unwrap();
        make_theme(&wrapper, "One"); make_theme(&wrapper, "Two");
        let archive = work.join("pack.tar.gz");
        assert!(Command::new("tar").arg("-czf").arg(&archive).arg("-C").arg(&work).arg("pack").status().unwrap().success());
        let mut names = install_archive_into(&archive, &icons).unwrap();
        names.sort();
        assert_eq!(names, vec!["One", "Two"]);
        let _ = fs::remove_dir_all(&work); let _ = fs::remove_dir_all(&icons);
    }

    #[test]
    fn rejects_traversal_non_themes_and_bad_types_without_writing() {
        if !have("tar") { return; }
        let work = tmp("bad-src"); let icons = tmp("bad-dest");
        // path traversal member
        fs::write(work.join("f.txt"), "x").unwrap();
        let evil = work.join("evil.tar");
        assert!(Command::new("tar").arg("-cf").arg(&evil).arg("-C").arg(&work).arg("--transform").arg("s,^,../,").arg("f.txt").status().unwrap().success());
        let err = install_archive_into(&evil, &icons).unwrap_err();
        assert!(err.contains("Unsafe path"), "{err}");
        assert!(!icons.parent().unwrap().join("f.txt").exists());
        // archive without a theme
        let plain = work.join("plain.tar.gz");
        assert!(Command::new("tar").arg("-czf").arg(&plain).arg("-C").arg(&work).arg("f.txt").status().unwrap().success());
        assert!(install_archive_into(&plain, &icons).unwrap_err().contains("does not contain an icon theme"));
        // unsupported extension / missing file
        assert!(install_archive_into(&work.join("f.txt"), &icons).unwrap_err().contains("Unsupported"));
        assert!(install_archive_into(&work.join("nope.tar"), &icons).is_err());
        // nothing but the (empty) icons dir remains
        assert_eq!(fs::read_dir(&icons).unwrap().count(), 0, "staging must be cleaned up after failures");
        let _ = fs::remove_dir_all(&work); let _ = fs::remove_dir_all(&icons);
    }

    #[test]
    fn installs_from_zip_when_unzip_is_available() {
        if !have("tar") || Command::new("unzip").arg("-v").output().is_err() || Command::new("zip").arg("-v").output().is_err() { return; }
        let work = tmp("zip-src"); let icons = tmp("zip-dest");
        make_theme(&work, "Zipped");
        let archive = work.join("Zipped.zip");
        assert!(Command::new("zip").current_dir(&work).arg("-qr").arg(&archive).arg("Zipped").status().unwrap().success());
        assert_eq!(install_archive_into(&archive, &icons).unwrap(), vec!["Zipped"]);
        let _ = fs::remove_dir_all(&work); let _ = fs::remove_dir_all(&icons);
    }
}
