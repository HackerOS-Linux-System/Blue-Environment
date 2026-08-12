use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn calendar_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/Blue-Environment/blue-calendar")
}

fn events_path() -> PathBuf {
    calendar_dir().join("events.json")
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    /// ISO 8601 date, `YYYY-MM-DD` — the event's calendar day.
    pub date: String,
    /// `HH:MM` 24h, or `None` for an all-day event.
    pub time: Option<String>,
    /// Duration in minutes, ignored (and meaningless) for all-day
    /// events. Purely for how long a block to draw in a future
    /// week/day view — see ROADMAP.md, only the month grid is
    /// implemented so far so nothing reads this yet, but it's cheap to
    /// capture from the create form now rather than needing a data
    /// migration later.
    pub duration_minutes: Option<u32>,
    pub description: String,
    /// Hex color for the event's dot/chip in the month grid — one of a
    /// small fixed palette the frontend offers, not free-form.
    pub color: String,
}

fn read_events() -> Vec<CalendarEvent> {
    fs::read_to_string(events_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_events(events: &[CalendarEvent]) -> Result<(), String> {
    fs::create_dir_all(calendar_dir()).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(events).map_err(|e| e.to_string())?;
    fs::write(events_path(), json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn calendar_load_events() -> Vec<CalendarEvent> {
    read_events()
}

#[tauri::command]
pub fn calendar_save_event(event: CalendarEvent) -> Result<(), String> {
    let mut events = read_events();
    if let Some(existing) = events.iter_mut().find(|e| e.id == event.id) {
        *existing = event;
    } else {
        events.push(event);
    }
    write_events(&events)
}

#[tauri::command]
pub fn calendar_delete_event(id: String) -> Result<(), String> {
    let mut events = read_events();
    events.retain(|e| e.id != id);
    write_events(&events)
}
