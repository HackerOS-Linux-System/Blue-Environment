use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

/// Prefix of protocol lines on the privileged child's stdout.
pub const LINE_PREFIX: &str = "BLUE> ";

#[derive(Debug, Clone)]
pub struct PrivError {
    pub message: String,
    pub hint: Option<String>,
    pub detail: Option<String>,
}

impl PrivError {
    fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), hint: None, detail: None }
    }
    fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = Some(h.into());
        self
    }
    fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }
}

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

fn sudo_is_passwordless() -> bool {
    Command::new("sudo")
        .args(["-n", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p.join(name).is_file()))
        .unwrap_or(false)
}

/// Builds the command that runs `blue-environment <args…>` as root.
pub fn root_command(args: &[&str]) -> Result<Command, PrivError> {
    let exe = std::env::current_exe().map_err(|e| PrivError::new("Cannot locate the Blue Environment binary").detail(e.to_string()))?;
    if is_root() {
        let mut c = Command::new(exe);
        c.args(args);
        return Ok(c);
    }
    if command_exists("sudo") && sudo_is_passwordless() {
        let mut c = Command::new("sudo");
        c.args(["-n", "--"]).arg(exe).args(args);
        return Ok(c);
    }
    if command_exists("pkexec") {
        let mut c = Command::new("pkexec");
        c.arg(exe).args(args);
        return Ok(c);
    }
    Err(PrivError::new("Administrator rights are required but no way to obtain them was found")
        .hint("Install polkit (pkexec) or run the operation from a terminal with sudo, e.g. `sudo blue apps install <blue.hk URL>`."))
}

pub struct PrivOutcome {
    pub success: bool,
    pub exit_code: Option<i32>,
    /// Last lines of stderr (useful when pkexec/sudo itself failed).
    pub stderr_tail: String,
}

/// Runs the privileged sub-command, feeding `input` (one JSON document) to
/// its stdin and calling `on_line` for every protocol line (prefix removed)
/// and `on_log` for every other stdout line.
pub fn run_privileged(
    args: &[&str],
    input: &str,
    mut on_line: impl FnMut(&str),
    mut on_log: impl FnMut(&str),
) -> Result<PrivOutcome, PrivError> {
    let mut cmd = root_command(args)?;
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PrivError::new("Could not start the privileged helper").detail(e.to_string()))?;

    if let Some(mut stdin) = child.stdin.take() {
        // The request (which for the installer contains the new user's
        // password) travels over a pipe — never on a command line.
        let _ = stdin.write_all(input.as_bytes());
        let _ = stdin.write_all(b"\n");
    }

    // Drain stderr on a helper thread so a chatty child can't block on a full pipe.
    let stderr = child.stderr.take();
    let err_thread = std::thread::spawn(move || {
        let mut lines: Vec<String> = Vec::new();
        if let Some(s) = stderr {
            for l in BufReader::new(s).lines().map_while(Result::ok) {
                lines.push(l);
                if lines.len() > 60 {
                    lines.remove(0);
                }
            }
        }
        lines
    });

    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(rest) = line.strip_prefix(LINE_PREFIX) {
                on_line(rest);
            } else {
                on_log(&line);
            }
        }
    }
    let status = child.wait().map_err(|e| PrivError::new("Lost the privileged helper").detail(e.to_string()))?;
    let stderr_lines = err_thread.join().unwrap_or_default();
    let tail = stderr_lines.join("\n");
    Ok(PrivOutcome { success: status.success(), exit_code: status.code(), stderr_tail: tail })
}

/// Interprets a failed pkexec/sudo launch (as opposed to a failure of the
/// helper's own work) into something actionable.
pub fn explain_launch_failure(outcome: &PrivOutcome) -> PrivErrorInfo {
    let t = outcome.stderr_tail.to_lowercase();
    let code = outcome.exit_code;
    if t.contains("no authentication agent") || t.contains("error getting authority") || code == Some(127) {
        return PrivErrorInfo {
            message: "Administrator authentication is not available (no polkit authentication agent is running)".into(),
            hint: Some("Log out and back in, install an authentication agent (e.g. polkit-kde-agent-1), or run from a terminal: sudo blue-environment …".into()),
        };
    }
    if code == Some(126) || t.contains("not authorized") || t.contains("dismissed") {
        return PrivErrorInfo { message: "Administrator authentication was cancelled or refused".into(), hint: Some("Try again and enter your password when asked.".into()) };
    }
    if t.contains("a password is required") || t.contains("sudo:") {
        return PrivErrorInfo { message: "sudo needs a password".into(), hint: Some("Run the operation again — a polkit prompt will be used.".into()) };
    }
    PrivErrorInfo { message: "The privileged helper exited unexpectedly".into(), hint: None }
}

pub struct PrivErrorInfo {
    pub message: String,
    pub hint: Option<String>,
}

impl From<PrivError> for (String, Option<String>, Option<String>) {
    fn from(e: PrivError) -> Self {
        (e.message, e.hint, e.detail)
    }
}
