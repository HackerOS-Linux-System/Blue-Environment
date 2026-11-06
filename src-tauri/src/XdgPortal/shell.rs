use crate::xdg_portal::{self, ChooserMode, ChooserRequest, PortalUi};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath, MessageDialogButtons, MessageDialogKind};

static CONN: OnceLock<zbus::Connection> = OnceLock::new();

struct ShellUi {
    app: AppHandle,
}

fn into_paths(v: Vec<FilePath>) -> Vec<PathBuf> {
    v.into_iter().filter_map(|p| p.into_path().ok()).collect()
}

impl PortalUi for ShellUi {
    fn choose(&self, req: &ChooserRequest) -> Option<Vec<PathBuf>> {
        let mut d = self.app.dialog().file();
        if !req.title.is_empty() {
            d = d.set_title(&req.title);
        }
        if let Some(dir) = &req.current_folder {
            d = d.set_directory(dir);
        }
        if let Some(name) = &req.current_name {
            d = d.set_file_name(name);
        }
        for f in &req.filters {
            if f.extensions.is_empty() {
                continue; // "any file" needs no filter
            }
            let exts: Vec<&str> = f.extensions.iter().map(String::as_str).collect();
            d = d.add_filter(&f.name, &exts);
        }
        match req.mode {
            ChooserMode::Open if req.directory => d.blocking_pick_folder().map(|p| into_paths(vec![p])),
            ChooserMode::Open if req.multiple => d.blocking_pick_files().map(into_paths),
            ChooserMode::Open => d.blocking_pick_file().map(|p| into_paths(vec![p])),
            ChooserMode::Save => d.blocking_save_file().map(|p| into_paths(vec![p])),
            ChooserMode::SaveFiles => d.blocking_pick_folder().map(|p| into_paths(vec![p])),
        }
        .filter(|v| !v.is_empty())
    }

    fn confirm(&self, title: &str, message: &str) -> bool {
        self.app
            .dialog()
            .message(message)
            .title(title)
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom("Allow".to_string(), "Deny".to_string()))
            .blocking_show()
    }
}

/// Starts serving the portal backend on the session bus.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        match xdg_portal::run(Arc::new(ShellUi { app })).await {
            Ok(Some(conn)) => {
                let _ = CONN.set(conn);
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("portal backend failed to start: {e}"),
        }
    });
}

/// Everything the Diagnostics page needs to say why screen sharing / file
/// pickers do (not) work in Flatpak apps and browsers.
#[tauri::command]
pub async fn portal_status() -> xdg_portal::PortalStatus {
    xdg_portal::status().await
}
