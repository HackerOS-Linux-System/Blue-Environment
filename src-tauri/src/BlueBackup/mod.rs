pub mod engine;

use engine::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

static CANCEL: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);

pub use engine::run_cli;

#[tauri::command(async)]
pub fn backup_get_config() -> BackupConfig {
    load_config()
}

/// Validates, saves and (re)installs the schedule.
#[tauri::command(async)]
pub fn backup_set_config(config: BackupConfig) -> Result<(), String> {
    validate(&config)?;
    save_config(&config)?;
    apply_schedule(&config.schedule)
}

#[tauri::command(async)]
pub fn backup_list() -> Vec<Snapshot> {
    list_snapshots(&load_config())
}

#[tauri::command(async)]
pub fn backup_is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

#[tauri::command]
pub async fn backup_run(app: AppHandle) -> Result<Snapshot, String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("A backup is already running".into());
    }
    CANCEL.store(false, Ordering::SeqCst);
    let res = tokio::task::spawn_blocking(move || {
        let cfg = load_config();
        run_backup(&cfg, &CANCEL, &mut |p| {
            let _ = app.emit("blue-backup://progress", &p);
        })
    })
    .await
    .map_err(|e| e.to_string());
    RUNNING.store(false, Ordering::SeqCst);
    res?
}

#[tauri::command(async)]
pub fn backup_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

#[tauri::command(async)]
pub fn backup_browse(snapshot: String, path: String) -> Result<Vec<Entry>, String> {
    browse(&load_config(), &snapshot, &path)
}

/// Returns the folder the items were restored into.
#[tauri::command(async)]
pub fn backup_restore(snapshot: String, path: String, in_place: bool) -> Result<String, String> {
    restore(&load_config(), &snapshot, &path, in_place)
}

#[tauri::command(async)]
pub fn backup_delete(snapshot: String) -> Result<(), String> {
    delete_snapshot(&load_config(), &snapshot)
}
