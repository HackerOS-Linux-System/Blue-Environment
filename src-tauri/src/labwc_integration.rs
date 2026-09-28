use crate::backend;

/// Whether the active labwc config already has Blue's keybinds. `None` when
/// the active backend isn't labwc (nothing to repair).
#[tauri::command]
pub async fn labwc_integration_status() -> Result<Option<backend::labwc_config::IntegrationStatus>, String> {
    if backend::active() != backend::BackendKind::Labwc {
        return Ok(None);
    }
    let cfg = backend::load_config();
    let status = tokio::task::spawn_blocking(move || backend::labwc_config::check_integration(cfg.config_dir.as_deref()))
        .await
        .map_err(|e| e.to_string())?;
    Ok(Some(status))
}

/// Merges Blue's keybinds into the user's existing labwc `rc.xml` (original
/// is backed up first) — see `labwc_config::install_keybinds_into_existing`.
#[tauri::command]
pub async fn labwc_integration_repair() -> Result<backend::labwc_config::InstallReport, String> {
    let cfg = backend::load_config();
    tokio::task::spawn_blocking(move || backend::labwc_config::install_keybinds_into_existing(cfg.config_dir.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}
