use std::process::Command;

const SUBCOMMANDS: &[&str] = &[
    "list", "info", "permission-list", "permission-show", "permission-set", "permission-reset", "override",
];

/// `--flag` or `--flag=value` forms the panel is allowed to use.
const FLAG_PREFIXES: &[&str] = &[
    "--app", "--columns=", "--show-permissions", "--show", "--user", "--reset",
    "--share=", "--unshare=", "--socket=", "--nosocket=", "--device=", "--nodevice=",
    "--allow=", "--disallow=", "--filesystem=", "--nofilesystem=",
];

fn plain_token(s: &str) -> bool {
    !s.is_empty() && s.len() <= 200 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | ':' | '/' | '~' | ','))
}

/// Pure validation, unit-tested below.
pub fn validate_args(args: &[String]) -> Result<(), String> {
    let first = args.first().ok_or("no flatpak subcommand")?;
    if args.len() == 1 && first == "--version" {
        return Ok(());
    }
    if !SUBCOMMANDS.contains(&first.as_str()) {
        return Err(format!("flatpak subcommand not allowed: {first}"));
    }
    for a in &args[1..] {
        if a.contains('\0') || a.contains('\n') {
            return Err("invalid character in argument".into());
        }
        if a.starts_with('-') {
            if !FLAG_PREFIXES.iter().any(|p| if p.ends_with('=') { a.starts_with(p) } else { a == p }) {
                return Err(format!("flatpak option not allowed: {a}"));
            }
            if let Some((_, v)) = a.split_once('=') {
                if v.contains("..") || (!v.is_empty() && !plain_token(v)) {
                    return Err(format!("invalid option value: {a}"));
                }
            }
        } else if !plain_token(a) {
            return Err(format!("invalid argument: {a}"));
        }
    }
    // Overrides must always target the per-user installation; never a system-wide one.
    if first == "override" && !args.iter().any(|a| a == "--user") {
        return Err("override requires --user".into());
    }
    Ok(())
}

#[tauri::command(async)]
pub fn flatpak_exec(args: Vec<String>) -> Result<String, String> {
    validate_args(&args)?;
    let out = Command::new("flatpak")
        .args(&args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| format!("flatpak not available: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn allows_panel_commands() {
        assert!(validate_args(&v(&["--version"])).is_ok());
        assert!(validate_args(&v(&["list", "--app", "--columns=application,name,version"])).is_ok());
        assert!(validate_args(&v(&["info", "--show-permissions", "org.mozilla.firefox"])).is_ok());
        assert!(validate_args(&v(&["override", "--user", "--show", "org.a.B"])).is_ok());
        assert!(validate_args(&v(&["override", "--user", "--unshare=network", "org.a.B"])).is_ok());
        assert!(validate_args(&v(&["override", "--user", "--nofilesystem=home", "org.a.B"])).is_ok());
        assert!(validate_args(&v(&["permission-set", "devices", "camera", "org.a.B", "yes"])).is_ok());
        assert!(validate_args(&v(&["permission-reset", "org.a.B"])).is_ok());
    }

    #[test]
    fn rejects_everything_else() {
        assert!(validate_args(&[]).is_err());
        assert!(validate_args(&v(&["run", "org.a.B"])).is_err());
        assert!(validate_args(&v(&["install", "x"])).is_err());
        assert!(validate_args(&v(&["override", "--system", "org.a.B"])).is_err());
        assert!(validate_args(&v(&["override", "--unshare=network", "org.a.B"])).is_err()); // missing --user
        assert!(validate_args(&v(&["override", "--user", "--talk-name=org.x", "org.a.B"])).is_err());
        assert!(validate_args(&v(&["override", "--user", "--filesystem=../etc", "org.a.B"])).is_err());
        assert!(validate_args(&v(&["info", "org.a.B; rm -rf /"])).is_err());
        assert!(validate_args(&v(&["info", "org.a.B\nx"])).is_err());
    }
}
