slint::include_modules!();

use slint::{ModelRc, VecModel, SharedString, Weak};
use tokio::process::Command;
use std::process::Stdio;

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let ui = BlueSoftware::new()?;

    // Inicjalizacja
    ui.set_current_tab(0);
    ui.set_is_loading(false);
    ui.set_show_dialog(false);
    ui.set_current_results(ModelRc::new(VecModel::<AppInfo>::from(vec![])));

    let ui_weak = ui.as_weak();

    /* ====================== WYSZUKIWANIE ====================== */
    async fn search_flatpak(query: String) -> Vec<AppInfo> {
        let output = match Command::new("flatpak").args(["search", &query]).output().await {
            Ok(o) => o,
            Err(_) => return vec![],
        };
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .skip(1)
            .take(15)
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let trimmed = line.trim();
                let words: Vec<&str> = trimmed.split_whitespace().collect();
                let name = words.first().copied().unwrap_or("").to_string();
                let id = words.iter().find(|w| w.contains('.')).copied().unwrap_or(&name).to_string();
                AppInfo {
                    name: SharedString::from(name),
                    id: SharedString::from(id),
                    desc: SharedString::from(trimmed),
                    backend: SharedString::from("flatpak"),
                }
            })
            .collect()
    }

    async fn search_snap(query: String) -> Vec<AppInfo> {
        let output = match Command::new("snap").args(["find", &query]).output().await {
            Ok(o) => o,
            Err(_) => return vec![],
        };
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .skip(1)
            .take(15)
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let trimmed = line.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let name = parts.first().copied().unwrap_or("unknown").to_string();
                AppInfo {
                    name: SharedString::from(&name),
                    id: SharedString::from(&name),
                    desc: SharedString::from(if parts.len() > 4 {
                        parts[4..].join(" ")
                    } else {
                        trimmed
                    }),
                    backend: SharedString::from("snap"),
                }
            })
            .collect()
    }

    async fn search_lpm(query: String) -> Vec<AppInfo> {
        let output = match Command::new("apt-cache").args(["search", &query]).output().await {
            Ok(o) => o,
            Err(_) => return vec![],
        };
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .take(15)
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| {
                let trimmed = line.trim();
                if let Some((name, desc)) = trimmed.split_once(" - ") {
                    Some(AppInfo {
                        name: SharedString::from(name),
                        id: SharedString::from(name),
                        desc: SharedString::from(desc),
                        backend: SharedString::from("lpm"),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /* ====================== WYKONYWANIE POLECEŃ ====================== */
    async fn run_command(args: &[String], title: &str) -> String {
        let mut cmd = Command::new(&args[0]);
        cmd.args(&args[1..]).stdout(Stdio::piped()).stderr(Stdio::piped());
        let output = match cmd.output().await {
            Ok(o) => o,
            Err(e) => return format!("Błąd uruchomienia: {}\n", e),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!("{}:\n{}\n{}", title, stdout, stderr)
    }

    async fn execute_and_show(weak: Weak<BlueSoftware>, cmd: Vec<String>, title: String) {
        let _ = weak.upgrade_in_event_loop(|ui| ui.set_is_loading(true));
        let output = run_command(&cmd, &title).await;

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_dialog_title(SharedString::from(title));
                ui.set_dialog_output(SharedString::from(output));
                ui.set_show_dialog(true);
                ui.set_is_loading(false);
            }
        });
    }

    /* ====================== CALLBACKI ====================== */
    let weak = ui_weak.clone();

    /* Wyszukiwanie */
    let do_search = move |weak: Weak<BlueSoftware>, tab: i32, query: SharedString| {
        tokio::spawn(async move {
            let results = match tab {
                0 => search_flatpak(query.to_string()).await,
                1 => search_snap(query.to_string()).await,
                2 => search_lpm(query.to_string()).await,
                _ => vec![],
            };
            let model = ModelRc::new(VecModel::from(results));

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_current_results(model);
                    ui.set_is_loading(false);
                }
            });
        });
    };

    ui.on_search_changed({
        let weak = weak.clone();
        move |q| {
            let weak2 = weak.clone();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                let tab = ui.get_current_tab();
                ui.set_search_query(q.clone());
                do_search(weak2.clone(), tab, q);
            });
        }
    });

    ui.on_tab_changed({
        let weak = weak.clone();
        move |tab| {
            let weak2 = weak.clone();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                let q = ui.get_search_query();
                do_search(weak2.clone(), tab, q);
            });
        }
    });

    ui.on_refresh({
        let weak = weak.clone();
        move || {
            let weak2 = weak.clone();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                let tab = ui.get_current_tab();
                let q = ui.get_search_query();
                do_search(weak2.clone(), tab, q);
            });
        }
    });

    /* Instalacja / Usuwanie */
    ui.on_install_app({
        let weak = weak.clone();
        move |app| {
            let weak2 = weak.clone();
            let backend = app.backend.clone();
            let id = app.id.clone();

            let _ = weak.upgrade_in_event_loop(move |ui| {
                let cmd = match backend.as_str() {
                    "flatpak" => vec!["flatpak".into(), "install".into(), "-y".into(), "flathub".into(), id.to_string()],
                    "snap" => vec!["sudo".into(), "snap".into(), "install".into(), id.to_string()],
                    "lpm" => vec!["sudo".into(), "lpm".into(), "install".into(), id.to_string()],
                    _ => return,
                };
                let title = match backend.as_str() {
                    "flatpak" => "Instalacja Flatpak",
                    "snap" => "Instalacja Snap",
                    "lpm" => "Instalacja LPM (APT)",
                    _ => "Instalacja",
                }
                .to_string();

                tokio::spawn(execute_and_show(weak2.clone(), cmd, title));
            });
        }
    });

    ui.on_remove_app({
        let weak = weak.clone();
        move |app| {
            let weak2 = weak.clone();
            let backend = app.backend.clone();
            let id = app.id.clone();

            let _ = weak.upgrade_in_event_loop(move |ui| {
                let cmd = match backend.as_str() {
                    "flatpak" => vec!["flatpak".into(), "uninstall".into(), "-y".into(), id.to_string()],
                    "snap" => vec!["sudo".into(), "snap".into(), "remove".into(), id.to_string()],
                    "lpm" => vec!["sudo".into(), "lpm".into(), "remove".into(), id.to_string()],
                    _ => return,
                };
                let title = match backend.as_str() {
                    "flatpak" => "Usuwanie Flatpak",
                    "snap" => "Usuwanie Snap",
                    "lpm" => "Usuwanie LPM (APT)",
                    _ => "Usuwanie",
                }
                .to_string();

                tokio::spawn(execute_and_show(weak2.clone(), cmd, title));
            });
        }
    });

    /* AppImage */
    ui.on_install_appimage({
        let weak = weak.clone();
        move |name, url| {
            if name.trim().is_empty() || url.trim().is_empty() {
                return;
            }
            let name = name.to_string();
            let url = url.to_string();
            let weak2 = weak.clone();

            tokio::spawn(async move {
                let result = install_appimage(name, url).await;
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = weak2.upgrade() {
                        ui.set_dialog_title(SharedString::from("AppImage"));
                        ui.set_dialog_output(SharedString::from(result));
                        ui.set_show_dialog(true);
                    }
                });
            });
        }
    });

    async fn install_appimage(name: String, url: String) -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let dir = format!("{}/Applications", home);
        let _ = std::fs::create_dir_all(&dir);

        let path = format!("{}/{}.AppImage", dir, name);
        let wget = Command::new("wget").args([&url, "-O", &path]).output().await;

        if let Ok(res) = wget {
            if res.status.success() {
                let _ = Command::new("chmod").args(["+x", &path]).output().await;

                let desktop_dir = format!("{}/.local/share/applications", home);
                let _ = std::fs::create_dir_all(&desktop_dir);
                let desktop_path = format!("{}/{}.desktop", desktop_dir, name);

                let content = format!(
                    r#"[Desktop Entry]
Name={}
Exec={}
Icon=application-x-executable
Type=Application
Categories=Utility;"#,
                    name, path
                );

                if std::fs::write(desktop_path, content).is_ok() {
                    return "✅ AppImage zainstalowany pomyślnie!\nMożesz go uruchomić z menu aplikacji.".to_string();
                }
            }
        }
        "❌ Błąd instalacji AppImage".to_string()
    }

    ui.run()
}
