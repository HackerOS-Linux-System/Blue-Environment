use bollard::container::{
    ListContainersOptions, LogsOptions, RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::image::{CreateImageOptions, ListImagesOptions, RemoveImageOptions};
use bollard::Docker;
use futures_util::StreamExt;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

fn socket_candidates() -> Vec<(String, PathBuf)> {
    let mut v = Vec::new();
    if let Ok(rt) = std::env::var("XDG_RUNTIME_DIR") {
        v.push(("podman".to_string(), PathBuf::from(rt).join("podman/podman.sock")));
    }
    v.push(("docker".to_string(), PathBuf::from("/var/run/docker.sock")));
    v
}

fn connect() -> Result<(Docker, String), String> {
    for (engine, path) in socket_candidates() {
        if path.exists() {
            let docker = Docker::connect_with_unix(&path.to_string_lossy(), 120, bollard::API_DEFAULT_VERSION)
                .map_err(|e| e.to_string())?;
            return Ok((docker, engine));
        }
    }
    Err("Brak działającego silnika kontenerów. Uruchom: systemctl --user enable --now podman.socket".into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub engine: Option<String>,
    pub connected: bool,
    pub distrobox: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn containers_status() -> EngineStatus {
    let distrobox = crate::backend::find_in_path("distrobox").is_some();
    match connect() {
        Ok((docker, engine)) => match docker.ping().await {
            Ok(_) => EngineStatus { engine: Some(engine), connected: true, distrobox, error: None },
            Err(e) => EngineStatus { engine: Some(engine), connected: false, distrobox, error: Some(e.to_string()) },
        },
        Err(e) => EngineStatus { engine: None, connected: false, distrobox, error: Some(e) },
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub created: i64,
    /// Utworzony przez distrobox (etykieta `manager=distrobox`).
    pub distrobox: bool,
}

#[tauri::command]
pub async fn containers_list() -> Result<Vec<ContainerInfo>, String> {
    let (docker, _) = connect()?;
    let list = docker
        .list_containers(Some(ListContainersOptions::<String> { all: true, ..Default::default() }))
        .await
        .map_err(|e| e.to_string())?;
    Ok(list
        .into_iter()
        .map(|c| ContainerInfo {
            id: c.id.unwrap_or_default(),
            name: c.names.and_then(|n| n.into_iter().next()).unwrap_or_default().trim_start_matches('/').to_string(),
            image: c.image.unwrap_or_default(),
            state: c.state.unwrap_or_default(),
            status: c.status.unwrap_or_default(),
            created: c.created.unwrap_or(0),
            distrobox: c.labels.map(|l| l.get("manager").map(|m| m == "distrobox").unwrap_or(false)).unwrap_or(false),
        })
        .collect())
}

fn valid_ref(s: &str) -> bool {
    !s.is_empty() && s.len() < 256 && s.chars().all(|c| c.is_ascii_alphanumeric() || "._-/:@".contains(c))
}

#[tauri::command]
pub async fn containers_action(id: String, action: String) -> Result<(), String> {
    if !valid_ref(&id) {
        return Err("nieprawidłowy identyfikator".into());
    }
    let (docker, _) = connect()?;
    let r = match action.as_str() {
        "start" => docker.start_container(&id, None::<StartContainerOptions<String>>).await,
        "stop" => docker.stop_container(&id, Some(StopContainerOptions { t: 10 })).await,
        "restart" => docker.restart_container(&id, None).await,
        "pause" => docker.pause_container(&id).await,
        "unpause" => docker.unpause_container(&id).await,
        "remove" => {
            docker
                .remove_container(&id, Some(RemoveContainerOptions { force: true, ..Default::default() }))
                .await
        }
        other => return Err(format!("nieznana akcja: {other}")),
    };
    r.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn containers_logs(id: String, tail: Option<u32>) -> Result<String, String> {
    if !valid_ref(&id) {
        return Err("nieprawidłowy identyfikator".into());
    }
    let (docker, _) = connect()?;
    let opts = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        tail: tail.unwrap_or(200).min(5000).to_string(),
        ..Default::default()
    };
    let mut stream = docker.logs(&id, Some(opts));
    let mut out = String::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(line) => out.push_str(&line.to_string()),
            Err(e) => return Err(e.to_string()),
        }
        if out.len() > 512 * 1024 {
            break;
        }
    }
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub id: String,
    pub tags: Vec<String>,
    pub size: i64,
    pub created: i64,
}

#[tauri::command]
pub async fn containers_images() -> Result<Vec<ImageInfo>, String> {
    let (docker, _) = connect()?;
    let list = docker
        .list_images(Some(ListImagesOptions::<String> { all: false, ..Default::default() }))
        .await
        .map_err(|e| e.to_string())?;
    Ok(list
        .into_iter()
        .map(|i| ImageInfo { id: i.id, tags: i.repo_tags, size: i.size, created: i.created })
        .collect())
}

#[tauri::command]
pub async fn containers_pull_image(image: String) -> Result<(), String> {
    if !valid_ref(&image) {
        return Err("nieprawidłowa nazwa obrazu".into());
    }
    let (docker, _) = connect()?;
    let mut stream = docker.create_image(
        Some(CreateImageOptions { from_image: image, ..Default::default() }),
        None,
        None,
    );
    while let Some(item) = stream.next().await {
        item.map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn containers_remove_image(id: String) -> Result<(), String> {
    if !valid_ref(&id) {
        return Err("nieprawidłowy identyfikator".into());
    }
    let (docker, _) = connect()?;
    docker
        .remove_image(&id, Some(RemoveImageOptions { force: true, ..Default::default() }), None)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ───────────────────────── distrobox ─────────────────────────

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DistroboxEntry {
    pub id: String,
    pub name: String,
    pub status: String,
    pub image: String,
}

/// Parsuje `distrobox list --no-color` (kolumny rozdzielone `|`).
pub fn parse_distrobox_list(out: &str) -> Vec<DistroboxEntry> {
    out.lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<&str> = l.split('|').map(|s| s.trim()).collect();
            (c.len() >= 4 && !c[1].is_empty())
                .then(|| DistroboxEntry { id: c[0].into(), name: c[1].into(), status: c[2].into(), image: c[3].into() })
        })
        .collect()
}

fn run_distrobox(args: &[&str]) -> Result<String, String> {
    let bin = crate::backend::find_in_path("distrobox").ok_or("distrobox nie jest zainstalowany")?;
    let out = Command::new(bin).args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[tauri::command]
pub fn distrobox_list() -> Result<Vec<DistroboxEntry>, String> {
    Ok(parse_distrobox_list(&run_distrobox(&["list", "--no-color"])?))
}

#[tauri::command]
pub async fn distrobox_create(name: String, image: String, nvidia: bool, init: bool) -> Result<(), String> {
    if !valid_ref(&name) || name.contains('/') || !valid_ref(&image) {
        return Err("nieprawidłowa nazwa lub obraz".into());
    }
    tokio::task::spawn_blocking(move || {
        let mut args = vec!["create", "--yes", "--name", &name, "--image", &image];
        if nvidia {
            args.push("--nvidia");
        }
        if init {
            args.push("--init");
            args.push("--additional-packages");
            args.push("systemd");
        }
        run_distrobox(&args).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn distrobox_enter(name: String) -> Result<(), String> {
    if !valid_ref(&name) || name.contains('/') {
        return Err("nieprawidłowa nazwa".into());
    }
    // Pierwszy dostępny emulator terminala.
    const TERMS: &[(&str, &str)] = &[
        ("foot", "-e"), ("kgx", "-e"), ("gnome-terminal", "--"), ("konsole", "-e"),
        ("alacritty", "-e"), ("kitty", ""), ("xterm", "-e"),
    ];
    for (term, flag) in TERMS {
        if let Some(bin) = crate::backend::find_in_path(term) {
            let mut c = Command::new(bin);
            if !flag.is_empty() {
                c.arg(flag);
            }
            c.args(["distrobox", "enter", &name]);
            return c.spawn().map(|_| ()).map_err(|e| e.to_string());
        }
    }
    Err("nie znaleziono emulatora terminala".into())
}

#[tauri::command]
pub async fn distrobox_remove(name: String) -> Result<(), String> {
    if !valid_ref(&name) || name.contains('/') {
        return Err("nieprawidłowa nazwa".into());
    }
    tokio::task::spawn_blocking(move || run_distrobox(&["rm", "--force", &name]).map(|_| ()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn distrobox_upgrade(name: String) -> Result<(), String> {
    if !valid_ref(&name) || name.contains('/') {
        return Err("nieprawidłowa nazwa".into());
    }
    tokio::task::spawn_blocking(move || run_distrobox(&["upgrade", &name]).map(|_| ()))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_distrobox_list() {
        let out = "ID           | NAME        | STATUS          | IMAGE\nabc123       | arch        | Up 2 hours      | docker.io/library/archlinux:latest\n";
        let v = parse_distrobox_list(out);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "arch");
        assert_eq!(v[0].image, "docker.io/library/archlinux:latest");
    }

    #[test]
    fn rejects_shell_metacharacters() {
        assert!(valid_ref("docker.io/library/ubuntu:24.04"));
        assert!(!valid_ref("x; rm -rf /"));
        assert!(!valid_ref(""));
    }
}
