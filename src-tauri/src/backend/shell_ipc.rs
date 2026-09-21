use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

/// Commands accepted over the socket. Anything else is rejected.
pub const COMMANDS: &[&str] = &[
    "toggle-start-menu",
    "fullscreen-menu",
    "toggle-control-center",
    "toggle-notifications",
    "toggle-clipboard",
    "open-terminal",
    "screenshot",
    "lock",
    "show-desktop",
    "close-panels",
    "restart-shell",
    "open-app",
    "ping",
];

pub fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    PathBuf::from(runtime).join("blue-environment.sock")
}

/// Splits a request line into (command, optional argument) and validates it.
pub fn parse(line: &str) -> Option<(String, Option<String>)> {
    let line = line.trim();
    let (cmd, arg) = match line.split_once(' ') {
        Some((c, a)) => (c, Some(a.trim().to_string()).filter(|a| !a.is_empty())),
        None => (line, None),
    };
    COMMANDS.contains(&cmd).then(|| (cmd.to_string(), arg))
}

/// Starts the server thread. `on_command` runs for every valid command
/// (except `ping`, which is answered internally).
pub fn start_server<F>(on_command: F)
where
    F: Fn(String, Option<String>) + Send + Sync + 'static,
{
    let path = socket_path();
    // Stale socket from a crashed shell? Only remove it if nobody answers.
    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            eprintln!("[blue-ipc] another Blue shell already owns {}", path.display());
            return;
        }
        let _ = std::fs::remove_file(&path);
    }
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[blue-ipc] cannot bind {}: {e}", path.display());
            return;
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    let on_command = std::sync::Arc::new(on_command);
    let _ = std::thread::Builder::new().name("blue-ipc".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            let handler = on_command.clone();
            let _ = std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut writer = match stream.try_clone() {
                    Ok(w) => w,
                    Err(_) => return,
                };
                let mut line = String::new();
                if BufReader::new(stream).read_line(&mut line).is_err() {
                    return;
                }
                match parse(&line) {
                    Some((cmd, _)) if cmd == "ping" => {
                        let _ = writeln!(writer, "ok");
                    }
                    Some((cmd, arg)) => {
                        handler(cmd, arg);
                        let _ = writeln!(writer, "ok");
                    }
                    None => {
                        let _ = writeln!(writer, "error: unknown command");
                    }
                }
            });
        }
    });
}

pub fn cleanup() {
    let _ = std::fs::remove_file(socket_path());
}

/// CLI client: `blue-environment --ctl <command> [arg]`. Returns the
/// process exit code.
pub fn run_cli(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("usage: blue-environment --ctl <command> [argument]\ncommands: {}", COMMANDS.join(", "));
        return 2;
    }
    let request = args.join(" ");
    if parse(&request).is_none() {
        eprintln!("blue-environment --ctl: unknown command '{}'\ncommands: {}", args[0], COMMANDS.join(", "));
        return 2;
    }
    let path = socket_path();
    let mut stream = match UnixStream::connect(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("blue-environment --ctl: Blue shell is not running ({}: {e})", path.display());
            return 1;
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    if writeln!(stream, "{request}").is_err() {
        return 1;
    }
    let mut reply = String::new();
    let _ = BufReader::new(stream).read_line(&mut reply);
    if reply.trim() == "ok" {
        0
    } else {
        eprintln!("blue-environment --ctl: {}", reply.trim());
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_validates() {
        assert_eq!(parse("toggle-start-menu\n"), Some(("toggle-start-menu".into(), None)));
        assert_eq!(parse("open-app firefox"), Some(("open-app".into(), Some("firefox".into()))));
        assert_eq!(parse("rm -rf /"), None);
        assert_eq!(parse(""), None);
    }
}
