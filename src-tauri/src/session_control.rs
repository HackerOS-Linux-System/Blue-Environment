use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Wyciąga ID sesji logind: najpierw `XDG_SESSION_ID`, potem
/// `loginctl show-user <uid> -p Display --value`.
pub fn session_id() -> Option<String> {
    if let Ok(id) = std::env::var("XDG_SESSION_ID") {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return Some(id);
        }
    }
    let uid = unsafe { libc::getuid() }.to_string();
    let out = Command::new("loginctl")
        .args(["show-user", &uid, "-p", "Display", "--value"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    parse_session_id(&String::from_utf8_lossy(&out.stdout))
}

fn parse_session_id(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() || s.contains(char::is_whitespace) {
        None
    } else {
        Some(s.to_string())
    }
}

fn pid_alive(pid: i32) -> bool {
    // kill(pid, 0) sprawdza istnienie procesu; EPERM też znaczy "żyje".
    unsafe { libc::kill(pid, 0) == 0 || *libc::__errno_location() == libc::EPERM }
}

fn wait_gone(pid: i32, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !pid_alive(pid)
}

fn loginctl(args: &[&str]) -> bool {
    Command::new("loginctl")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Pełne wylogowanie. Wywoływane z wątku — nigdy nie wraca (kończy proces).
pub fn logout() -> ! {
    // 1) Łagodne zamknięcie.
    crate::backend::shell_ipc::cleanup();
    let comp_pid = crate::backend::compositor_pid();
    crate::backend::exit_compositor();
    if let Some(pid) = comp_pid {
        if !wait_gone(pid, Duration::from_secs(3)) {
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    }

    // 2) logind kończy cały scope sesji → SDDM wraca do greetera.
    let ok = match session_id() {
        Some(id) => loginctl(&["terminate-session", &id]),
        None => false,
    };
    if !ok {
        let uid = unsafe { libc::getuid() }.to_string();
        // terminate-user zabija też inne sesje tego użytkownika — dlatego
        // tylko jako fallback, gdy ID sesji jest nieznane.
        let _ = loginctl(&["terminate-user", &uid]);
    }

    // 3) Jeśli logind nas jeszcze nie zabił, kończymy sami; reszta procesów
    //    sesji została już zakończona w kroku 2.
    std::thread::sleep(Duration::from_millis(500));
    std::process::exit(0);
}

/// Uruchamia wylogowanie w osobnym wątku (komenda Tauri nie może blokować).
pub fn logout_async() {
    std::thread::spawn(|| logout());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_id() {
        assert_eq!(parse_session_id("3\n"), Some("3".into()));
        assert_eq!(parse_session_id("c2\n"), Some("c2".into()));
        assert_eq!(parse_session_id("\n"), None);
        assert_eq!(parse_session_id("a b"), None);
    }

    #[test]
    fn own_pid_is_alive() {
        assert!(pid_alive(std::process::id() as i32));
    }
}
