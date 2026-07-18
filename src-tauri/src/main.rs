#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate libc;

mod types;
mod commands;

mod session;
mod cache;
mod apps;
mod window_tracker;
mod ai;
mod packages;
mod icon_resolver;
#[path = "CameraApp/mod.rs"]
mod camera_app;
#[path = "BlueWebApp/mod.rs"]
mod blue_web_app;
#[path = "BlueCodeApp/mod.rs"]
mod blue_code_app;
#[path = "TerminalApp/mod.rs"]
mod terminal_app;
use terminal_app::{spawn_terminal, write_to_terminal, pty_create, pty_write, pty_resize, pty_close};
#[path = "ExplolerApp/mod.rs"]
mod exploler_app;
#[path = "SettingsApp/mod.rs"]
#[allow(non_snake_case)]
mod SettingsApp;
#[path = "BlueDocs/mod.rs"]
#[allow(non_snake_case)]
mod BlueDocs;
#[path = "BlueScreenshot/mod.rs"]
mod blue_screenshot;
#[path = "SystemMonitorApp/mod.rs"]
mod system_monitor_app;
#[path = "BlueArchiveApp/mod.rs"]
mod blue_archive_app;
#[path = "MailApp/mod.rs"]
mod mail_app;
#[path = "BlueTranslateApp/mod.rs"]
mod blue_translate_app;
#[path = "BlueInstallerApp/mod.rs"]
mod blue_installer_app;

use cache::CachedApp;
use camera_app::{camera_list_devices, camera_check_available, camera_capture_frame, camera_capture_photo, camera_record_video};
use blue_web_app::{web_open_native, web_fetch_site_info};
use blue_code_app::{start_language_server, stop_language_server};
use blue_translate_app::translate_text;
use blue_installer_app::{installer_list_disks, installer_run};

// This file used to be a second, fully independent ~1260-line Tauri
// application living side by side with a similarly-sized src/lib.rs (its
// own Builder, its own command list, `[lib]` target in Cargo.toml wired for
// mobile that nothing here actually needs). Only ONE of the two ever
// actually ran — this one, since Cargo's default `[[bin]]` target is
// src/main.rs — which meant lib.rs's plugin registrations and its handful
// of unique commands (clipboard_copy/paste, save_file_from_data_url, and
// the bootc/battery/brightness/cache-file helpers this file had its own
// copies of already) were silently dead code. lib.rs has been removed;
// everything it had that this file was missing is folded in below, and the
// six previously-unregistered plugins (see the `.plugin(...)` chain) are
// now actually wired up.
//
// This file itself used to be ~1260 lines of commands all in one place; per
// request it's now split into src-tauri/src/types.rs (shared structs) and
// src-tauri/src/commands/{session,system_stats,network,power,display,
// config,ai,packages,misc}.rs (grouped by concern), imported below via
// glob `use` so the invoke_handler! list further down didn't need to change
// at all — same command names, same behavior, just organized into smaller
// files.
use types::*;
use commands::session::*;
use commands::system_stats::*;
use commands::network::*;
use commands::power::*;
use commands::display::*;
use commands::config::*;
use commands::ai::*;
use commands::packages::*;
use commands::misc::*;

fn main() {
    cache::ensure_dirs();

    let config = cache::load_user_config();
    let config_parsed: cache::UserConfig = serde_json::from_str(&config).unwrap_or_default();

    if config_parsed.panel_enabled {
        start_panel();
    }

    tauri::Builder::default()
    .manage(terminal_app::new_pty_sessions())
    // These 6 plugins were never registered in the old main.rs (it had
    // ZERO .plugin() calls despite the frontend depending on several of
    // them directly — e.g. every SystemBridge.pickFile/pickDirectory call
    // goes straight to tauri_plugin_dialog's JS bindings). Confirmed via
    // Cargo.toml: all 6 crates were already dependencies, just unused.
    .plugin(tauri_plugin_fs::init())
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_http::init())
    .plugin(tauri_plugin_notification::init())
    .plugin(tauri_plugin_process::init())
    .plugin(tauri_plugin_os::init())
    .plugin(tauri_plugin_clipboard_manager::init())
    .plugin(tauri_plugin_global_shortcut::Builder::new().build())
    .invoke_handler(tauri::generate_handler![
        get_session_type,
        get_system_apps, get_recent_apps, record_app_launch, invalidate_app_cache, launch_process,
        get_external_windows, focus_external_window, minimize_external_window, close_external_window, embed_external_window,
        exploler_app::list_files, exploler_app::read_text_file, exploler_app::write_text_file, exploler_app::git_status,
        get_system_stats, system_monitor_app::get_processes,
        system_monitor_app::get_cpu_metrics,
        system_monitor_app::get_memory_metrics,
        system_monitor_app::get_disk_metrics,
        system_monitor_app::get_network_metrics,
        system_monitor_app::get_gpu_metrics,
        system_monitor_app::get_temp_sensors,
        system_monitor_app::get_system_snapshot,
        system_monitor_app::kill_process,
        system_monitor_app::renice_process,
        read_config_file, write_config_file, read_cache_file, write_cache_file,
        blue_screenshot::take_screenshot, get_wallpapers, get_wallpaper_preview, load_distro_info, system_power,
        get_audio_sinks, set_sink_volume, set_default_sink, toggle_sink_mute, set_volume,
        get_wifi_networks_real, connect_wifi_real, disconnect_wifi, toggle_wifi,
        get_bluetooth_devices_real, bluetooth_connect, bluetooth_disconnect, bluetooth_pair,
        get_power_profiles, set_power_profile,
        set_brightness,
        save_config, load_config, save_window_state, load_window_state,
        exploler_app::read_file_as_data_url, exploler_app::create_folder, exploler_app::delete_file, exploler_app::copy_file, exploler_app::move_file,
        execute_command, pty_create, pty_write, pty_resize, pty_close, spawn_terminal, write_to_terminal,
        exploler_app::get_default_desktop_path, exploler_app::create_text_file, exploler_app::get_username, exploler_app::get_hostname, exploler_app::get_home_path,
        get_clipboard_history, add_to_clipboard_history, clear_clipboard_history,
        set_night_light_enabled, set_night_light_temperature,
        get_notification_history, save_notification_history,
        get_custom_themes, save_custom_theme, delete_custom_theme,
        ai_call, get_ai_config, save_ai_config,
        get_dnf_packages, get_flatpak_packages, get_appimage_packages,
        install_dnf_package, remove_dnf_package, update_dnf_package,
        get_native_packages, get_detected_backend,
        install_native_package, remove_native_package,
        get_bootc_status, bootc_upgrade, bootc_switch_image,
        install_flatpak_package, remove_flatpak_package, update_flatpak_package,
        install_appimage, remove_appimage, update_appimage,
        set_panel_enabled,
        clipboard_copy, clipboard_paste, write_clipboard_image, save_file_from_data_url,
        translate_text,
        installer_list_disks, installer_run,
        list_icon_themes, set_icon_theme,
        has_cellular_modem, get_cellular_status, set_cellular_enabled,
        save_pattern_lock, delete_pattern_lock, pattern_is_configured, has_fingerprint,
        camera_list_devices, camera_check_available, camera_capture_frame, camera_capture_photo, camera_record_video,
        web_open_native, web_fetch_site_info,
        start_language_server, stop_language_server,
        blue_archive_app::archive_list, blue_archive_app::archive_extract, blue_archive_app::archive_create,
        mail_app::mail_get_accounts, mail_app::mail_save_account, mail_app::mail_delete_account,
        mail_app::mail_fetch_inbox, mail_app::mail_send, mail_app::mail_mark_read, mail_app::mail_move_message,
        // SettingsApp backend commands
        SettingsApp::settings_get_displays,
        SettingsApp::settings_set_brightness,
        SettingsApp::settings_get_brightness,
        SettingsApp::settings_set_display_scale,
        SettingsApp::settings_set_resolution,
        SettingsApp::settings_get_wifi_networks,
        SettingsApp::settings_wifi_connect,
        SettingsApp::settings_wifi_disconnect,
        SettingsApp::settings_wifi_scan,
        SettingsApp::settings_wifi_toggle,
        SettingsApp::settings_get_bluetooth_devices,
        SettingsApp::settings_bluetooth_pair,
        SettingsApp::settings_bluetooth_connect,
        SettingsApp::settings_bluetooth_disconnect,
        SettingsApp::settings_bluetooth_remove,
        SettingsApp::settings_bluetooth_scan,
        SettingsApp::settings_bluetooth_toggle,
        SettingsApp::settings_get_battery,
        SettingsApp::settings_get_power_profiles,
        SettingsApp::settings_set_power_profile,
        SettingsApp::settings_get_current_user,
        SettingsApp::settings_get_users,
        SettingsApp::settings_change_password,
        SettingsApp::settings_set_avatar,
        SettingsApp::settings_get_system_info,
        SettingsApp::settings_get_night_light,
        SettingsApp::settings_set_night_light,
        SettingsApp::settings_get_panel_config,
        SettingsApp::settings_save_panel_config,
        SettingsApp::settings_send_to_compositor,
        SettingsApp::settings_set_workspace_count,
        SettingsApp::settings_set_dpms_timeout,
        SettingsApp::settings_get_wallpapers,
        // Blue Docs commands
        BlueDocs::docs_read_file,
        BlueDocs::docs_write_file,
        BlueDocs::docs_get_recent,
        BlueDocs::docs_get_meta,
        BlueDocs::docs_export_pdf,
        BlueDocs::docs_export_plaintext,
        BlueDocs::docs_export_markdown,
        BlueDocs::docs_spellcheck,
        BlueDocs::docs_get_templates,
        BlueDocs::docs_autosave,
        BlueDocs::docs_list_autosaved,
        BlueDocs::docs_read_docx,
        BlueDocs::docs_write_docx,
        BlueDocs::docs_read_pdf,
        BlueDocs::docs_export_docx,
    ])
    .setup(|app| {
        // ── Compositor IPC relay ──────────────────────────────────────────
        // Listens to the compositor's Unix socket and re-emits events
        // as Tauri events so the frontend can react to window list changes,
        // workspace switches, screenshot completions, etc.
        let app_handle = app.handle().clone();
        std::thread::spawn(move || {
            compositor_ipc_relay(app_handle);
        });
        Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running Blue Environment");
}
