use serde::{Deserialize, Serialize};
use std::process::{Command, Output};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl InstallError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.to_string(), message: message.into(), hint: None, detail: None }
    }
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
    pub fn io(context: &str, e: std::io::Error) -> Self {
        Self::new("io", format!("{context}: {e}")).detail(e.to_string())
    }
    pub fn command_failed(cmd: &Command, out: &Output) -> Self {
        let program = cmd.get_program().to_string_lossy().to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        Self::new("command_failed", format!("`{program}` failed (exit code {})", out.status.code().unwrap_or(-1))).detail(detail)
    }
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}
impl std::error::Error for InstallError {}

pub type InstallResult<T> = Result<T, InstallError>;
