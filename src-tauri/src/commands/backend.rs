use crate::backend;

/// Which backend is configured / actually running, where its `config.hk`
/// lives and what the system offers.
#[tauri::command]
pub async fn backend_get_info() -> Result<backend::BackendInfo, String> {
    tokio::task::spawn_blocking(backend::info).await.map_err(|e| e.to_string())
}

/// Writes `[backend] compositor` to `config.hk` (comments preserved). Takes
/// effect the next time Blue Environment is started.
#[tauri::command]
pub async fn backend_set_compositor(compositor: String) -> Result<String, String> {
    let kind = backend::BackendKind::parse(&compositor)
        .ok_or_else(|| format!("unknown compositor '{compositor}' (expected hackeros-comp or labwc)"))?;
    tokio::task::spawn_blocking(move || backend::set_compositor(kind).map(|p| p.display().to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// Asks the compositor to give the shell's own window keyboard focus.
///
/// Needed when a text field opens (e.g. the Wi-Fi password prompt) while a
/// native app still holds the Wayland keyboard focus — the DOM caret blinks
/// in the field, but key events go to the other client. Returns whether any
/// focus request could be issued.
#[tauri::command]
pub async fn focus_shell(window: tauri::WebviewWindow) -> bool {
    let toolkit = window.set_focus().is_ok();
    let compositor = if backend::is_labwc() { backend::focus_shell_window() } else { false };
    toolkit || compositor
}
