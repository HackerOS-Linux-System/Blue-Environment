use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::resolve_path;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    Replace,
    Skip,
    KeepBoth,
    Merge,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Decision {
    pub policy: Policy,
    /// Dla `Merge`: co robić z konfliktami PLIKÓW wewnątrz scalanych folderów
    /// (Replace / Skip / KeepBoth). Domyślnie Skip.
    #[serde(default)]
    pub inner: Option<Policy>,
}

#[derive(Debug, Serialize)]
pub struct Conflict {
    pub src: String,
    pub name: String,
    pub dest: String,
    pub src_is_dir: bool,
    pub dest_is_dir: bool,
    pub src_size: u64,
    pub dest_size: u64,
    pub src_modified: Option<i64>,
    pub dest_modified: Option<i64>,
    /// Wklejenie kopii do folderu, w którym już leży źródło (cel == źródło).
    pub same_location: bool,
}

#[derive(Debug, Serialize, Default)]
pub struct TransferSummary {
    pub done: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
    /// Użytkownik przerwał operację (przycisk Anuluj w oknie postępu).
    pub cancelled: bool,
}

// ───────────────────────── pomocnicze ─────────────────────────

fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

fn mtime(p: &Path) -> Option<i64> {
    fs::metadata(p).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

/// Rozmiar pliku albo suma rozmiarów w folderze (z limitem, by nie mielić dysku).
fn size_of(p: &Path) -> u64 {
    fn walk(p: &Path, budget: &mut u32) -> u64 {
        let Ok(m) = fs::symlink_metadata(p) else { return 0 };
        if !m.is_dir() {
            return m.len();
        }
        let mut total = 0;
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                if *budget == 0 {
                    break;
                }
                *budget -= 1;
                total += walk(&e.path(), budget);
            }
        }
        total
    }
    walk(p, &mut 5000)
}

pub fn valid_name(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty() && n.len() <= 255 && n != "." && n != ".." && !n.contains('/') && !n.contains('\0')
}

/// `plik.txt` → `plik (2).txt`, `arch.tar.gz` → `arch (2).tar.gz`, `folder` → `folder (2)`.
pub fn unique_name(dir: &Path, name: &str) -> String {
    if !exists(&dir.join(name)) {
        return name.to_string();
    }
    let (stem, ext) = split_name(name);
    for i in 2..10_000 {
        let cand = format!("{stem} ({i}){ext}");
        if !exists(&dir.join(&cand)) {
            return cand;
        }
    }
    format!("{stem} (kopia){ext}")
}

fn split_name(name: &str) -> (String, String) {
    for double in [".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst"] {
        if let Some(stem) = name.strip_suffix(double) {
            if !stem.is_empty() {
                return (stem.to_string(), double.to_string());
            }
        }
    }
    match name.rfind('.') {
        Some(i) if i > 0 => (name[..i].to_string(), name[i..].to_string()),
        _ => (name.to_string(), String::new()),
    }
}

fn remove_any(p: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(p),
        Ok(_) => fs::remove_file(p),
        Err(_) => Ok(()),
    }
}

// ───────────────────────── postęp, pauza, anulowanie ─────────────────────────

const CANCELLED: &str = "__cancelled__";
const CHUNK: usize = 256 * 1024;

/// Sterowanie zadaniem z zewnątrz (okno postępu: Wstrzymaj / Anuluj).
#[derive(Default)]
pub struct JobCtl {
    pub cancel: AtomicBool,
    pub paused: AtomicBool,
}

static JOBS: Mutex<Option<HashMap<String, Arc<JobCtl>>>> = Mutex::new(None);

fn register_job(id: &str) -> Arc<JobCtl> {
    let ctl = Arc::new(JobCtl::default());
    JOBS.lock().unwrap().get_or_insert_with(HashMap::new).insert(id.to_string(), ctl.clone());
    ctl
}
fn unregister_job(id: &str) {
    if let Some(m) = JOBS.lock().unwrap().as_mut() {
        m.remove(id);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub job_id: String,
    pub mode: String,
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub done_files: u64,
    pub total_files: u64,
    pub current: String,
    pub bytes_per_sec: f64,
    pub eta_secs: Option<u64>,
    pub paused: bool,
}

/// Kontekst jednej operacji: liczniki, sterowanie i odbiornik zdarzeń.
pub struct Ctx {
    id: String,
    mode: &'static str,
    ctl: Arc<JobCtl>,
    emit: Box<dyn Fn(&ProgressEvent) + Send + Sync>,
    total_bytes: u64,
    total_files: u64,
    done_bytes: u64,
    done_files: u64,
    current: String,
    started: Instant,
    last_emit: Instant,
    /// Minimalny odstęp między zdarzeniami (domyślnie 100 ms; testy ustawiają 0).
    pub min_interval: Duration,
    /// Okno do wygładzenia prędkości: (czas, bajty).
    window: Vec<(Instant, u64)>,
}

impl Ctx {
    pub fn new(id: &str, mv: bool, ctl: Arc<JobCtl>, emit: Box<dyn Fn(&ProgressEvent) + Send + Sync>) -> Self {
        let now = Instant::now();
        Ctx {
            id: id.to_string(), mode: if mv { "move" } else { "copy" }, ctl, emit,
            total_bytes: 0, total_files: 0, done_bytes: 0, done_files: 0,
            current: String::new(), started: now, last_emit: now - Duration::from_secs(1),
            min_interval: Duration::from_millis(100), window: Vec::new(),
        }
    }
    pub fn noop() -> Self {
        Ctx::new("", false, Arc::new(JobCtl::default()), Box::new(|_| {}))
    }

    fn speed(&mut self) -> f64 {
        let now = Instant::now();
        self.window.push((now, self.done_bytes));
        self.window.retain(|(t, _)| now.duration_since(*t) <= Duration::from_secs(3));
        match (self.window.first(), self.window.last()) {
            (Some((t0, b0)), Some((t1, b1))) if t1 > t0 => (*b1 - *b0) as f64 / t1.duration_since(*t0).as_secs_f64(),
            _ => 0.0,
        }
    }

    fn event(&mut self) -> ProgressEvent {
        let bps = self.speed();
        let remaining = self.total_bytes.saturating_sub(self.done_bytes);
        ProgressEvent {
            job_id: self.id.clone(), mode: self.mode.to_string(),
            done_bytes: self.done_bytes, total_bytes: self.total_bytes,
            done_files: self.done_files, total_files: self.total_files,
            current: self.current.clone(), bytes_per_sec: bps,
            eta_secs: (bps > 1.0).then(|| (remaining as f64 / bps).ceil() as u64),
            paused: self.ctl.paused.load(Ordering::SeqCst),
        }
    }

    /// Wywoływane często: obsługuje pauzę i anulowanie, a zdarzenia wysyła
    /// najwyżej co 100 ms (`force` = zawsze).
    fn tick(&mut self, force: bool) -> Result<(), String> {
        while self.ctl.paused.load(Ordering::SeqCst) && !self.ctl.cancel.load(Ordering::SeqCst) {
            let ev = self.event();
            (self.emit)(&ev);
            std::thread::sleep(Duration::from_millis(150));
        }
        if self.ctl.cancel.load(Ordering::SeqCst) {
            return Err(CANCELLED.into());
        }
        if force || self.last_emit.elapsed() >= self.min_interval {
            self.last_emit = Instant::now();
            let ev = self.event();
            (self.emit)(&ev);
        }
        Ok(())
    }
}

/// (bajty, liczba plików) w całym drzewie — pełne przejście, do paska postępu.
pub fn measure(p: &Path) -> (u64, u64) {
    let Ok(m) = fs::symlink_metadata(p) else { return (0, 0) };
    if m.is_dir() {
        let (mut b, mut f) = (0, 0);
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                let (cb, cf) = measure(&e.path());
                b += cb;
                f += cf;
            }
        }
        (b, f.max(0))
    } else {
        (m.len(), 1)
    }
}

/// Kopia pliku w kawałkach — z postępem, pauzą i anulowaniem. Przy przerwaniu
/// albo błędzie niedokończony plik docelowy jest usuwany.
fn copy_file_ctx(src: &Path, dst: &Path, ctx: &mut Ctx) -> Result<(), String> {
    ctx.current = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut input = fs::File::open(src).map_err(|e| e.to_string())?;
    let perms = input.metadata().map(|m| m.permissions()).ok();
    let mut out = fs::File::create(dst).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; CHUNK];
    let result = (|| -> Result<(), String> {
        loop {
            ctx.tick(false)?;
            let n = input.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            ctx.done_bytes += n as u64;
        }
        out.flush().map_err(|e| e.to_string())
    })();
    drop(out);
    if let Err(e) = result {
        let _ = fs::remove_file(dst);
        return Err(e);
    }
    if let Some(p) = perms {
        let _ = fs::set_permissions(dst, p);
    }
    ctx.done_files += 1;
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path, ctx: &mut Ctx) -> Result<(), String> {
    let m = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
    if m.file_type().is_symlink() {
        let target = fs::read_link(src).map_err(|e| e.to_string())?;
        std::os::unix::fs::symlink(target, dst).map_err(|e| e.to_string())?;
        ctx.done_files += 1;
        return ctx.tick(false);
    }
    if m.is_dir() {
        fs::create_dir_all(dst).map_err(|e| e.to_string())?;
        for e in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
            copy_tree(&e.path(), &dst.join(e.file_name()), ctx)?;
        }
        if let Ok(p) = fs::metadata(src).map(|m| m.permissions()) {
            let _ = fs::set_permissions(dst, p);
        }
        return Ok(());
    }
    copy_file_ctx(src, dst, ctx)
}

/// Przeniesienie: najpierw `rename` (natychmiast, ten sam system plików),
/// przy EXDEV kopia z postępem + usunięcie źródła.
fn move_plain(src: &Path, dst: &Path, ctx: &mut Ctx) -> Result<(), String> {
    ctx.tick(false)?;
    match fs::rename(src, dst) {
        Ok(()) => {
            let (b, f) = measure(dst);
            ctx.done_bytes += b;
            ctx.done_files += f;
            ctx.current = dst.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            ctx.tick(false)
        }
        Err(e) if e.raw_os_error() == Some(18) => {
            copy_tree(src, dst, ctx)?;
            remove_any(src).map_err(|e| e.to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn place(src: &Path, dst: &Path, mv: bool, ctx: &mut Ctx) -> Result<(), String> {
    if mv { move_plain(src, dst, ctx) } else { copy_tree(src, dst, ctx) }
}

/// Zastąpienie celu. Plik→plik: kopia do pliku tymczasowego + atomowy rename
/// (przerwanie w połowie nie niszczy starego pliku). Reszta: usuń i połóż.
fn replace(src: &Path, dst: &Path, mv: bool, ctx: &mut Ctx) -> Result<(), String> {
    let src_is_dir = fs::symlink_metadata(src).map(|m| m.is_dir()).unwrap_or(false);
    let dst_is_dir = fs::symlink_metadata(dst).map(|m| m.is_dir()).unwrap_or(false);
    if !src_is_dir && !dst_is_dir {
        let tmp = dst.with_file_name(format!(".{}.blue-tmp-{}", dst.file_name().and_then(|n| n.to_str()).unwrap_or("x"), std::process::id()));
        copy_file_ctx(src, &tmp, ctx)?;
        fs::rename(&tmp, dst).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            e.to_string()
        })?;
        if mv {
            remove_any(src).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    remove_any(dst).map_err(|e| e.to_string())?;
    place(src, dst, mv, ctx)
}

fn merge_dirs(src: &Path, dst: &Path, mv: bool, inner: Policy, sum: &mut TransferSummary, ctx: &mut Ctx) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let s = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let d = dst.join(&name);
        let s_dir = fs::symlink_metadata(&s).map(|m| m.is_dir()).unwrap_or(false);
        let d_dir = fs::symlink_metadata(&d).map(|m| m.is_dir()).unwrap_or(false);
        let res = if !exists(&d) {
            place(&s, &d, mv, ctx).map(|_| sum.done += 1)
        } else if s_dir && d_dir {
            merge_dirs(&s, &d, mv, inner, sum, ctx)
        } else {
            match inner {
                Policy::Replace => replace(&s, &d, mv, ctx).map(|_| sum.done += 1),
                Policy::KeepBoth => {
                    let nd = dst.join(unique_name(dst, &name));
                    place(&s, &nd, mv, ctx).map(|_| sum.done += 1)
                }
                _ => {
                    sum.skipped += 1;
                    Ok(())
                }
            }
        };
        match res {
            Err(e) if e == CANCELLED => return Err(e),
            Err(e) => sum.errors.push(format!("{}: {e}", s.display())),
            Ok(()) => {}
        }
    }
    if mv {
        let _ = fs::remove_dir(src); // tylko jeśli pusty (pominięte pliki zostają)
    }
    Ok(())
}

// ───────────────────────── komendy ─────────────────────────

#[tauri::command(async)]
pub fn fm_check_conflicts(sources: Vec<String>, dest_dir: String, mode: String) -> Result<Vec<Conflict>, String> {
    let dest_dir = resolve_path(&dest_dir);
    let mv = mode == "move";
    let mut out = Vec::new();
    for s in sources {
        let src = resolve_path(&s);
        let Some(name) = src.file_name().map(|n| n.to_string_lossy().to_string()) else { continue };
        let dest = dest_dir.join(&name);
        if !exists(&dest) {
            continue;
        }
        let same = src == dest;
        if same && mv {
            continue; // przeniesienie na to samo miejsce = nic do roboty
        }
        out.push(Conflict {
            src: s,
            name,
            dest: dest.to_string_lossy().to_string(),
            src_is_dir: src.is_dir(),
            dest_is_dir: dest.is_dir(),
            src_size: size_of(&src),
            dest_size: size_of(&dest),
            src_modified: mtime(&src),
            dest_modified: mtime(&dest),
            same_location: same,
        });
    }
    Ok(out)
}

/// Wersja bez postępu (testy, wywołania wewnętrzne).
pub fn transfer_impl(sources: &[String], dest_dir: &Path, mv: bool, decisions: &HashMap<String, Decision>) -> TransferSummary {
    transfer_with(sources, dest_dir, mv, decisions, &mut Ctx::noop())
}

#[tauri::command(async)]
pub fn fm_transfer(
    app: AppHandle,
    sources: Vec<String>,
    dest_dir: String,
    mode: String,
    decisions: HashMap<String, Decision>,
    job_id: Option<String>,
) -> TransferSummary {
    let id = job_id.unwrap_or_else(|| format!("job-{}", std::process::id()));
    let ctl = register_job(&id);
    let mv = mode == "move";
    let app2 = app.clone();
    let mut ctx = Ctx::new(&id, mv, ctl, Box::new(move |ev| { let _ = app2.emit("fm:progress", ev); }));
    let summary = transfer_with(&sources, &resolve_path(&dest_dir), mv, &decisions, &mut ctx);
    unregister_job(&id);
    summary
}

/// Wstrzymanie / wznowienie / anulowanie trwającego zadania.
#[tauri::command]
pub fn fm_job_control(job_id: String, action: String) -> Result<(), String> {
    let jobs = JOBS.lock().unwrap();
    let ctl = jobs.as_ref().and_then(|m| m.get(&job_id)).ok_or("Zadanie już się zakończyło")?;
    match action.as_str() {
        "pause" => ctl.paused.store(true, Ordering::SeqCst),
        "resume" => ctl.paused.store(false, Ordering::SeqCst),
        "cancel" => {
            ctl.cancel.store(true, Ordering::SeqCst);
            ctl.paused.store(false, Ordering::SeqCst);
        }
        other => return Err(format!("nieznana akcja: {other}")),
    }
    Ok(())
}

pub fn transfer_with(sources: &[String], dest_dir: &Path, mv: bool, decisions: &HashMap<String, Decision>, ctx: &mut Ctx) -> TransferSummary {
    let mut sum = TransferSummary::default();
    if !dest_dir.is_dir() {
        sum.errors.push(format!("Folder docelowy nie istnieje: {}", dest_dir.display()));
        return sum;
    }
    for s in sources {
        let (b, f) = measure(&resolve_path(s));
        ctx.total_bytes += b;
        ctx.total_files += f;
    }
    ctx.started = Instant::now();
    let _ = ctx.tick(true);
    for s in sources {
        let src = resolve_path(s);
        let Some(name) = src.file_name().map(|n| n.to_string_lossy().to_string()) else { continue };
        let dest = dest_dir.join(&name);
        let is_dir = fs::symlink_metadata(&src).map(|m| m.is_dir()).unwrap_or(false);
        if is_dir && dest_dir.starts_with(&src) {
            sum.errors.push(format!("{name}: nie można {} folderu do jego własnego wnętrza", if mv { "przenieść" } else { "skopiować" }));
            continue;
        }
        if !exists(&src) {
            sum.errors.push(format!("{name}: źródło już nie istnieje"));
            continue;
        }
        if src == dest && mv {
            sum.skipped += 1;
            continue;
        }
        let res: Result<bool, String> = if !exists(&dest) {
            place(&src, &dest, mv, ctx).map(|_| true)
        } else {
            match decisions.get(s).map(|d| d.policy).unwrap_or(Policy::Skip) {
                Policy::Skip => Ok(false),
                Policy::KeepBoth => {
                    let nd = dest_dir.join(unique_name(dest_dir, &name));
                    place(&src, &nd, mv, ctx).map(|_| true)
                }
                Policy::Replace if src == dest => Err("źródło i cel to ten sam element".into()),
                Policy::Replace => replace(&src, &dest, mv, ctx).map(|_| true),
                Policy::Merge => {
                    if !is_dir || !dest.is_dir() {
                        Err("scalić można tylko folder z folderem".into())
                    } else if src == dest {
                        Err("nie można scalić folderu z samym sobą".into())
                    } else {
                        let inner = decisions.get(s).and_then(|d| d.inner).unwrap_or(Policy::Skip);
                        merge_dirs(&src, &dest, mv, inner, &mut sum, ctx).map(|_| false)
                    }
                }
            }
        };
        match res {
            Ok(true) => sum.done += 1,
            Ok(false) => {
                if !matches!(decisions.get(s).map(|d| d.policy), Some(Policy::Merge)) {
                    sum.skipped += 1;
                }
            }
            Err(e) if e == CANCELLED => {
                sum.cancelled = true;
                break;
            }
            Err(e) => sum.errors.push(format!("{name}: {e}")),
        }
    }
    // Końcowe zdarzenie z pełnym stanem (okno postępu zawsze widzi 100 %).
    let _ = ctx.tick(true);
    sum
}


#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Folder,
}

#[tauri::command(async)]
pub fn fm_exists(dir: String, name: String) -> Option<EntryKind> {
    if !valid_name(&name) {
        return None;
    }
    let p = resolve_path(&dir).join(name.trim());
    let m = fs::symlink_metadata(&p).ok()?;
    Some(if m.is_dir() { EntryKind::Folder } else { EntryKind::File })
}

/// Tworzy plik/folder. Gdy nazwa jest zajęta, a nie podano `policy`, zwraca
/// błąd `EXISTS:file|folder` — frontend pyta wtedy użytkownika.
pub fn create_impl(dir: &Path, name: &str, kind: &str, content: &str, policy: Option<Policy>) -> Result<String, String> {
    if !valid_name(name) {
        return Err("Nieprawidłowa nazwa".into());
    }
    let name = name.trim();
    let target = dir.join(name);
    let mut final_path = target.clone();
    if let Ok(m) = fs::symlink_metadata(&target) {
        let existing = if m.is_dir() { "folder" } else { "file" };
        match policy {
            None => return Err(format!("EXISTS:{existing}")),
            Some(Policy::Skip) => return Err("SKIPPED".into()),
            Some(Policy::KeepBoth) => final_path = dir.join(unique_name(dir, name)),
            Some(Policy::Merge) | Some(Policy::Replace) => {
                if m.is_dir() && kind == "folder" {
                    return Ok(target.to_string_lossy().to_string()); // istniejący folder zostaje
                }
                if m.is_dir() || kind == "folder" {
                    return Err("Nie można zastąpić folderu plikiem ani odwrotnie".into());
                }
            }
        }
    }
    if kind == "folder" {
        fs::create_dir(&final_path).map_err(|e| e.to_string())?;
    } else {
        fs::write(&final_path, content).map_err(|e| e.to_string())?;
    }
    Ok(final_path.to_string_lossy().to_string())
}

#[tauri::command(async)]
pub fn fm_create(dir: String, name: String, kind: String, content: Option<String>, policy: Option<Policy>) -> Result<String, String> {
    create_impl(&resolve_path(&dir), &name, &kind, &content.unwrap_or_default(), policy)
}

pub fn rename_impl(path: &Path, new_name: &str, replace_existing: bool) -> Result<String, String> {
    if !valid_name(new_name) {
        return Err("Nieprawidłowa nazwa".into());
    }
    let parent = path.parent().ok_or("brak folderu nadrzędnego")?;
    let target: PathBuf = parent.join(new_name.trim());
    if target == path {
        return Ok(target.to_string_lossy().to_string());
    }
    if let Ok(m) = fs::symlink_metadata(&target) {
        let existing = if m.is_dir() { "folder" } else { "file" };
        if !replace_existing {
            return Err(format!("EXISTS:{existing}"));
        }
        let src_dir = path.is_dir();
        if src_dir != m.is_dir() {
            return Err("Nie można zastąpić folderu plikiem ani odwrotnie".into());
        }
        if m.is_dir() {
            fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
        }
        // plik: rename atomowo nadpisuje
    }
    fs::rename(path, &target).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command(async)]
pub fn fm_rename(path: String, new_name: String, replace_existing: bool) -> Result<String, String> {
    rename_impl(&resolve_path(&path), &new_name, replace_existing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("blue-fm-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn w(p: &Path, s: &str) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, s).unwrap();
    }
    fn dec(policy: Policy, inner: Option<Policy>) -> Decision { Decision { policy, inner } }
    fn s(p: &Path) -> String { p.to_string_lossy().to_string() }

    #[test]
    fn unique_names() {
        let d = tmp("uniq");
        w(&d.join("a.txt"), "x");
        w(&d.join("a (2).txt"), "x");
        w(&d.join("z.tar.gz"), "x");
        fs::create_dir(d.join("dir")).unwrap();
        assert_eq!(unique_name(&d, "a.txt"), "a (3).txt");
        assert_eq!(unique_name(&d, "z.tar.gz"), "z (2).tar.gz");
        assert_eq!(unique_name(&d, "dir"), "dir (2)");
        assert_eq!(unique_name(&d, "nowy.txt"), "nowy.txt");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn detects_conflicts_and_same_location() {
        let d = tmp("chk");
        w(&d.join("src/a.txt"), "new");
        w(&d.join("dst/a.txt"), "old!");
        w(&d.join("dst/free.txt"), "x");
        w(&d.join("src/free.txt"), "x2");
        fs::remove_file(d.join("dst/free.txt")).unwrap();
        let c = fm_check_conflicts(vec![s(&d.join("src/a.txt")), s(&d.join("src/free.txt"))], s(&d.join("dst")), "copy".into()).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].src_size, c[0].dest_size), (3, 4));
        // wklejenie kopii do tego samego folderu
        let c = fm_check_conflicts(vec![s(&d.join("src/a.txt"))], s(&d.join("src")), "copy".into()).unwrap();
        assert!(c[0].same_location);
        // przeniesienie na to samo miejsce nie jest konfliktem
        assert!(fm_check_conflicts(vec![s(&d.join("src/a.txt"))], s(&d.join("src")), "move".into()).unwrap().is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn no_decision_never_overwrites() {
        let d = tmp("safe");
        w(&d.join("src/a.txt"), "NEW");
        w(&d.join("dst/a.txt"), "OLD");
        let r = transfer_impl(&[s(&d.join("src/a.txt"))], &d.join("dst"), false, &HashMap::new());
        assert_eq!((r.done, r.skipped), (0, 1));
        assert_eq!(fs::read_to_string(d.join("dst/a.txt")).unwrap(), "OLD");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn replace_keepboth_skip_for_files() {
        let d = tmp("pol");
        w(&d.join("src/a.txt"), "NEW");
        w(&d.join("dst/a.txt"), "OLD");
        let src = s(&d.join("src/a.txt"));
        let mut m = HashMap::new();
        m.insert(src.clone(), dec(Policy::KeepBoth, None));
        let r = transfer_impl(&[src.clone()], &d.join("dst"), false, &m);
        assert_eq!(r.done, 1);
        assert_eq!(fs::read_to_string(d.join("dst/a (2).txt")).unwrap(), "NEW");
        assert_eq!(fs::read_to_string(d.join("dst/a.txt")).unwrap(), "OLD");
        m.insert(src.clone(), dec(Policy::Replace, None));
        let r = transfer_impl(&[src.clone()], &d.join("dst"), true, &m); // przenieś + zastąp
        assert_eq!(r.done, 1);
        assert_eq!(fs::read_to_string(d.join("dst/a.txt")).unwrap(), "NEW");
        assert!(!d.join("src/a.txt").exists());
        // brak śmieci po pliku tymczasowym
        assert!(fs::read_dir(d.join("dst")).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().contains("blue-tmp")));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn merge_folders_with_inner_policy() {
        let d = tmp("merge");
        w(&d.join("src/proj/a.txt"), "src-a");
        w(&d.join("src/proj/new.txt"), "src-new");
        w(&d.join("src/proj/sub/x.txt"), "src-x");
        w(&d.join("dst/proj/a.txt"), "dst-a");
        w(&d.join("dst/proj/keep.txt"), "dst-keep");
        w(&d.join("dst/proj/sub/y.txt"), "dst-y");
        let src = s(&d.join("src/proj"));
        let mut m = HashMap::new();
        m.insert(src.clone(), dec(Policy::Merge, Some(Policy::Skip)));
        let r = transfer_impl(&[src.clone()], &d.join("dst"), false, &m);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let p = d.join("dst/proj");
        assert_eq!(fs::read_to_string(p.join("a.txt")).unwrap(), "dst-a");      // Skip zachował stary
        assert_eq!(fs::read_to_string(p.join("new.txt")).unwrap(), "src-new");  // nowy dołożony
        assert_eq!(fs::read_to_string(p.join("keep.txt")).unwrap(), "dst-keep");
        assert!(p.join("sub/x.txt").exists() && p.join("sub/y.txt").exists());   // rekurencyjne scalenie
        assert_eq!(r.skipped, 1);
        // przenoszenie ze scaleniem i Replace: źródło znika, plik nadpisany
        m.insert(src.clone(), dec(Policy::Merge, Some(Policy::Replace)));
        let r = transfer_impl(&[src.clone()], &d.join("dst"), true, &m);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(fs::read_to_string(p.join("a.txt")).unwrap(), "src-a");
        assert!(!d.join("src/proj").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn merge_rejects_file_and_self_nesting() {
        let d = tmp("bad");
        w(&d.join("src/a"), "file");
        fs::create_dir_all(d.join("dst/a")).unwrap();
        let src = s(&d.join("src/a"));
        let mut m = HashMap::new();
        m.insert(src.clone(), dec(Policy::Merge, None));
        let r = transfer_impl(&[src], &d.join("dst"), false, &m);
        assert_eq!(r.errors.len(), 1);
        // folder do własnego wnętrza
        fs::create_dir_all(d.join("f/sub")).unwrap();
        let r = transfer_impl(&[s(&d.join("f"))], &d.join("f/sub"), false, &HashMap::new());
        assert_eq!(r.errors.len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn create_requires_decision_when_name_taken() {
        let d = tmp("create");
        assert!(create_impl(&d, "x.txt", "file", "1", None).is_ok());
        assert_eq!(create_impl(&d, "x.txt", "file", "2", None).unwrap_err(), "EXISTS:file");
        assert_eq!(fs::read_to_string(d.join("x.txt")).unwrap(), "1"); // nic nie nadpisane
        let p = create_impl(&d, "x.txt", "file", "3", Some(Policy::KeepBoth)).unwrap();
        assert!(p.ends_with("x (2).txt"));
        create_impl(&d, "x.txt", "file", "4", Some(Policy::Replace)).unwrap();
        assert_eq!(fs::read_to_string(d.join("x.txt")).unwrap(), "4");
        create_impl(&d, "dir", "folder", "", None).unwrap();
        assert_eq!(create_impl(&d, "dir", "folder", "", None).unwrap_err(), "EXISTS:folder");
        assert!(create_impl(&d, "dir", "file", "", Some(Policy::Replace)).is_err()); // folder≠plik
        assert!(create_impl(&d, "a/b", "file", "", None).is_err());
        assert!(create_impl(&d, "..", "folder", "", None).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rename_asks_before_replacing() {
        let d = tmp("ren");
        w(&d.join("a.txt"), "A");
        w(&d.join("b.txt"), "B");
        assert_eq!(rename_impl(&d.join("a.txt"), "b.txt", false).unwrap_err(), "EXISTS:file");
        assert_eq!(fs::read_to_string(d.join("b.txt")).unwrap(), "B");
        rename_impl(&d.join("a.txt"), "b.txt", true).unwrap();
        assert_eq!(fs::read_to_string(d.join("b.txt")).unwrap(), "A");
        assert!(rename_impl(&d.join("b.txt"), "b.txt", false).is_ok()); // bez zmiany
        let _ = fs::remove_dir_all(&d);
    }
    #[test]
    fn progress_events_report_bytes_and_files() {
        let d = tmp("prog");
        fs::create_dir_all(d.join("dst")).unwrap();
        fs::create_dir_all(d.join("src")).unwrap();
        fs::write(d.join("src/big.bin"), vec![7u8; 3_000_000]).unwrap();
        w(&d.join("src/small.txt"), "x");
        let events = Arc::new(Mutex::new(Vec::<ProgressEvent>::new()));
        let e2 = events.clone();
        let mut ctx = Ctx::new("t1", false, Arc::new(JobCtl::default()), Box::new(move |e| e2.lock().unwrap().push(e.clone())));
        ctx.min_interval = Duration::ZERO;
        let r = transfer_with(&[s(&d.join("src/big.bin")), s(&d.join("src/small.txt"))], &d.join("dst"), false, &HashMap::new(), &mut ctx);
        assert_eq!(r.done, 2);
        let ev = events.lock().unwrap();
        let last = ev.last().unwrap();
        assert_eq!((last.total_bytes, last.total_files), (3_000_001, 2));
        assert_eq!((last.done_bytes, last.done_files), (3_000_001, 2));
        assert!(ev.len() > 3 && ev.iter().all(|e| e.job_id == "t1"));
        assert!(ev.windows(2).all(|p| p[1].done_bytes >= p[0].done_bytes)); // monotonicznie
        assert_eq!(fs::read(d.join("dst/big.bin")).unwrap().len(), 3_000_000);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn cancel_mid_copy_removes_partial_file() {
        let d = tmp("cancel");
        fs::create_dir_all(d.join("dst")).unwrap();
        fs::create_dir_all(d.join("src")).unwrap();
        fs::write(d.join("src/big.bin"), vec![1u8; 5_000_000]).unwrap();
        let ctl = Arc::new(JobCtl::default());
        let c2 = ctl.clone();
        let mut ctx = Ctx::new("t2", false, ctl, Box::new(move |e| {
            if e.done_bytes >= 1_000_000 { c2.cancel.store(true, Ordering::SeqCst); }
        }));
        ctx.min_interval = Duration::ZERO;
        let r = transfer_with(&[s(&d.join("src/big.bin"))], &d.join("dst"), false, &HashMap::new(), &mut ctx);
        assert!(r.cancelled);
        assert_eq!(r.done, 0);
        assert!(!d.join("dst/big.bin").exists(), "niedokończony plik musi zniknąć");
        assert!(d.join("src/big.bin").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn cancel_before_start_and_pause_resume() {
        let d = tmp("pause");
        fs::create_dir_all(d.join("dst")).unwrap();
        w(&d.join("src/a.txt"), "data");
        let ctl = Arc::new(JobCtl::default());
        ctl.cancel.store(true, Ordering::SeqCst);
        let mut ctx = Ctx::new("t3", false, ctl, Box::new(|_| {}));
        let r = transfer_with(&[s(&d.join("src/a.txt"))], &d.join("dst"), false, &HashMap::new(), &mut ctx);
        assert!(r.cancelled && !d.join("dst/a.txt").exists());

        // wstrzymane zadanie czeka, a po wznowieniu kończy pracę
        let ctl = Arc::new(JobCtl::default());
        ctl.paused.store(true, Ordering::SeqCst);
        let c2 = ctl.clone();
        let h = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            c2.paused.store(false, Ordering::SeqCst);
        });
        let started = Instant::now();
        let mut ctx = Ctx::new("t4", false, ctl, Box::new(|_| {}));
        let r = transfer_with(&[s(&d.join("src/a.txt"))], &d.join("dst"), false, &HashMap::new(), &mut ctx);
        h.join().unwrap();
        assert_eq!(r.done, 1);
        assert!(started.elapsed() >= Duration::from_millis(250));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn move_same_filesystem_counts_progress_without_copying() {
        let d = tmp("mvprog");
        fs::create_dir_all(d.join("dst")).unwrap();
        w(&d.join("src/dir/a.txt"), "12345");
        w(&d.join("src/dir/b.txt"), "678");
        let mut ctx = Ctx::noop();
        let r = transfer_with(&[s(&d.join("src/dir"))], &d.join("dst"), true, &HashMap::new(), &mut ctx);
        assert_eq!(r.done, 1);
        assert_eq!((ctx.done_bytes, ctx.done_files), (8, 2));
        assert!(!d.join("src/dir").exists() && d.join("dst/dir/a.txt").exists());
        let _ = fs::remove_dir_all(&d);
    }

}
