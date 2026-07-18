use std::process::Command;

#[tauri::command]
pub async fn translate_text(text: String, from: String, to: String) -> Result<String, String> {
    let from_arg = if from == "auto" { "auto".to_string() } else { from };

    let check = Command::new("which").arg("trans").output();
    if check.map(|o| !o.status.success()).unwrap_or(true) {
        return Err(
            "translate-shell (the `trans` command) is not installed. Install it with your \
             package manager (e.g. `sudo dnf install translate-shell`), or configure LibreTranslate \
             / an AI service instead — see Settings.".to_string()
        );
    }

    let output = Command::new("trans")
        .arg("-b") // brief output: translation only, no extra formatting
        .arg(format!("{}:{}", from_arg, to))
        .arg(&text)
        .output()
        .map_err(|e| format!("Failed to run translate-shell: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "translate-shell exited with an error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
