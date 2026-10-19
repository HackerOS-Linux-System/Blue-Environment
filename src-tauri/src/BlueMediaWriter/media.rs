use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

/// A whole-disk device the user may choose as a target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    /// `/dev/sdb`
    pub path: String,
    pub model: String,
    pub vendor: String,
    pub size_bytes: u64,
    /// Transport as reported by lsblk (`usb`, `sata`, `nvme`, …), may be empty.
    pub transport: String,
    /// Every mount point of the disk or any of its partitions.
    pub mounted: Vec<String>,
    /// Filesystem labels of the partitions (shown so the user recognises the stick).
    pub labels: Vec<String>,
}

/// Mount points that mean "this disk carries the running system".
const SYSTEM_MOUNTS: &[&str] = &[
    "/", "/boot", "/boot/efi", "/efi", "/usr", "/var", "/etc", "/home", "/opt", "/nix", "/sysroot",
    "[SWAP]", "/run/live/medium", "/cdrom", "/run/initramfs/live",
];

/// Largest drive we will offer. Anything bigger is almost certainly a
/// backup/data disk, not a USB stick (can be overridden by the caller).
pub const DEFAULT_MAX_TARGET_BYTES: u64 = 2 * 1024 * 1024 * 1024 * 1024; // 2 TiB

fn s(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(x)) => x.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => (if *b { "1" } else { "0" }).to_string(),
        _ => String::new(),
    }
}

fn truthy(v: &Value, k: &str) -> bool {
    match v.get(k) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(x)) => x == "1" || x.eq_ignore_ascii_case("true"),
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0) != 0,
        _ => false,
    }
}

fn collect_mounts(node: &Value, out: &mut Vec<String>, labels: &mut Vec<String>) {
    if let Some(mps) = node.get("mountpoints").and_then(|m| m.as_array()) {
        for m in mps {
            if let Some(p) = m.as_str() {
                if !p.is_empty() {
                    out.push(p.to_string());
                }
            }
        }
    }
    if let Some(p) = node.get("mountpoint").and_then(|m| m.as_str()) {
        if !p.is_empty() {
            out.push(p.to_string());
        }
    }
    let label = s(node, "label");
    if !label.is_empty() {
        labels.push(label);
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for c in children {
            collect_mounts(c, out, labels);
        }
    }
}

/// True when the disk or any partition/holder below it is mounted at a path
/// that belongs to the running system.
pub fn carries_system(mounts: &[String]) -> bool {
    mounts.iter().any(|m| SYSTEM_MOUNTS.iter().any(|sm| m == sm))
}

/// Parses `lsblk -J -b -o NAME,PATH,SIZE,RM,HOTPLUG,TRAN,MODEL,VENDOR,TYPE,RO,MOUNTPOINTS,LABEL`
/// and returns **only drives that are safe to offer**:
/// whole disks, writable, removable / hot-pluggable / USB, not carrying the
/// running system and not larger than `max_bytes`.
pub fn parse_lsblk(json: &str, max_bytes: u64) -> Result<Vec<DriveInfo>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("Cannot parse lsblk output: {e}"))?;
    let devices = v
        .get("blockdevices")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "lsblk output has no \"blockdevices\"".to_string())?;
    let mut out = Vec::new();
    for d in devices {
        if s(d, "type") != "disk" {
            continue;
        }
        if truthy(d, "ro") {
            continue;
        }
        let transport = s(d, "tran");
        let removable = truthy(d, "rm") || truthy(d, "hotplug") || transport == "usb";
        if !removable {
            continue;
        }
        let size: u64 = s(d, "size").parse().unwrap_or(0);
        if size == 0 || size > max_bytes {
            continue; // empty card reader slot, or implausibly large for a stick
        }
        let mut mounted = Vec::new();
        let mut labels = Vec::new();
        collect_mounts(d, &mut mounted, &mut labels);
        if carries_system(&mounted) {
            continue;
        }
        let path = {
            let p = s(d, "path");
            if p.is_empty() { format!("/dev/{}", s(d, "name")) } else { p }
        };
        if !is_valid_device_path(&path) {
            continue;
        }
        out.push(DriveInfo {
            path,
            model: s(d, "model"),
            vendor: s(d, "vendor"),
            size_bytes: size,
            transport,
            mounted,
            labels,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Accepts only whole-disk node names of the kinds a USB stick / SD card
/// can have. Rejects partitions (`/dev/sdb1`), loop and device-mapper nodes,
/// relative paths and anything with `..`.
pub fn is_valid_device_path(p: &str) -> bool {
    let Some(name) = p.strip_prefix("/dev/") else { return false };
    if name.is_empty() || name.contains('/') || name.contains("..") {
        return false;
    }
    let b = name.as_bytes();
    let all = |f: &dyn Fn(u8) -> bool, x: &[u8]| !x.is_empty() && x.iter().all(|c| f(*c));
    let alpha = |c: u8| c.is_ascii_lowercase();
    let digit = |c: u8| c.is_ascii_digit();
    // sda, sdb, sdaa — vda — xvda
    for pre in ["sd", "vd", "xvd", "hd"] {
        if let Some(rest) = name.strip_prefix(pre) {
            return all(&alpha, rest.as_bytes());
        }
    }
    // mmcblk0, nvme0n1 (no trailing p<N>)
    if let Some(rest) = name.strip_prefix("mmcblk") {
        return all(&digit, rest.as_bytes());
    }
    if let Some(rest) = name.strip_prefix("nvme") {
        let mut it = rest.splitn(2, 'n');
        let (a, c) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
        return all(&digit, a.as_bytes()) && all(&digit, c.as_bytes());
    }
    let _ = b;
    false
}

/// `/dev/sdb` + 1 → `/dev/sdb1`; `/dev/mmcblk0` + 1 → `/dev/mmcblk0p1`.
pub fn partition_path(dev: &str, n: u32) -> String {
    let last_is_digit = dev.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false);
    if last_is_digit { format!("{dev}p{n}") } else { format!("{dev}{n}") }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    None,
    Xz,
    Gzip,
    Zstd,
    Bzip2,
}

impl Compression {
    /// Program that decompresses to stdout, with its arguments.
    pub fn decompressor(self) -> Option<(&'static str, &'static [&'static str])> {
        match self {
            Compression::None => None,
            Compression::Xz => Some(("xz", &["-dc", "--"])),
            Compression::Gzip => Some(("gzip", &["-dc", "--"])),
            Compression::Zstd => Some(("zstd", &["-dc", "--"])),
            Compression::Bzip2 => Some(("bzip2", &["-dc", "--"])),
        }
    }
}

pub fn detect_compression(path: &str) -> Compression {
    let l = path.to_ascii_lowercase();
    if l.ends_with(".xz") {
        Compression::Xz
    } else if l.ends_with(".gz") {
        Compression::Gzip
    } else if l.ends_with(".zst") || l.ends_with(".zstd") {
        Compression::Zstd
    } else if l.ends_with(".bz2") {
        Compression::Bzip2
    } else {
        Compression::None
    }
}

/// File extensions the writer accepts (after stripping one compression suffix).
pub fn is_supported_image(path: &str) -> bool {
    let mut l = path.to_ascii_lowercase();
    for suf in [".xz", ".gz", ".zst", ".zstd", ".bz2"] {
        if let Some(x) = l.strip_suffix(suf) {
            l = x.to_string();
            break;
        }
    }
    [".iso", ".img", ".raw", ".bin", ".dd", ".usb"].iter().any(|e| l.ends_with(e))
}

/// Uncompressed size from the trailing ISIZE field of a gzip file (exact for
/// images < 4 GiB, wraps otherwise — callers must treat it as a hint).
pub fn gzip_isize(path: &Path) -> Option<u64> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    if len < 18 {
        return None;
    }
    f.seek(SeekFrom::End(-4)).ok()?;
    let mut b = [0u8; 4];
    f.read_exact(&mut b).ok()?;
    Some(u32::from_le_bytes(b) as u64)
}

/// Parses the `totals` line of `xz --robot --list`; the 5th field is the
/// uncompressed size in bytes.
pub fn parse_xz_robot_totals(out: &str) -> Option<u64> {
    for line in out.lines() {
        if line.starts_with("totals") {
            let f: Vec<&str> = line.split('\t').collect();
            return f.get(4).and_then(|x| x.trim().parse().ok());
        }
    }
    None
}

/// Size of the image once unpacked, when it can be learned cheaply.
pub fn uncompressed_size(path: &str, comp: Compression) -> Option<u64> {
    match comp {
        Compression::None => std::fs::metadata(path).ok().map(|m| m.len()),
        Compression::Gzip => gzip_isize(Path::new(path)),
        Compression::Xz => {
            let o = Command::new("xz").args(["--robot", "--list", "--", path]).output().ok()?;
            parse_xz_robot_totals(&String::from_utf8_lossy(&o.stdout))
        }
        _ => None,
    }
}

/// Normalises a user-supplied SHA-256 (accepts `sha256sum` lines, upper case,
/// surrounding whitespace). `None` when it is not a 64-hex-digit digest.
pub fn normalise_sha256(input: &str) -> Option<String> {
    let first = input.split_whitespace().next()?.trim_start_matches("sha256:");
    let first = first.trim_start_matches('*');
    if first.len() == 64 && first.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(first.to_ascii_lowercase())
    } else {
        None
    }
}

pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Human readable size used in error messages.
pub fn human_bytes(n: u64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{n} B") } else { format!("{v:.1} {}", U[i]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "blockdevices": [
        {"name":"nvme0n1","path":"/dev/nvme0n1","size":"512110190592","rm":false,"hotplug":false,"tran":"nvme","model":"Samsung","vendor":null,"type":"disk","ro":false,
         "mountpoints":[null],"label":null,
         "children":[{"name":"nvme0n1p1","type":"part","mountpoints":["/boot/efi"],"label":null},
                     {"name":"nvme0n1p2","type":"part","mountpoints":["/"],"label":null}]},
        {"name":"sdb","path":"/dev/sdb","size":"15518924800","rm":true,"hotplug":true,"tran":"usb","model":"Ultra ","vendor":"SanDisk ","type":"disk","ro":false,
         "mountpoints":[null],"label":null,
         "children":[{"name":"sdb1","type":"part","mountpoints":["/run/media/u/STICK"],"label":"STICK"}]},
        {"name":"sdc","path":"/dev/sdc","size":"0","rm":true,"hotplug":true,"tran":"usb","model":"Reader","vendor":"Generic","type":"disk","ro":false,"mountpoints":[null]},
        {"name":"sdd","path":"/dev/sdd","size":"8000000000","rm":true,"hotplug":true,"tran":"usb","model":"Locked","vendor":"X","type":"disk","ro":true,"mountpoints":[null]},
        {"name":"sde","path":"/dev/sde","size":"2000398934016","rm":false,"hotplug":false,"tran":"sata","model":"Data","vendor":"ATA","type":"disk","ro":false,"mountpoints":[null]},
        {"name":"loop0","path":"/dev/loop0","size":"100000","rm":false,"hotplug":false,"tran":null,"model":null,"vendor":null,"type":"loop","ro":false,"mountpoints":[null]},
        {"name":"sdf","path":"/dev/sdf","size":"31000000000","rm":true,"hotplug":true,"tran":"usb","model":"Live","vendor":"Y","type":"disk","ro":false,
         "mountpoints":[null],"children":[{"name":"sdf1","type":"part","mountpoints":["/run/live/medium"],"label":"LIVE"}]}
      ]}"#;

    #[test]
    fn offers_only_the_usb_stick() {
        let d = parse_lsblk(SAMPLE, DEFAULT_MAX_TARGET_BYTES).unwrap();
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].path, "/dev/sdb");
        assert_eq!(d[0].vendor, "SanDisk");
        assert_eq!(d[0].model, "Ultra");
        assert_eq!(d[0].mounted, vec!["/run/media/u/STICK"]);
        assert_eq!(d[0].labels, vec!["STICK"]);
    }

    #[test]
    fn never_offers_system_or_live_medium_or_internal_disks() {
        let d = parse_lsblk(SAMPLE, DEFAULT_MAX_TARGET_BYTES).unwrap();
        for bad in ["/dev/nvme0n1", "/dev/sde", "/dev/sdf", "/dev/sdc", "/dev/sdd", "/dev/loop0"] {
            assert!(!d.iter().any(|x| x.path == bad), "{bad} must not be offered");
        }
    }

    #[test]
    fn size_cap_is_respected() {
        let d = parse_lsblk(SAMPLE, 1_000_000).unwrap();
        assert!(d.is_empty());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_lsblk("not json", 1).is_err());
        assert!(parse_lsblk("{}", 1).is_err());
    }

    #[test]
    fn device_path_validation() {
        for ok in ["/dev/sdb", "/dev/sdaa", "/dev/vdb", "/dev/mmcblk0", "/dev/nvme1n1"] {
            assert!(is_valid_device_path(ok), "{ok}");
        }
        for bad in [
            "/dev/sdb1", "/dev/mmcblk0p1", "/dev/nvme0n1p2", "/dev/loop0", "/dev/dm-0", "/dev/",
            "/dev/sd", "sdb", "/dev/../etc/passwd", "/dev/sd/b", "/etc/passwd", "/dev/sdb ;rm", "/dev/mapper/x",
        ] {
            assert!(!is_valid_device_path(bad), "{bad}");
        }
    }

    #[test]
    fn partition_naming() {
        assert_eq!(partition_path("/dev/sdb", 1), "/dev/sdb1");
        assert_eq!(partition_path("/dev/mmcblk0", 1), "/dev/mmcblk0p1");
        assert_eq!(partition_path("/dev/nvme0n1", 2), "/dev/nvme0n1p2");
    }

    #[test]
    fn compression_and_image_kinds() {
        assert_eq!(detect_compression("a.iso"), Compression::None);
        assert_eq!(detect_compression("A.IMG.XZ"), Compression::Xz);
        assert_eq!(detect_compression("a.raw.gz"), Compression::Gzip);
        assert_eq!(detect_compression("a.img.zst"), Compression::Zstd);
        assert!(is_supported_image("fedora.iso"));
        assert!(is_supported_image("raspios.img.xz"));
        assert!(!is_supported_image("notes.txt"));
        assert!(!is_supported_image("archive.tar.gz"));
    }

    #[test]
    fn xz_totals() {
        let out = "name\tfoo.xz\t1\t1\t100\t5000\t0.020\tCRC64\t0\ntotals\t1\t1\t100\t5000\t0.020\tCRC64\t0\t0\n";
        assert_eq!(parse_xz_robot_totals(out), Some(5000));
        assert_eq!(parse_xz_robot_totals("nothing"), None);
    }

    #[test]
    fn sha256_normalisation() {
        let h = "A".repeat(64);
        assert_eq!(normalise_sha256(&format!("{h}  file.iso")), Some("a".repeat(64)));
        assert_eq!(normalise_sha256(&format!("sha256:{h}")), Some("a".repeat(64)));
        assert_eq!(normalise_sha256("abc"), None);
        assert_eq!(normalise_sha256(&"g".repeat(64)), None);
    }

    #[test]
    fn sizes() {
        assert_eq!(human_bytes(10), "10 B");
        assert_eq!(human_bytes(1536), "1.5 KiB");
        assert_eq!(hex(&[0, 255, 16]), "00ff10");
    }
}

/// Finds the SHA-256 of `filename` in the text of a `sha256sums` file
/// (`<hash>  <name>`, `<hash> *<name>`, BSD `SHA256 (name) = <hash>`), or the
/// only digest in a single-hash file.
pub fn find_checksum(text: &str, filename: &str) -> Option<String> {
    let mut lone: Vec<String> = Vec::new();
    for raw in text.lines() {
        let l = raw.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("-----") {
            continue;
        }
        // BSD style
        if let Some(rest) = l.strip_prefix("SHA256 (") {
            if let Some((name, hash)) = rest.split_once(") = ") {
                if name.trim() == filename {
                    return normalise_sha256(hash);
                }
            }
            continue;
        }
        let mut it = l.split_whitespace();
        let (Some(h), name) = (it.next(), it.next()) else { continue };
        let Some(hash) = normalise_sha256(h) else { continue };
        match name {
            Some(n) if n.trim_start_matches('*') == filename => return Some(hash),
            Some(_) => {}
            None => lone.push(hash),
        }
    }
    if lone.len() == 1 { lone.pop() } else { None }
}

#[cfg(test)]
mod checksum_tests {
    use super::*;

    #[test]
    fn finds_in_sums_file() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let txt = format!("# comment\n{a}  one.iso\n{b} *two.iso\n");
        assert_eq!(find_checksum(&txt, "two.iso"), Some(b.clone()));
        assert_eq!(find_checksum(&txt, "one.iso"), Some(a));
        assert_eq!(find_checksum(&txt, "three.iso"), None);
    }

    #[test]
    fn bsd_and_single_hash() {
        let a = "c".repeat(64);
        assert_eq!(find_checksum(&format!("SHA256 (x.iso) = {a}\n"), "x.iso"), Some(a.clone()));
        assert_eq!(find_checksum(&format!("{a}\n"), "anything.iso"), Some(a));
    }
}

/// Makes a file name coming from a URL / HTTP header safe to create in our
/// cache directory: keeps `[A-Za-z0-9._-]`, never starts with a dot, never
/// contains a path separator, capped at 120 characters.
pub fn sanitize_filename(raw: &str) -> String {
    let last = raw.split(['/', '\\']).filter(|p| !p.is_empty()).last().unwrap_or("");
    let last = last.split(['?', '#']).next().unwrap_or("");
    let cleaned: String = last
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' })
        .collect();
    let cleaned = cleaned.trim_start_matches('.').to_string();
    let cleaned: String = cleaned.chars().take(120).collect();
    if cleaned.is_empty() { "image.img".to_string() } else { cleaned }
}

#[cfg(test)]
mod name_tests {
    use super::*;

    #[test]
    fn sanitises() {
        assert_eq!(sanitize_filename("https://x.org/a/b/arch linux.iso?x=1"), "arch_linux.iso");
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("https://x.org/"), "x.org");
        assert_eq!(sanitize_filename(""), "image.img");
        assert_eq!(sanitize_filename(".hidden.iso"), "hidden.iso");
        assert!(sanitize_filename(&"a".repeat(300)).len() <= 120);
    }
}
