use super::media::*;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const CHUNK: usize = 4 * 1024 * 1024;
const SYNC_EVERY: u64 = 64 * 1024 * 1024;
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// `write` | `restore`
    pub op: String,
    pub device: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub verify: bool,
    /// When this file appears the operation stops as soon as possible.
    #[serde(default)]
    pub cancel_file: Option<String>,
}

fn line(v: serde_json::Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}{}", crate::privileged::LINE_PREFIX, v);
    let _ = out.flush();
}

fn progress(phase: &str, done: u64, total: Option<u64>, speed: f64) {
    line(json!({"event": "progress", "phase": phase, "done": done, "total": total, "speed": speed}));
}

fn fail(msg: impl Into<String>) -> i32 {
    line(json!({"event": "error", "message": msg.into()}));
    1
}

fn cancelled(req: &Request) -> bool {
    req.cancel_file.as_deref().map(|p| Path::new(p).exists()).unwrap_or(false)
}

/// Entry point of `blue-environment --media-writer-priv`.
pub fn run_priv_helper() -> i32 {
    // Survive the GUI going away mid-write: a half-written stick is worse than
    // a finished one. (The cancel file is the supported way to stop.)
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return fail("Could not read the request");
    }
    let req: Request = match serde_json::from_str(input.trim()) {
        Ok(r) => r,
        Err(e) => return fail(format!("Malformed request: {e}")),
    };
    let drive = match validate_target(&req.device) {
        Ok(d) => d,
        Err(e) => return fail(e),
    };
    if let Err(e) = unmount_all(&drive) {
        return fail(e);
    }
    let result = match req.op.as_str() {
        "write" => write_image(&req, &drive),
        "restore" => restore_drive(&drive),
        other => Err(format!("Unknown operation \"{other}\"")),
    };
    match result {
        Ok(sha) => {
            line(json!({"event": "done", "sha256": sha}));
            0
        }
        Err(e) => fail(e),
    }
}

/// Rules 1–3 of the module doc.
pub fn validate_target(device: &str) -> Result<DriveInfo, String> {
    if !is_valid_device_path(device) {
        return Err(format!("\"{device}\" is not a whole-disk device"));
    }
    let canon = std::fs::canonicalize(device).map_err(|e| format!("Cannot resolve {device}: {e}"))?;
    if canon != Path::new(device) {
        return Err(format!("{device} is a link to {} — refusing", canon.display()));
    }
    let meta = std::fs::metadata(device).map_err(|e| format!("Cannot stat {device}: {e}"))?;
    if !meta.file_type().is_block_device() {
        return Err(format!("{device} is not a block device"));
    }
    let out = Command::new("lsblk")
        .args(["-J", "-b", "-o", "NAME,PATH,SIZE,RM,HOTPLUG,TRAN,MODEL,VENDOR,TYPE,RO,MOUNTPOINTS,LABEL", device])
        .output()
        .map_err(|e| format!("lsblk failed: {e}"))?;
    let drives = parse_lsblk(&String::from_utf8_lossy(&out.stdout), DEFAULT_MAX_TARGET_BYTES)?;
    drives
        .into_iter()
        .find(|d| d.path == device)
        .ok_or_else(|| format!("{device} is not a removable drive that is safe to overwrite (system disks, read-only and internal disks are never written)"))
}

fn unmount_all(d: &DriveInfo) -> Result<(), String> {
    for mp in &d.mounted {
        let st = Command::new("umount")
            .arg("--")
            .arg(mp)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("umount failed: {e}"))?;
        if !st.status.success() {
            return Err(format!(
                "Could not unmount {mp}: {}. Close any program using the drive and try again.",
                String::from_utf8_lossy(&st.stderr).trim()
            ));
        }
    }
    Ok(())
}

enum Source {
    File(File),
    Pipe(Child),
}

fn open_source(path: &str, comp: Compression) -> Result<Source, String> {
    match comp.decompressor() {
        None => File::open(path).map(Source::File).map_err(|e| format!("Cannot open the image: {e}")),
        Some((prog, args)) => {
            let child = Command::new(prog)
                .args(args)
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Cannot start `{prog}` to unpack the image (is it installed?): {e}"))?;
            Ok(Source::Pipe(child))
        }
    }
}

fn read_full(r: &mut dyn Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

fn write_image(req: &Request, drive: &DriveInfo) -> Result<String, String> {
    let image = req.image.as_deref().ok_or("No image was chosen")?;
    if !Path::new(image).is_file() {
        return Err(format!("The image file {image} does not exist"));
    }
    if !is_supported_image(image) {
        return Err("Unsupported image type (expected .iso, .img, .raw — optionally .xz/.gz/.zst/.bz2)".into());
    }
    let comp = detect_compression(image);
    let total = uncompressed_size(image, comp);
    if let Some(t) = total {
        if t > drive.size_bytes {
            return Err(format!(
                "The image ({}) is larger than the drive ({}). Choose a bigger drive.",
                human_bytes(t),
                human_bytes(drive.size_bytes)
            ));
        }
    }

    let mut dev = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_EXCL)
        .open(&drive.path)
        .map_err(|e| format!("Cannot open {} for writing (is something still using it?): {e}", drive.path))?;

    let mut source = open_source(image, comp)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let (mut written, mut since_sync, mut window_bytes) = (0u64, 0u64, 0u64);
    let mut window = Instant::now();
    let mut speed = 0.0f64;
    progress("write", 0, total, 0.0);

    loop {
        if cancelled(req) {
            kill_source(&mut source);
            return Err("Cancelled".into());
        }
        let n = match &mut source {
            Source::File(f) => read_full(f, &mut buf),
            Source::Pipe(c) => read_full(c.stdout.as_mut().ok_or("no pipe")?, &mut buf),
        }
        .map_err(|e| format!("Reading the image failed: {e}"))?;
        if n == 0 {
            break;
        }
        dev.write_all(&buf[..n]).map_err(|e| {
            if e.raw_os_error() == Some(libc::ENOSPC) {
                "The image does not fit on this drive.".to_string()
            } else {
                format!("Writing to {} failed: {e}", drive.path)
            }
        })?;
        hasher.update(&buf[..n]);
        written += n as u64;
        since_sync += n as u64;
        window_bytes += n as u64;
        if since_sync >= SYNC_EVERY {
            dev.sync_data().map_err(|e| format!("Flushing to the drive failed: {e}"))?;
            since_sync = 0;
        }
        if window.elapsed() >= PROGRESS_EVERY {
            speed = window_bytes as f64 / window.elapsed().as_secs_f64().max(0.001);
            progress("write", written, total, speed);
            window = Instant::now();
            window_bytes = 0;
        }
    }
    if let Source::Pipe(c) = &mut source {
        let st = c.wait().map_err(|e| e.to_string())?;
        if !st.success() {
            return Err("Unpacking the image failed — the file is probably corrupted".into());
        }
    }
    progress("flush", written, Some(written), speed);
    dev.sync_all().map_err(|e| format!("Flushing to the drive failed: {e}"))?;
    drop(dev);
    let digest = hex(&hasher.finalize());

    if req.verify {
        verify_device(req, &drive.path, written, &digest)?;
    }
    let _ = Command::new("blockdev").args(["--rereadpt", &drive.path]).output();
    let _ = Command::new("udevadm").arg("settle").output();
    progress("done", written, Some(written), 0.0);
    Ok(digest)
}

fn kill_source(s: &mut Source) {
    if let Source::Pipe(c) = s {
        let _ = c.kill();
        let _ = c.wait();
    }
}

/// Reads `len` bytes back from the device and compares their SHA-256 with what
/// was written.
fn verify_device(req: &Request, path: &str, len: u64, expected: &str) -> Result<(), String> {
    let mut f = File::open(path).map_err(|e| format!("Cannot re-open {path} for verification: {e}"))?;
    let mut buf = vec![0u8; CHUNK];
    let mut h = Sha256::new();
    let (mut done, mut win_bytes) = (0u64, 0u64);
    let mut win = Instant::now();
    while done < len {
        if cancelled(req) {
            return Err("Cancelled".into());
        }
        let want = ((len - done) as usize).min(CHUNK);
        let n = read_full(&mut f, &mut buf[..want]).map_err(|e| format!("Reading back failed: {e}"))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        done += n as u64;
        win_bytes += n as u64;
        if win.elapsed() >= PROGRESS_EVERY {
            progress("verify", done, Some(len), win_bytes as f64 / win.elapsed().as_secs_f64().max(0.001));
            win = Instant::now();
            win_bytes = 0;
        }
    }
    if done != len || hex(&h.finalize()) != expected {
        return Err("Verification failed: the data on the drive differs from the image. The drive may be faulty — try another one.".into());
    }
    Ok(())
}

/// Wipes the drive and makes one FAT32 partition spanning it — turns a
/// "Live USB" back into an ordinary stick.
fn restore_drive(drive: &DriveInfo) -> Result<String, String> {
    let run = |prog: &str, args: &[&str]| -> Result<(), String> {
        let o = Command::new(prog)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("Cannot run `{prog}` (is it installed?): {e}"))?;
        if o.status.success() {
            Ok(())
        } else {
            Err(format!("`{prog}` failed: {}", String::from_utf8_lossy(&o.stderr).trim()))
        }
    };
    progress("restore", 0, Some(4), 0.0);
    run("wipefs", &["-a", &drive.path])?;
    progress("restore", 1, Some(4), 0.0);
    run("parted", &["-s", &drive.path, "mklabel", "msdos", "mkpart", "primary", "fat32", "1MiB", "100%"])?;
    progress("restore", 2, Some(4), 0.0);
    let _ = Command::new("udevadm").arg("settle").output();
    let part = partition_path(&drive.path, 1);
    for _ in 0..20 {
        if Path::new(&part).exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    progress("restore", 3, Some(4), 0.0);
    run("mkfs.vfat", &["-F", "32", "-n", "USB_DRIVE", &part])?;
    progress("done", 4, Some(4), 0.0);
    Ok(String::new())
}
