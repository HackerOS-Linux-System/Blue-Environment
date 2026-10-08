use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

// ───────────────────────── skanowanie sysfs ─────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbDevice {
    pub id: String,
    pub vendor_id: String,
    pub product_id: String,
    pub manufacturer: String,
    pub product: String,
    /// `bDeviceClass` (hex), np. "00" (z interfejsów), "08" (pamięć masowa), "e0" (bezprzewodowe).
    pub class: String,
}

fn read_trim(p: &Path) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

/// Nazwy urządzeń USB w sysfs to `BUS-PORT[.PORT…]` (np. `1-2`, `3-1.4`);
/// `usbN` (huby główne) i `1-2:1.0` (interfejsy) pomijamy.
fn is_usb_device_name(n: &str) -> bool {
    let mut parts = n.splitn(2, '-');
    let (Some(bus), Some(ports)) = (parts.next(), parts.next()) else { return false };
    !bus.is_empty()
        && bus.chars().all(|c| c.is_ascii_digit())
        && !ports.is_empty()
        && ports.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

pub fn scan_usb(root: &Path) -> Vec<UsbDevice> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(root) else { return out };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !is_usb_device_name(&name) {
            continue;
        }
        let p = e.path();
        let (Some(vendor), Some(product_id)) = (read_trim(&p.join("idVendor")), read_trim(&p.join("idProduct"))) else { continue };
        let class = read_trim(&p.join("bDeviceClass")).unwrap_or_default().to_lowercase();
        if class == "09" {
            continue; // huby nie są „urządzeniami" dla użytkownika
        }
        out.push(UsbDevice {
            id: name,
            vendor_id: vendor,
            product_id,
            manufacturer: read_trim(&p.join("manufacturer")).unwrap_or_default(),
            product: read_trim(&p.join("product")).unwrap_or_default(),
            class,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Wymienne nośniki z `/sys/block`: dyski z `removable=1` albo na magistrali
/// USB. Dla dysku z partycjami zwraca partycje, inaczej sam dysk.
pub fn scan_removable_blocks(sys_block: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Ok(rd) = fs::read_dir(sys_block) else { return out };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if ["loop", "ram", "zram", "dm-", "md", "sr", "fd", "nbd"].iter().any(|p| name.starts_with(p)) {
            continue;
        }
        let p = e.path();
        let removable = read_trim(&p.join("removable")).as_deref() == Some("1");
        let on_usb = fs::canonicalize(p.join("device")).map(|d| d.to_string_lossy().contains("/usb")).unwrap_or(false);
        if !(removable || on_usb) {
            continue;
        }
        let parts: Vec<String> = fs::read_dir(&p)
            .map(|rd| {
                rd.flatten()
                    .filter(|c| c.path().join("partition").exists())
                    .map(|c| c.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        if parts.is_empty() {
            // Czytnik kart bez karty ma rozmiar 0 — to nie jest „podłączony nośnik".
            if read_trim(&p.join("size")).as_deref() == Some("0") {
                continue;
            }
            out.insert(name);
        } else {
            out.extend(parts);
        }
    }
    out
}

/// `sdb1` → `sdb`, `mmcblk0p1` → `mmcblk0`, `nvme0n1p2` → `nvme0n1`, `sdb` → `sdb`.
pub fn parent_disk(name: &str) -> String {
    let is_p_style = name.starts_with("mmcblk") || name.starts_with("nvme");
    if is_p_style {
        if let Some(i) = name.rfind('p') {
            let tail = &name[i + 1..];
            if i > 0 && !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) && name[..i].chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                return name[..i].to_string();
            }
        }
        return name.to_string();
    }
    name.trim_end_matches(|c: char| c.is_ascii_digit()).to_string()
}

// ───────────────────────── lsblk / udisksctl ─────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub name: String,
    pub label: String,
    pub size_bytes: u64,
    pub fstype: String,
    pub mountpoint: Option<String>,
    pub model: String,
    pub vendor: String,
    pub transport: String,
}

#[derive(Deserialize)]
struct LsblkNode {
    name: String,
    #[serde(default)] label: Option<String>,
    #[serde(default)] size: Option<serde_json::Value>,
    #[serde(default)] fstype: Option<String>,
    #[serde(default)] mountpoint: Option<String>,
    #[serde(default)] model: Option<String>,
    #[serde(default)] vendor: Option<String>,
    #[serde(default)] tran: Option<String>,
    #[serde(default)] children: Vec<LsblkNode>,
}

/// Spłaszcza drzewo `lsblk -J`; partycje dziedziczą model/producenta/magistralę po dysku.
pub fn parse_lsblk(json: &str) -> Vec<StorageInfo> {
    #[derive(Deserialize)]
    struct Root { blockdevices: Vec<LsblkNode> }
    fn walk(n: &LsblkNode, inherit: (&str, &str, &str), out: &mut Vec<StorageInfo>) {
        let t = |o: &Option<String>| o.clone().unwrap_or_default().trim().to_string();
        let (model, vendor, tran) = (
            if t(&n.model).is_empty() { inherit.0.to_string() } else { t(&n.model) },
            if t(&n.vendor).is_empty() { inherit.1.to_string() } else { t(&n.vendor) },
            if t(&n.tran).is_empty() { inherit.2.to_string() } else { t(&n.tran) },
        );
        let size_bytes = match &n.size {
            Some(serde_json::Value::Number(x)) => x.as_u64().unwrap_or(0),
            Some(serde_json::Value::String(s)) => s.parse().unwrap_or(0),
            _ => 0,
        };
        out.push(StorageInfo {
            name: n.name.clone(), label: t(&n.label), size_bytes, fstype: t(&n.fstype),
            mountpoint: n.mountpoint.clone().filter(|m| !m.is_empty()),
            model: model.clone(), vendor: vendor.clone(), transport: tran.clone(),
        });
        for c in &n.children {
            walk(c, (&model, &vendor, &tran), out);
        }
    }
    let Ok(root) = serde_json::from_str::<Root>(json) else { return vec![] };
    let mut out = Vec::new();
    for n in &root.blockdevices {
        walk(n, ("", "", ""), &mut out);
    }
    out
}

fn lsblk(disk: &str) -> Vec<StorageInfo> {
    Command::new("lsblk")
        .args(["-J", "-b", "-o", "NAME,LABEL,SIZE,FSTYPE,MOUNTPOINT,MODEL,VENDOR,TRAN"])
        .arg(format!("/dev/{disk}"))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_lsblk(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

fn valid_block_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 32 && n.chars().all(|c| c.is_ascii_alphanumeric())
        && ["sd", "hd", "vd", "mmcblk", "nvme", "sr"].iter().any(|p| n.starts_with(p))
}

/// Wyciąga punkt montowania z komunikatu `udisksctl mount` — zarówno z sukcesu
/// („Mounted /dev/sdb1 at /run/media/u/ETYKIETA."), jak i z błędu „już zamontowane".
pub fn parse_udisks_mount(out: &str) -> Option<String> {
    if let Some(i) = out.find(" at ") {
        let rest = out[i + 4..].trim();
        let rest = rest.trim_end_matches(['.', '\n', ' ']);
        if rest.starts_with('/') {
            return Some(rest.to_string());
        }
    }
    if let (Some(a), Some(b)) = (out.find('`'), out.rfind('\'')) {
        if b > a + 1 && out[a + 1..b].starts_with('/') {
            return Some(out[a + 1..b].to_string());
        }
    }
    None
}

fn udisksctl() -> Result<std::path::PathBuf, String> {
    crate::backend::find_in_path("udisksctl").ok_or_else(|| "Zainstaluj pakiet udisks2, aby montować i wysuwać nośniki".to_string())
}

/// Montuje partycję (jeśli trzeba) i zwraca punkt montowania.
#[tauri::command]
pub async fn device_mount(name: String) -> Result<String, String> {
    if !valid_block_name(&name) {
        return Err("Nieprawidłowa nazwa urządzenia".into());
    }
    tokio::task::spawn_blocking(move || {
        if let Some(mp) = lsblk(&name).iter().find(|d| d.name == name).and_then(|d| d.mountpoint.clone()) {
            return Ok(mp); // już zamontowane
        }
        let out = Command::new(udisksctl()?)
            .args(["mount", "--no-user-interaction", "-b", &format!("/dev/{name}")])
            .output()
            .map_err(|e| e.to_string())?;
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        parse_udisks_mount(&text).ok_or_else(|| text.trim().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Odmontowuje i (dla całego urządzenia) wyłącza zasilanie — bezpieczne wyjęcie.
#[tauri::command]
pub async fn device_eject(name: String) -> Result<(), String> {
    if !valid_block_name(&name) {
        return Err("Nieprawidłowa nazwa urządzenia".into());
    }
    tokio::task::spawn_blocking(move || {
        let ctl = udisksctl()?;
        let disk = parent_disk(&name);
        // Odmontuj wszystkie zamontowane partycje tego dysku.
        for d in lsblk(&disk).iter().filter(|d| d.mountpoint.is_some()) {
            let out = Command::new(&ctl)
                .args(["unmount", "--no-user-interaction", "-b", &format!("/dev/{}", d.name)])
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!("Nie można odmontować {}: {}", d.name, String::from_utf8_lossy(&out.stderr).trim()));
            }
        }
        let out = Command::new(&ctl)
            .args(["power-off", "--no-user-interaction", "-b", &format!("/dev/{disk}")])
            .output()
            .map_err(|e| e.to_string())?;
        // Nie każde urządzenie da się wyłączyć (np. karta w czytniku) — samo odmontowanie wystarcza.
        let _ = out;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ───────────────────────── monitor ─────────────────────────

#[derive(Serialize, Clone)]
struct Removed {
    name: String,
}

/// Różnica zbiorów kluczy: (dodane, usunięte).
pub fn diff_keys<V>(old: &HashMap<String, V>, new: &HashMap<String, V>) -> (Vec<String>, Vec<String>) {
    let mut added: Vec<String> = new.keys().filter(|k| !old.contains_key(*k)).cloned().collect();
    let mut removed: Vec<String> = old.keys().filter(|k| !new.contains_key(*k)).cloned().collect();
    added.sort();
    removed.sort();
    (added, removed)
}

pub fn spawn_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut usb_prev: Option<HashMap<String, UsbDevice>> = None;
        let mut blk_prev: Option<BTreeSet<String>> = None;
        loop {
            let usb: HashMap<String, UsbDevice> = scan_usb(Path::new("/sys/bus/usb/devices")).into_iter().map(|d| (d.id.clone(), d)).collect();
            let blk = scan_removable_blocks(Path::new("/sys/block"));

            if let Some(prev) = &usb_prev {
                let (added, removed) = diff_keys(prev, &usb);
                for id in added {
                    let _ = app.emit("device:usb-added", &usb[&id]);
                }
                for id in removed {
                    let _ = app.emit("device:usb-removed", &prev[&id]);
                }
            }
            if let Some(prev) = &blk_prev {
                let added: Vec<String> = blk.difference(prev).cloned().collect();
                let removed: Vec<String> = prev.difference(&blk).cloned().collect();
                if !added.is_empty() {
                    // Daj jądru/udev chwilę na rozpoznanie systemu plików i etykiety.
                    tokio::time::sleep(Duration::from_millis(1200)).await;
                    for name in added {
                        let n2 = name.clone();
                        let info = tokio::task::spawn_blocking(move || lsblk(&parent_disk(&n2)).into_iter().find(|d| d.name == n2)).await.ok().flatten();
                        let info = info.unwrap_or(StorageInfo {
                            name: name.clone(), label: String::new(), size_bytes: 0, fstype: String::new(),
                            mountpoint: None, model: String::new(), vendor: String::new(), transport: String::new(),
                        });
                        let _ = app.emit("device:storage-added", info);
                    }
                }
                for name in removed {
                    let _ = app.emit("device:storage-removed", Removed { name });
                }
            }
            usb_prev = Some(usb);
            blk_prev = Some(blk);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("blue-dev-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn put(p: &Path, s: &str) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, s).unwrap();
    }

    #[test]
    fn usb_scan_skips_hubs_interfaces_and_root_hubs() {
        let d = tmp("usb");
        for (dir, vid, class) in [("1-2", "0781", "00"), ("1-3", "1d6b", "09"), ("3-1.4", "046d", "08")] {
            put(&d.join(dir).join("idVendor"), vid);
            put(&d.join(dir).join("idProduct"), "5581");
            put(&d.join(dir).join("bDeviceClass"), class);
        }
        put(&d.join("1-2").join("product"), "Ultra\n");
        put(&d.join("1-2").join("manufacturer"), "SanDisk");
        put(&d.join("usb1").join("idVendor"), "1d6b");        // root hub
        put(&d.join("1-2:1.0").join("idVendor"), "0781");      // interfejs
        let v = scan_usb(&d);
        assert_eq!(v.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), vec!["1-2", "3-1.4"]);
        assert_eq!((v[0].manufacturer.as_str(), v[0].product.as_str()), ("SanDisk", "Ultra"));
        assert_eq!(v[1].class, "08");
        assert!(is_usb_device_name("3-1.4") && !is_usb_device_name("usb1") && !is_usb_device_name("1-2:1.0") && !is_usb_device_name("1-"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn block_scan_finds_removable_partitions_only() {
        let d = tmp("blk");
        put(&d.join("sdb/removable"), "1\n");
        put(&d.join("sdb/size"), "30000000\n");
        put(&d.join("sdb/sdb1/partition"), "1");
        put(&d.join("sdb/sdb2/partition"), "2");
        put(&d.join("sdc/removable"), "1\n");               // dysk bez partycji (np. sformatowany w całości)
        put(&d.join("sdc/size"), "1000\n");
        put(&d.join("sdd/removable"), "1\n");               // pusty czytnik kart
        put(&d.join("sdd/size"), "0\n");
        put(&d.join("nvme0n1/removable"), "0\n");           // wewnętrzny
        put(&d.join("nvme0n1/nvme0n1p1/partition"), "1");
        put(&d.join("loop0/removable"), "1\n");
        let s = scan_removable_blocks(&d);
        assert_eq!(s.iter().map(|x| x.as_str()).collect::<Vec<_>>(), vec!["sdb1", "sdb2", "sdc"]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn parent_disk_names() {
        assert_eq!(parent_disk("sdb1"), "sdb");
        assert_eq!(parent_disk("sdb"), "sdb");
        assert_eq!(parent_disk("mmcblk0p1"), "mmcblk0");
        assert_eq!(parent_disk("mmcblk0"), "mmcblk0");
        assert_eq!(parent_disk("nvme0n1p2"), "nvme0n1");
        assert_eq!(parent_disk("nvme0n1"), "nvme0n1");
    }

    #[test]
    fn lsblk_tree_is_flattened_with_inherited_model() {
        let j = r#"{"blockdevices":[{"name":"sdb","label":null,"size":31914983424,"fstype":null,"mountpoint":null,"model":"Ultra ","vendor":"SanDisk ","tran":"usb",
          "children":[{"name":"sdb1","label":"KLUCZYK","size":"31913934848","fstype":"vfat","mountpoint":"/run/media/u/KLUCZYK","model":null,"vendor":null,"tran":null}]}]}"#;
        let v = parse_lsblk(j);
        assert_eq!(v.len(), 2);
        let p = v.iter().find(|x| x.name == "sdb1").unwrap();
        assert_eq!((p.label.as_str(), p.fstype.as_str(), p.size_bytes), ("KLUCZYK", "vfat", 31913934848));
        assert_eq!((p.model.as_str(), p.vendor.as_str(), p.transport.as_str()), ("Ultra", "SanDisk", "usb"));
        assert_eq!(p.mountpoint.as_deref(), Some("/run/media/u/KLUCZYK"));
        assert!(parse_lsblk("nie json").is_empty());
    }

    #[test]
    fn udisks_mount_output_is_parsed() {
        assert_eq!(parse_udisks_mount("Mounted /dev/sdb1 at /run/media/u/MOJ DYSK.\n").as_deref(), Some("/run/media/u/MOJ DYSK"));
        assert_eq!(
            parse_udisks_mount("Error mounting /dev/sdb1: GDBus.Error:org.freedesktop.UDisks2.Error.AlreadyMounted: Device /dev/sdb1 is already mounted at `/run/media/u/X'.").as_deref(),
            Some("/run/media/u/X")
        );
        assert!(parse_udisks_mount("Error: Not authorized").is_none());
    }

    #[test]
    fn block_names_are_validated_and_diffed() {
        assert!(valid_block_name("sdb1") && valid_block_name("mmcblk0p1") && valid_block_name("nvme0n1p1"));
        assert!(!valid_block_name("../etc") && !valid_block_name("sdb1; rm") && !valid_block_name("loop0") && !valid_block_name(""));
        let mut a = HashMap::new();
        a.insert("1-1".to_string(), 1);
        a.insert("1-2".to_string(), 2);
        let mut b = HashMap::new();
        b.insert("1-2".to_string(), 2);
        b.insert("1-3".to_string(), 3);
        assert_eq!(diff_keys(&a, &b), (vec!["1-3".to_string()], vec!["1-1".to_string()]));
    }
}
