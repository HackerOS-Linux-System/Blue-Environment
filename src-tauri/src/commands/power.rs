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
#[tauri::command]
pub fn get_power_profiles() -> Vec<PowerProfile> {
    let mut profiles = Vec::new();
    let out = Command::new("powerprofilesctl").arg("list").output();
    let (has_saver, has_balanced, has_perf, active) = if let Ok(o) = out {
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        let active = text.lines().find(|l| l.contains('*'))
        .and_then(|l| l.split_whitespace().next())
        .unwrap_or("").trim_end_matches(':').to_string();
        (text.contains("power-saver"), text.contains("balanced"), text.contains("performance"), active)
    } else { (false, false, false, "balanced".to_string()) };

    if has_saver || !has_balanced {
        profiles.push(PowerProfile { name: "power-saver".to_string(), active: active == "power-saver", icon: Some("Battery".to_string()), description: "Power Saver".to_string() });
    }
    profiles.push(PowerProfile { name: "balanced".to_string(), active: active == "balanced" || active.is_empty(), icon: Some("Wind".to_string()), description: "Balanced".to_string() });
    if has_perf {
        profiles.push(PowerProfile { name: "performance".to_string(), active: active == "performance", icon: Some("Zap".to_string()), description: "Performance".to_string() });
    }
    profiles
}

#[tauri::command]
pub fn set_power_profile(profile: String) -> Result<(), String> {
    Command::new("powerprofilesctl").args(["set", &profile]).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_brightness(level: i32) {
    if Command::new("brightnessctl").args(["set", &format!("{}%", level)]).spawn().is_err() {
        let _ = Command::new("sh").arg("-c")
        .arg(format!("xrandr --output $(xrandr | grep ' connected' | head -1 | cut -d' ' -f1) --brightness {:.2}", level as f32 / 100.0))
        .spawn();
    }
}
