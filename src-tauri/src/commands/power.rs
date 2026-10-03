use crate::types::*;
use std::process::Command;

/// `name`/`icon` are stable identifiers (`powerprofilesctl`'s own
/// profile names, or the fixed "balanced" fallback) — the frontend maps
/// these through its i18n system for display (see `PowerSection.svelte`
/// `KNOWN_PROFILE_KEYS`). `description` used to be hardcoded Polish text
/// baked in here and sent to *every* user regardless of their selected
/// UI language — a real, previously-shipped i18n bug: an English- or
/// German-language install would still show "Oszczędzanie energii" for
/// the power-saver profile, because that string never went through
/// `$t()` at all, it was just backend-authored prose displayed as-is.
/// `description` is now just an English fallback label (for the rare
/// case a future profile id the frontend doesn't recognize shows up —
/// see that same `KNOWN_PROFILE_KEYS` fallback path), not the
/// authoritative display string.
/// Profile ids found in `powerprofilesctl list` output (order preserved).
fn parse_profile_names(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in text.lines() {
        let t = line.trim_start_matches(|c: char| c == '*' || c.is_whitespace());
        if let Some(name) = t.strip_suffix(':') {
            if matches!(name, "power-saver" | "balanced" | "performance") && !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::parse_profile_names;

    #[test]
    fn parses_real_powerprofilesctl_output() {
        let out = "  performance:\n    CpuDriver:\tintel_pstate\n    PlatformDriver:\tplatform_profile\n    Degraded:   no\n\n* balanced:\n    CpuDriver:\tintel_pstate\n    PlatformDriver:\tplatform_profile\n\n  power-saver:\n    CpuDriver:\tintel_pstate\n";
        assert_eq!(parse_profile_names(out), vec!["performance", "balanced", "power-saver"]);
    }

    #[test]
    fn two_profile_machine_has_no_performance() {
        let out = "* balanced:\n    Driver:\tplatform_profile\n\n  power-saver:\n    Driver:\tplatform_profile\n";
        assert_eq!(parse_profile_names(out), vec!["balanced", "power-saver"]);
    }

    #[test]
    fn ignores_garbage() {
        assert!(parse_profile_names("").is_empty());
        assert!(parse_profile_names("    Driver:\tx\n  unknown:\n").is_empty());
    }
}

#[tauri::command]
pub async fn get_power_profiles() -> Result<Vec<PowerProfile>, String> {
    tokio::task::spawn_blocking(move || -> Vec<PowerProfile> {
        // `powerprofilesctl list` prints e.g.
        //     * performance:      <- the active one starts with "* "
        //         Driver: ...
        //       balanced:
        //       power-saver:
        // The old parser took the first whitespace token of the "*" line,
        // which is the literal "*" — so NO profile ever matched as active.
        // Profile names are the lines that end with ':' and are not indented
        // key/value pairs, so parse those, and ask `powerprofilesctl get`
        // for the active one (authoritative).
        let Ok(out) = Command::new("powerprofilesctl").arg("list").output() else {
            // power-profiles-daemon not installed: nothing can be switched, so
            // report no profiles (the UI hides the panel switcher) instead of
            // faking a list whose buttons silently do nothing.
            return Vec::new();
        };
        if !out.status.success() {
            return Vec::new();
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let names = parse_profile_names(&text);
        let active = Command::new("powerprofilesctl").arg("get").output().ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| text.lines().find(|l| l.trim_start().starts_with('*'))
                .map(|l| l.trim_start_matches(|c: char| c == '*' || c.is_whitespace()).trim_end_matches(':').to_string()))
            .unwrap_or_default();

        // Stable display order: saver → balanced → performance.
        let mut profiles = Vec::new();
        for (id, icon, label) in [("power-saver", "Battery", "Power Saver"), ("balanced", "Wind", "Balanced"), ("performance", "Zap", "Performance")] {
            if names.iter().any(|n| n == id) {
                profiles.push(PowerProfile { name: id.to_string(), active: active == id, icon: Some(icon.to_string()), description: label.to_string() });
            }
        }
        profiles
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn set_power_profile(profile: String) -> Result<(), String> {
    // Whitelist: the value comes from the webview, and `powerprofilesctl set`
    // would otherwise accept (and we'd forward) any string as an argument.
    if !matches!(profile.as_str(), "power-saver" | "balanced" | "performance") {
        return Err(format!("unknown power profile: {profile}"));
    }
    // `output()` (waits) instead of `spawn()`: the old fire-and-forget call
    // reported success even when the daemon rejected the profile, and the
    // UI could re-read the state before the change had happened.
    let out = Command::new("powerprofilesctl").args(["set", &profile]).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { format!("powerprofilesctl set {profile} failed") } else { err })
    }
}

#[tauri::command(async)]
pub fn set_brightness(level: i32) {
    // Never 0 %: a fully black panel with no way to see the slider again.
    let level = level.clamp(1, 100);
    // `status()` (waits) instead of `spawn()` — a stream of fire-and-forget
    // spawns while dragging the slider left zombie processes and let older
    // calls finish AFTER newer ones, making the brightness jump back.
    let ok = Command::new("brightnessctl").args(["set", &format!("{}%", level)])
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        let _ = Command::new("sh").arg("-c")
            .arg(format!("xrandr --output $(xrandr | grep ' connected' | head -1 | cut -d' ' -f1) --brightness {:.2}", level as f32 / 100.0))
            .status();
    }
}
