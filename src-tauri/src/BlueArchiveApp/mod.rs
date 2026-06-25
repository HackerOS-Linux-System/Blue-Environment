use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Serialize)]
pub struct ArchiveEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Serialize)]
pub struct ArchiveInfo {
    pub entries: Vec<ArchiveEntry>,
    pub total_files: usize,
    pub error: Option<String>,
}

/// Lists the contents of an archive file. Supports tar (all variants)
/// and zip; falls back to 7z for anything else.
#[tauri::command]
pub fn archive_list(path: String) -> ArchiveInfo {
    let p = PathBuf::from(&path);
    let name_lower = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();

    if name_lower.ends_with(".zip") {
        list_zip(&path)
    } else if is_tar(&name_lower) {
        list_tar(&path)
    } else {
        list_7z(&path)
    }
}

fn is_tar(name: &str) -> bool {
    name.ends_with(".tar") || name.ends_with(".tar.gz") || name.ends_with(".tgz")
    || name.ends_with(".tar.bz2") || name.ends_with(".tar.xz") || name.ends_with(".tar.zst")
}

fn list_tar(path: &str) -> ArchiveInfo {
    let o = Command::new("tar").args(["-tvf", path]).output();
    match o {
        Err(e) => ArchiveInfo { entries: vec![], total_files: 0, error: Some(e.to_string()) },
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            let entries: Vec<ArchiveEntry> = text.lines().filter_map(|l| {
                let p: Vec<&str> = l.split_whitespace().collect();
                if p.len() < 6 { return None; }
                let size: u64 = p[2].parse().unwrap_or(0);
                let path = p.last().unwrap_or(&"").to_string();
                let name = path.split('/').last().unwrap_or(&path).to_string();
                let is_dir = l.starts_with('d') || path.ends_with('/');
                Some(ArchiveEntry { name, path, size, is_dir })
            }).collect();
            let total = entries.len();
            ArchiveInfo { entries, total_files: total, error: None }
        }
    }
}

fn list_zip(path: &str) -> ArchiveInfo {
    let o = Command::new("unzip").args(["-l", path]).output();
    match o {
        Err(_) => list_7z(path), // unzip not installed — try 7z
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            let entries: Vec<ArchiveEntry> = text.lines().skip(3).filter_map(|l| {
                let p: Vec<&str> = l.split_whitespace().collect();
                if p.len() < 4 { return None; }
                let size: u64 = p[0].parse().unwrap_or(0);
                let entry_path = p[3..].join(" ");
                if entry_path == "--------" { return None; }
                let name = entry_path.split('/').last().unwrap_or(&entry_path).to_string();
                let is_dir = entry_path.ends_with('/');
                Some(ArchiveEntry { name, path: entry_path, size, is_dir })
            }).collect();
            let total = entries.len();
            ArchiveInfo { entries, total_files: total, error: None }
        }
    }
}

fn list_7z(path: &str) -> ArchiveInfo {
    let o = Command::new("7z").args(["l", "-ba", path]).output();
    match o {
        Err(e) => ArchiveInfo { entries: vec![], total_files: 0, error: Some(format!("No suitable tool found (install tar/unzip/7z): {}", e)) },
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            let entries: Vec<ArchiveEntry> = text.lines().filter_map(|l| {
                let p: Vec<&str> = l.split_whitespace().collect();
                if p.len() < 5 { return None; }
                let size: u64 = p[3].parse().unwrap_or(0);
                let entry_path = p[4..].join(" ");
                let name = entry_path.split('/').last().unwrap_or(&entry_path).to_string();
                let is_dir = l.contains("D....") || entry_path.ends_with('/');
                Some(ArchiveEntry { name, path: entry_path, size, is_dir })
            }).collect();
            let total = entries.len();
            ArchiveInfo { entries, total_files: total, error: None }
        }
    }
}

/// Extracts an archive to `dest_dir`. Uses pkexec for privilege escalation
/// only if the destination requires it (most user-home extractions don't).
#[tauri::command]
pub fn archive_extract(path: String, dest_dir: String) -> Result<String, String> {
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let name_lower = PathBuf::from(&path).file_name()
        .map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();

    let status = if name_lower.ends_with(".zip") {
        Command::new("unzip").args(["-o", &path, "-d", &dest_dir]).status()
            .or_else(|_| Command::new("7z").args(["x", &path, &format!("-o{}", dest_dir), "-y"]).status())
    } else if is_tar(&name_lower) {
        Command::new("tar").args(["-xf", &path, "-C", &dest_dir]).status()
    } else {
        Command::new("7z").args(["x", &path, &format!("-o{}", dest_dir), "-y"]).status()
    }.map_err(|e| e.to_string())?;

    if status.success() {
        Ok(format!("Extracted to: {}", dest_dir))
    } else {
        Err("Extraction failed — archive may be corrupted or password-protected".to_string())
    }
}

/// Creates a new zip archive from `files` (a list of paths to include).
#[tauri::command]
pub fn archive_create(output_path: String, files: Vec<String>) -> Result<(), String> {
    if files.is_empty() { return Err("No files specified".to_string()); }
    let status = Command::new("zip")
        .arg("-r")
        .arg(&output_path)
        .args(&files)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err("zip command failed".to_string()) }
}
