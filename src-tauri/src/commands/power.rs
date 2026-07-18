use crate::types::*;
use crate::{apps, cache, ai, packages, window_tracker};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as TokioCommand;
use serde::{Serialize, Deserialize};

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
        profiles.push(PowerProfile { name: "power-saver".to_string(), active: active == "power-saver", icon: Some("Battery".to_string()), description: "Oszczędzanie energii".to_string() });
    }
    profiles.push(PowerProfile { name: "balanced".to_string(), active: active == "balanced" || active.is_empty(), icon: Some("Wind".to_string()), description: "Zrównoważony".to_string() });
    if has_perf {
        profiles.push(PowerProfile { name: "performance".to_string(), active: active == "performance", icon: Some("Zap".to_string()), description: "Wydajność".to_string() });
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
