use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LvDomain {
    pub name: String,
    pub uuid: String,
    pub state: String,
    pub memory_mb: u64,
    pub vcpus: u32,
    pub autostart: bool,
}

/// Nazwa stanu domeny (`virDomainState`).
pub fn state_name(state: u32) -> &'static str {
    match state {
        1 => "running",
        2 => "blocked",
        3 => "paused",
        4 => "shutting-down",
        5 => "stopped",
        6 => "crashed",
        7 => "suspended",
        _ => "unknown",
    }
}

fn valid_uri(uri: &str) -> bool {
    matches!(uri, "qemu:///session" | "qemu:///system")
}

fn valid_action(a: &str) -> bool {
    matches!(a, "start" | "shutdown" | "destroy" | "reboot" | "suspend" | "resume")
}

#[cfg(feature = "libvirt")]
mod imp {
    use super::*;
    use virt::connect::Connect;
    use virt::domain::Domain;

    fn connect(uri: &str) -> Result<Connect, String> {
        Connect::open(Some(uri)).map_err(|e| format!("libvirt ({uri}): {e}"))
    }

    pub fn list(uri: &str) -> Result<Vec<LvDomain>, String> {
        let mut conn = connect(uri)?;
        let result = (|| {
            let doms = conn.list_all_domains(0).map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            for d in doms {
                let info = d.get_info().map_err(|e| e.to_string())?;
                out.push(LvDomain {
                    name: d.get_name().unwrap_or_default(),
                    uuid: d.get_uuid_string().unwrap_or_default(),
                    state: state_name(info.state as u32).to_string(),
                    memory_mb: info.max_mem / 1024, // KiB → MiB
                    vcpus: info.nr_virt_cpu,
                    autostart: d.get_autostart().unwrap_or(false),
                });
            }
            out.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(out)
        })();
        let _ = conn.close();
        result
    }

    pub fn action(uri: &str, name: &str, action: &str) -> Result<(), String> {
        let mut conn = connect(uri)?;
        let result = (|| {
            let dom = Domain::lookup_by_name(&conn, name).map_err(|e| format!("domena „{name}”: {e}"))?;
            match action {
                "start" => dom.create().map(|_| ()),
                "shutdown" => dom.shutdown().map(|_| ()),
                "destroy" => dom.destroy().map(|_| ()),
                "reboot" => dom.reboot(0).map(|_| ()),
                "suspend" => dom.suspend().map(|_| ()),
                _ => dom.resume().map(|_| ()),
            }
            .map_err(|e| e.to_string())
        })();
        let _ = conn.close();
        result
    }
}

#[cfg(not(feature = "libvirt"))]
mod imp {
    use super::*;
    const MSG: &str = "Ta wersja Blue Environment jest zbudowana bez libvirt (cecha `libvirt`).";
    pub fn list(_: &str) -> Result<Vec<LvDomain>, String> { Err(MSG.into()) }
    pub fn action(_: &str, _: &str, _: &str) -> Result<(), String> { Err(MSG.into()) }
}

#[tauri::command]
pub fn lv_compiled_in() -> bool {
    cfg!(feature = "libvirt")
}

#[tauri::command]
pub async fn lv_list(uri: Option<String>) -> Result<Vec<LvDomain>, String> {
    let uri = uri.unwrap_or_else(|| "qemu:///session".into());
    if !valid_uri(&uri) {
        return Err("nieobsługiwany URI libvirt".into());
    }
    tokio::task::spawn_blocking(move || imp::list(&uri)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn lv_action(uri: Option<String>, name: String, action: String) -> Result<(), String> {
    let uri = uri.unwrap_or_else(|| "qemu:///session".into());
    if !valid_uri(&uri) || !valid_action(&action) || name.is_empty() || name.len() > 128 {
        return Err("nieprawidłowe parametry".into());
    }
    tokio::task::spawn_blocking(move || imp::action(&uri, &name, &action)).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_names() {
        assert_eq!(state_name(1), "running");
        assert_eq!(state_name(5), "stopped");
        assert_eq!(state_name(99), "unknown");
    }

    #[test]
    fn validation() {
        assert!(valid_uri("qemu:///session") && !valid_uri("qemu+ssh://evil/system"));
        assert!(valid_action("start") && !valid_action("rm -rf"));
    }

    #[cfg(not(feature = "libvirt"))]
    #[test]
    fn without_feature_reports_clear_error() {
        assert!(imp::list("qemu:///session").unwrap_err().contains("libvirt"));
    }
}
