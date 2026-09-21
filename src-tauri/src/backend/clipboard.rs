use base64::Engine;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static STARTED: AtomicBool = AtomicBool::new(false);

/// Texts above this size are ignored (huge pastes would bloat history).
const MAX_BYTES: usize = 256 * 1024;

pub fn is_available() -> bool {
    super::find_in_path("wl-paste").is_some()
}

/// Starts the watcher (idempotent). `on_text` is called for every new,
/// non-empty text selection.
pub fn start_watcher<F>(on_text: F)
where
    F: Fn(String) + Send + 'static,
{
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    if !is_available() {
        eprintln!("[blue-clipboard] wl-paste not found — install wl-clipboard for system-wide clipboard history");
        STARTED.store(false, Ordering::SeqCst);
        return;
    }
    let _ = std::thread::Builder::new().name("blue-clipboard".into()).spawn(move || {
        let mut failures = 0u32;
        loop {
            // The per-change command base64-encodes stdin onto one line, so
            // multi-line clipboard contents survive the line-based pipe.
            let child = Command::new("wl-paste")
                .args(["--type", "text", "--watch", "sh", "-c", "base64 -w0; echo"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .spawn();
            let mut child = match child {
                Ok(c) => c,
                Err(_) => {
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };
            let started = std::time::Instant::now();
            if let Some(out) = child.stdout.take() {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if let Some(text) = decode(&line) {
                        on_text(text);
                    }
                }
            }
            let _ = child.wait();
            // Exited quickly = compositor lacks data-control / no display;
            // back off instead of spinning.
            if started.elapsed() < Duration::from_secs(3) {
                failures += 1;
            } else {
                failures = 0;
            }
            std::thread::sleep(Duration::from_secs((2 * failures.min(15)).max(2) as u64));
        }
    });
}

fn decode(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(line).ok()?;
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_multiline_text() {
        let enc = base64::engine::general_purpose::STANDARD.encode("line1\nline2");
        assert_eq!(decode(&enc).as_deref(), Some("line1\nline2"));
        assert_eq!(decode(""), None);
        assert_eq!(decode(&base64::engine::general_purpose::STANDARD.encode("   \n")), None);
    }
}
