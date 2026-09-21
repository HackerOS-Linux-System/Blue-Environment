use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A command that dies within this window with a non-zero status is
/// reported as a failed launch. (Long-running apps that crash later are
/// not "launch failures".)
const FAIL_WINDOW: Duration = Duration::from_secs(15);

fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })))
}

/// First Wayland socket in the runtime dir (`wayland-0`, `wayland-1`, …).
fn find_wayland_socket() -> Option<String> {
    let mut names: Vec<String> = std::fs::read_dir(runtime_dir())
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("wayland-") && !n.ends_with(".lock"))
        .collect();
    names.sort();
    names.into_iter().next()
}

/// The environment a freshly launched app should see.
pub fn session_env() -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = Vec::new();
    let rt = runtime_dir();
    if std::env::var_os("XDG_RUNTIME_DIR").is_none() {
        env.push(("XDG_RUNTIME_DIR".into(), rt.display().to_string()));
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        if let Some(sock) = find_wayland_socket() {
            env.push(("WAYLAND_DISPLAY".into(), sock));
        }
    }
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        let bus = rt.join("bus");
        if bus.exists() {
            env.push(("DBUS_SESSION_BUS_ADDRESS".into(), format!("unix:path={}", bus.display())));
        }
    }
    env.push(("XDG_SESSION_TYPE".into(), "wayland".into()));
    if std::env::var_os("XDG_CURRENT_DESKTOP").is_none() {
        env.push(("XDG_CURRENT_DESKTOP".into(), "labwc:Blue".into()));
    }
    env
}

fn log_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".cache"))
        .join("Blue-Environment")
        .join("launch.log")
}

fn append_log(line: &str) {
    let path = log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

fn report_failure(command: &str, detail: &str) {
    append_log(&format!("[launch failed] {command}: {detail}"));
    super::emit("shell:launch-failed", serde_json::json!({ "command": command, "detail": detail }));
}

/// Launches `command` (a desktop-entry `Exec` line, already stripped of
/// `%f`/`%u` codes) in the background. Never blocks the caller.
pub fn launch(command: String) {
    let _ = std::thread::Builder::new().name("blue-launch".into()).spawn(move || {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&command);
        for (k, v) in session_env() {
            cmd.env(k, v);
        }
        // Our own marker must not leak into apps (a nested Blue Environment
        // would think it is the labwc child).
        cmd.env_remove(super::ENV_SESSION);
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
        // Own session: survives a shell restart, no shared controlling tty.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        append_log(&format!("[launch] {command}"));
        let started = Instant::now();
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return report_failure(&command, &format!("cannot start /bin/sh: {e}")),
        };

        // Drain stderr for the whole life of the app (never let the pipe fill up).
        let tail = std::sync::Arc::new(std::sync::Mutex::new(VecDeque::<String>::new()));
        if let Some(stderr) = child.stderr.take() {
            let tail = tail.clone();
            let cmd_name = command.clone();
            let _ = std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    append_log(&format!("[{cmd_name}] {line}"));
                    let mut t = tail.lock().unwrap();
                    t.push_back(line);
                    if t.len() > 6 {
                        t.pop_front();
                    }
                }
            });
        }

        match child.wait() {
            Ok(status) if !status.success() && started.elapsed() < FAIL_WINDOW => {
                // Give the drain thread a moment to read the last lines.
                std::thread::sleep(Duration::from_millis(150));
                let lines: Vec<String> = tail.lock().unwrap().iter().cloned().collect();
                let code = match (status.code(), status.signal_number()) {
                    (Some(c), _) => format!("exit code {c}"),
                    (None, Some(s)) => format!("killed by signal {s}"),
                    _ => "failed".into(),
                };
                let detail = if lines.is_empty() { code } else { format!("{code}\n{}", lines.join("\n")) };
                report_failure(&command, &detail);
            }
            _ => {}
        }
    });
}

trait SignalNumber {
    fn signal_number(&self) -> Option<i32>;
}
impl SignalNumber for std::process::ExitStatus {
    fn signal_number(&self) -> Option<i32> {
        std::os::unix::process::ExitStatusExt::signal(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn failing_command_is_reported_with_stderr() {
        let got: Arc<Mutex<Vec<(String, serde_json::Value)>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = got.clone();
        super::super::set_event_sink(Arc::new(move |n, p| sink.lock().unwrap().push((n.to_string(), p))));
        launch("echo boom >&2; exit 3".into());
        for _ in 0..50 {
            if !got.lock().unwrap().is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let events = got.lock().unwrap();
        let (name, payload) = events.first().expect("launch-failed event");
        assert_eq!(name, "shell:launch-failed");
        let detail = payload["detail"].as_str().unwrap();
        assert!(detail.contains("exit code 3") && detail.contains("boom"), "{detail}");
    }

    #[test]
    fn session_env_sets_wayland_session_type() {
        assert!(session_env().iter().any(|(k, v)| k == "XDG_SESSION_TYPE" && v == "wayland"));
    }
}
