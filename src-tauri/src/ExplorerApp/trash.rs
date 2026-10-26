use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_SIZE_WALK: usize = 200_000;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TrashEntry {
    /// Name under `files/` — the handle every other command takes.
    pub id: String,
    /// The original file name (what the person should see).
    pub name: String,
    pub original_path: String,
    pub deleted_at: String,
    pub is_dir: bool,
    pub size_bytes: u64,
}

// ── Locations ───────────────────────────────────────────────────────────────

pub fn trash_root() -> PathBuf {
    dirs::cache_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".cache")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Blue-Environment")
        .join("trash")
}

/// Creates `trash/`, `trash/files/` and `trash/info/` when missing and returns the root.
pub fn ensure_trash() -> Result<PathBuf, String> {
    let root = trash_root();
    for sub in ["files", "info"] {
        fs::create_dir_all(root.join(sub)).map_err(|e| format!("Cannot create the Trash folder ({}): {e}", root.join(sub).display()))?;
    }
    Ok(root)
}

// ── Small helpers (pure → unit tested) ──────────────────────────────────────

/// Percent-encodes a path the way the Trash spec wants it (keeps `/` and unreserved bytes).
pub fn encode_path(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for b in p.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub fn decode_path(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = hex {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// An id is a single file name — never a path. Blocks `..`, `/`, NUL (path traversal).
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id != "." && id != ".." && !id.contains('/') && !id.contains('\0')
}

fn parse_info(text: &str) -> Option<(String, String, Option<u64>, Option<bool>)> {
    let mut path = None;
    let mut date = String::new();
    let mut size = None;
    let mut is_dir = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("Path=") {
            path = Some(decode_path(v.trim()));
        } else if let Some(v) = line.strip_prefix("DeletionDate=") {
            date = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Size=") {
            size = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("IsDir=") {
            is_dir = Some(v.trim() == "true");
        }
    }
    path.map(|p| (p, date, size, is_dir))
}

fn disk_size(p: &Path) -> u64 {
    fn walk(p: &Path, budget: &mut usize) -> u64 {
        let Ok(meta) = fs::symlink_metadata(p) else { return 0 };
        if meta.is_dir() {
            let Ok(rd) = fs::read_dir(p) else { return 0 };
            let mut total = 0;
            for e in rd.flatten() {
                if *budget == 0 {
                    break;
                }
                *budget -= 1;
                total += walk(&e.path(), budget);
            }
            total
        } else {
            meta.len()
        }
    }
    let mut budget = MAX_SIZE_WALK;
    walk(p, &mut budget)
}

fn unique_id(files: &Path, info: &Path, name: &str) -> String {
    let taken = |id: &str| files.join(id).symlink_metadata().is_ok() || info.join(format!("{id}.trashinfo")).exists();
    if !taken(name) {
        return name.to_string();
    }
    let mut n = 2;
    loop {
        let cand = format!("{name}.{n}");
        if !taken(&cand) {
            return cand;
        }
        n += 1;
    }
}

fn remove_any(p: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(p),
        Ok(_) => fs::remove_file(p),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// rename, falling back to copy+delete across filesystems (Home on another disk than ~/.cache).
fn move_path(src: &Path, dst: &Path) -> Result<(), String> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(18) => {
            copy_recursive(src, dst).map_err(|e| e.to_string())?;
            remove_any(src).map_err(|e| e.to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn copy_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        std::os::unix::fs::symlink(fs::read_link(src)?, dst)
    } else if meta.is_dir() {
        fs::create_dir_all(dst)?;
        for e in fs::read_dir(src)? {
            let e = e?;
            copy_recursive(&e.path(), &dst.join(e.file_name()))?;
        }
        fs::set_permissions(dst, meta.permissions()).ok();
        Ok(())
    } else {
        fs::copy(src, dst).map(|_| ())
    }
}

// ── Core operations on an explicit root (testable) ──────────────────────────

pub fn trash_into(root: &Path, p: &Path) -> Result<String, String> {
    let files = root.join("files");
    let info = root.join("info");
    fs::create_dir_all(&files).map_err(|e| e.to_string())?;
    fs::create_dir_all(&info).map_err(|e| e.to_string())?;

    let meta = fs::symlink_metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let abs = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().map_err(|e| e.to_string())?.join(p) };
    if abs.starts_with(root) {
        return Err("This item is already in the Trash".into());
    }
    if abs.parent().is_none() || dirs::home_dir().map(|h| h == abs).unwrap_or(false) {
        return Err(format!("Refusing to trash {}", abs.display()));
    }
    let name = abs.file_name().ok_or("Invalid file name")?.to_string_lossy().to_string();
    let id = unique_id(&files, &info, &name);
    let is_dir = meta.is_dir();
    let size = disk_size(&abs);
    let when = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let info_path = info.join(format!("{id}.trashinfo"));
    fs::write(
        &info_path,
        format!("[Trash Info]\nPath={}\nDeletionDate={}\nSize={}\nIsDir={}\n", encode_path(&abs.to_string_lossy()), when, size, is_dir),
    )
    .map_err(|e| e.to_string())?;

    if let Err(e) = move_path(&abs, &files.join(&id)) {
        let _ = fs::remove_file(&info_path); // never leave an info file without its item
        return Err(e);
    }
    Ok(id)
}

pub fn list_in(root: &Path) -> Vec<TrashEntry> {
    let files = root.join("files");
    let info = root.join("info");
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(&files) else { return out };
    for e in rd.flatten() {
        let id = e.file_name().to_string_lossy().to_string();
        let item = e.path();
        let meta = fs::symlink_metadata(&item).ok();
        let parsed = fs::read_to_string(info.join(format!("{id}.trashinfo"))).ok().and_then(|t| parse_info(&t));
        // An item without (readable) info is still shown — it can be deleted, and restored to Home.
        let (orig, date, size, is_dir) = match parsed {
            Some((p, d, s, dir)) => (p, d, s, dir),
            None => (String::new(), String::new(), None, None),
        };
        let is_dir = is_dir.unwrap_or_else(|| meta.as_ref().map(|m| m.is_dir()).unwrap_or(false));
        let name = if orig.is_empty() {
            id.clone()
        } else {
            Path::new(&orig).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| id.clone())
        };
        out.push(TrashEntry {
            size_bytes: size.unwrap_or_else(|| meta.as_ref().map(|m| if m.is_dir() { 0 } else { m.len() }).unwrap_or(0)),
            id,
            name,
            original_path: orig,
            deleted_at: date,
            is_dir,
        });
    }
    // Newest first (ISO timestamps sort lexicographically), then by name.
    out.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at).then_with(|| a.name.cmp(&b.name)));
    out
}

pub fn restore_in(root: &Path, id: &str) -> Result<String, String> {
    if !valid_id(id) {
        return Err("Invalid Trash item".into());
    }
    let item = root.join("files").join(id);
    if item.symlink_metadata().is_err() {
        return Err(format!("\"{id}\" is no longer in the Trash"));
    }
    let info_path = root.join("info").join(format!("{id}.trashinfo"));
    let orig = fs::read_to_string(&info_path).ok().and_then(|t| parse_info(&t)).map(|(p, ..)| p);
    let mut dest = match orig {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")).join(id), // no info: Home is the safest guess
    };
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot recreate {}: {e}", parent.display()))?;
    }
    if dest.symlink_metadata().is_ok() {
        // Never overwrite what is there now.
        let parent = dest.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let stem = dest.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = dest.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let mut n = 1;
        loop {
            let tag = if n == 1 { " (restored)".to_string() } else { format!(" (restored {n})") };
            let cand = parent.join(format!("{stem}{tag}{ext}"));
            if cand.symlink_metadata().is_err() {
                dest = cand;
                break;
            }
            n += 1;
        }
    }
    move_path(&item, &dest)?;
    let _ = fs::remove_file(&info_path);
    Ok(dest.to_string_lossy().to_string())
}

pub fn purge_in(root: &Path, id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("Invalid Trash item".into());
    }
    remove_any(&root.join("files").join(id)).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(root.join("info").join(format!("{id}.trashinfo")));
    Ok(())
}

pub fn empty_in(root: &Path) -> Result<usize, String> {
    let mut n = 0;
    for sub in ["files", "info"] {
        if let Ok(rd) = fs::read_dir(root.join(sub)) {
            for e in rd.flatten() {
                remove_any(&e.path()).map_err(|er| er.to_string())?;
                if sub == "files" {
                    n += 1;
                }
            }
        }
    }
    Ok(n)
}

// ── Tauri commands ──────────────────────────────────────────────────────────

/// Absolute path of the Trash folder (created if missing).
#[tauri::command(async)]
pub fn get_trash_path() -> Result<String, String> {
    ensure_trash().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command(async)]
pub fn move_to_trash(paths: Vec<String>) -> Result<(), String> {
    let root = ensure_trash()?;
    let mut failed = Vec::new();
    for raw in paths {
        let p = super::resolve_path(&raw);
        if let Err(e) = trash_into(&root, &p) {
            failed.push(format!("{} ({e})", p.display()));
        }
    }
    if failed.is_empty() { Ok(()) } else { Err(format!("Could not move to Trash: {}", failed.join(", "))) }
}

#[tauri::command(async)]
pub fn list_trash() -> Result<Vec<TrashEntry>, String> {
    Ok(list_in(&ensure_trash()?))
}

#[tauri::command(async)]
pub fn restore_from_trash(ids: Vec<String>) -> Result<Vec<String>, String> {
    let root = ensure_trash()?;
    let mut restored = Vec::new();
    let mut errors = Vec::new();
    for id in ids {
        match restore_in(&root, &id) {
            Ok(p) => restored.push(p),
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() { Ok(restored) } else { Err(errors.join("; ")) }
}

#[tauri::command(async)]
pub fn delete_from_trash(ids: Vec<String>) -> Result<(), String> {
    let root = ensure_trash()?;
    let mut errors = Vec::new();
    for id in ids {
        if let Err(e) = purge_in(&root, &id) {
            errors.push(e);
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

#[tauri::command(async)]
pub fn empty_trash() -> Result<usize, String> {
    empty_in(&ensure_trash()?)
}

#[tauri::command(async)]
pub fn trash_item_count() -> usize {
    list_in(&ensure_trash().unwrap_or_else(|_| trash_root())).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("blue-trash-test-{}-{}", std::process::id(), chrono::Local::now().timestamp_nanos_opt().unwrap_or(0)));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn path_encoding_round_trips() {
        for p in ["/home/u/plain.txt", "/home/u/a b/ż ó ł 🙂.txt", "/x/100%/y#z?.md"] {
            assert_eq!(decode_path(&encode_path(p)), p);
        }
        assert_eq!(encode_path("/a b"), "/a%20b");
    }

    #[test]
    fn ids_cannot_escape() {
        assert!(!valid_id("../etc"));
        assert!(!valid_id("a/b"));
        assert!(!valid_id(".."));
        assert!(!valid_id(""));
        assert!(valid_id("report.pdf.2"));
    }

    #[test]
    fn trash_list_restore_purge_cycle() {
        let base = tmp();
        let root = base.join("trash"); // does not exist yet → must be created
        let work = base.join("work");
        fs::create_dir_all(work.join("sub")).unwrap();
        fs::write(work.join("a b.txt"), "hello").unwrap();
        fs::write(work.join("sub/inner.bin"), vec![0u8; 1000]).unwrap();

        let id1 = trash_into(&root, &work.join("a b.txt")).unwrap();
        let id2 = trash_into(&root, &work.join("sub")).unwrap();
        assert!(root.join("files").is_dir() && root.join("info").is_dir(), "structure created on demand");
        assert!(!work.join("a b.txt").exists() && !work.join("sub").exists());

        let list = list_in(&root);
        assert_eq!(list.len(), 2);
        let f = list.iter().find(|e| e.id == id1).unwrap();
        assert_eq!((f.name.as_str(), f.is_dir, f.size_bytes), ("a b.txt", false, 5));
        assert_eq!(f.original_path, work.join("a b.txt").to_string_lossy());
        let d = list.iter().find(|e| e.id == id2).unwrap();
        assert!(d.is_dir && d.size_bytes == 1000, "folder size is summed");

        // restore, parent was deleted in the meantime → recreated
        fs::remove_dir_all(&work).unwrap();
        let back = restore_in(&root, &id1).unwrap();
        assert_eq!(fs::read_to_string(&back).unwrap(), "hello");
        assert!(!root.join("info").join(format!("{id1}.trashinfo")).exists());

        // name clash on restore never overwrites
        fs::write(work.join("sub"), "i am a file now").unwrap();
        let back2 = restore_in(&root, &id2).unwrap();
        assert!(back2.ends_with("sub (restored)"), "{back2}");
        assert_eq!(fs::read_to_string(work.join("sub")).unwrap(), "i am a file now");
        assert!(Path::new(&back2).join("inner.bin").exists());

        // purge
        let id3 = trash_into(&root, &work.join("sub")).unwrap();
        purge_in(&root, &id3).unwrap();
        assert!(list_in(&root).is_empty());
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn same_name_gets_unique_ids_and_empty_works() {
        let base = tmp();
        let root = base.join("trash");
        for _ in 0..3 {
            fs::write(base.join("dup.txt"), "x").unwrap();
            trash_into(&root, &base.join("dup.txt")).unwrap();
        }
        let ids: Vec<String> = list_in(&root).into_iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"dup.txt".to_string()) && ids.contains(&"dup.txt.2".to_string()) && ids.contains(&"dup.txt.3".to_string()));
        assert_eq!(empty_in(&root).unwrap(), 3);
        assert!(list_in(&root).is_empty());
        assert_eq!(fs::read_dir(root.join("info")).unwrap().count(), 0);
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn refuses_trash_inside_trash_and_traversal() {
        let base = tmp();
        let root = base.join("trash");
        fs::create_dir_all(root.join("files")).unwrap();
        fs::write(root.join("files/x"), "1").unwrap();
        assert!(trash_into(&root, &root.join("files/x")).is_err());
        assert!(restore_in(&root, "../../etc/passwd").is_err());
        assert!(purge_in(&root, "../x").is_err());
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn item_without_info_is_listed_and_deletable() {
        let base = tmp();
        let root = base.join("trash");
        fs::create_dir_all(root.join("files")).unwrap();
        fs::write(root.join("files/orphan.txt"), "zz").unwrap();
        let l = list_in(&root);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].name.as_str(), l[0].size_bytes), ("orphan.txt", 2));
        purge_in(&root, "orphan.txt").unwrap();
        assert!(list_in(&root).is_empty());
        fs::remove_dir_all(&base).ok();
    }
}
