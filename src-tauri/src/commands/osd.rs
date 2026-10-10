use crate::shell_osd::{self, VolumeState};
use serde::Serialize;

#[derive(Serialize)]
pub struct OsdState {
    pub volume: Option<VolumeState>,
    pub brightness: Option<i32>,
}

/// Current levels (`None` where the machine has no such control).
#[tauri::command(async)]
pub fn osd_get_state() -> OsdState {
    OsdState { volume: shell_osd::read_volume(), brightness: shell_osd::read_brightness() }
}

/// Steps the default output's volume by `delta` percentage points.
#[tauri::command(async)]
pub fn osd_adjust_volume(delta: i32) -> Result<VolumeState, String> {
    shell_osd::adjust_volume(delta.clamp(-100, 100))
}

#[tauri::command(async)]
pub fn osd_toggle_mute() -> Result<VolumeState, String> {
    shell_osd::toggle_mute()
}

/// Steps the backlight by `delta` percentage points (never below 1 %).
#[tauri::command(async)]
pub fn osd_adjust_brightness(delta: i32) -> Result<i32, String> {
    let cur = shell_osd::read_brightness().ok_or_else(|| "no backlight found".to_string())?;
    let target = shell_osd::stepped_brightness(cur, delta.clamp(-100, 100));
    if target != cur {
        crate::commands::power::set_brightness(target);
    }
    Ok(shell_osd::read_brightness().unwrap_or(target))
}
