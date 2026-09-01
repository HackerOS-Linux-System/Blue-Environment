use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn calendar_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/Blue-Environment/blue-calendar")
}

fn events_path() -> PathBuf { calendar_dir().join("events.json") }
fn subscriptions_path() -> PathBuf { calendar_dir().join("subscriptions.json") }
fn subscription_cache_path(id: &str) -> PathBuf {
    calendar_dir().join(format!("subscription-{id}.json"))
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecurrenceRule {
    pub freq: String, // "daily" | "weekly" | "monthly" | "yearly"
    pub interval: u32,
    /// Weekly only: `["MO","WE","FR"]` style two-letter weekday codes
    /// (matches iCalendar's own `BYDAY` short codes, so ICS import can
    /// map directly without a separate vocabulary).
    pub by_day: Option<Vec<String>>,
    /// Inclusive end date `YYYY-MM-DD`, or `None` for "no end / until count".
    pub until: Option<String>,
    /// Total occurrence count (including the first), or `None` for
    /// "no count / until date / forever".
    pub count: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    /// ISO 8601 date, `YYYY-MM-DD` — the event's (first) calendar day.
    pub date: String,
    /// `HH:MM` 24h, or `None` for an all-day event.
    pub time: Option<String>,
    pub duration_minutes: Option<u32>,
    pub description: String,
    pub color: String,
    /// `None` for a one-off event — see module doc.
    pub recurrence: Option<RecurrenceRule>,
    /// Set only on events materialized from a `CalendarSubscription` —
    /// the frontend uses this to make them visually distinct and
    /// non-editable (see module doc: no write-back to the source).
    pub subscription_id: Option<String>,
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

// ── External calendar subscriptions (read-only ICS) ─────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CalendarSubscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub color: String,
    pub enabled: bool,
    /// Set after the first successful sync; `None` means "never synced
    /// yet" (shown differently in the UI than "sync failed").
    pub last_synced: Option<String>,
}

fn read_subscriptions() -> Vec<CalendarSubscription> {
    fs::read_to_string(subscriptions_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn write_subscriptions(subs: &[CalendarSubscription]) -> Result<(), String> {
    fs::create_dir_all(calendar_dir()).map_err(|e| e.to_string())?;
    fs::write(subscriptions_path(), serde_json::to_string_pretty(subs).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn calendar_list_subscriptions() -> Vec<CalendarSubscription> {
    read_subscriptions()
}

#[tauri::command]
pub fn calendar_add_subscription(name: String, url: String, color: String) -> Result<CalendarSubscription, String> {
    let sub = CalendarSubscription {
        id: format!("sub{}", chrono::Local::now().timestamp_millis()),
        name, url, color, enabled: true, last_synced: None,
    };
    let mut subs = read_subscriptions();
    subs.push(sub.clone());
    write_subscriptions(&subs)?;
    Ok(sub)
}

#[tauri::command]
pub fn calendar_remove_subscription(id: String) -> Result<(), String> {
    let mut subs = read_subscriptions();
    subs.retain(|s| s.id != id);
    write_subscriptions(&subs)?;
    let _ = fs::remove_file(subscription_cache_path(&id));
    Ok(())
}

#[tauri::command]
pub fn calendar_set_subscription_enabled(id: String, enabled: bool) -> Result<(), String> {
    let mut subs = read_subscriptions();
    if let Some(s) = subs.iter_mut().find(|s| s.id == id) { s.enabled = enabled; }
    write_subscriptions(&subs)
}

/// Returns whatever this subscription's cached events currently are
/// (from the last successful `calendar_sync_subscription` call), without
/// hitting the network — the frontend calls this on startup to show
/// last-known events immediately, then calls `calendar_sync_subscription`
/// in the background to refresh.
#[tauri::command]
pub fn calendar_cached_subscription_events(id: String) -> Vec<CalendarEvent> {
    fs::read_to_string(subscription_cache_path(&id)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Fetches `sub.url`, parses `VEVENT` blocks, converts each into a
/// `CalendarEvent` (with `subscription_id` set), caches the result, and
/// returns it. See module doc's "External calendars" section for
/// exactly what this does and doesn't implement.
#[tauri::command]
pub async fn calendar_sync_subscription(id: String) -> Result<Vec<CalendarEvent>, String> {
    let mut subs = read_subscriptions();
    let sub = subs.iter().find(|s| s.id == id).cloned().ok_or_else(|| format!("no subscription with id {id}"))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("BlueCalendar/1.0")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&sub.url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("calendar feed returned HTTP {}", resp.status()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let events = parse_ics(&body, &sub.id, &sub.color);

    fs::create_dir_all(calendar_dir()).map_err(|e| e.to_string())?;
    fs::write(subscription_cache_path(&id), serde_json::to_string_pretty(&events).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    if let Some(s) = subs.iter_mut().find(|s| s.id == id) {
        s.last_synced = Some(chrono::Local::now().to_rfc3339());
    }
    write_subscriptions(&subs)?;

    Ok(events)
}

/// Extracts unfolded (RFC 5545 line-folding undone: a continuation line
/// starts with a single space/tab) logical lines from raw ICS text.
/// Real ICS producers wrap long lines at ~75 octets with a leading
/// space on the continuation — without unfolding this first, a long
/// `SUMMARY` or `RRULE` line gets silently truncated at the wrap point.
fn unfold_ics_lines(raw: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        if (line.starts_with(' ') || line.starts_with('\t')) && !lines.is_empty() {
            let last = lines.last_mut().unwrap();
            last.push_str(line[1..].trim_end_matches('\r'));
        } else {
            lines.push(line.trim_end_matches('\r').to_string());
        }
    }
    lines
}

fn ics_prop(line: &str) -> Option<(&str, &str, &str)> {
    // "DTSTART;TZID=Europe/Warsaw:20260101T120000" -> ("DTSTART", ";TZID=Europe/Warsaw", "20260101T120000")
    let colon = line.find(':')?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let (name, params) = head.split_once(';').unwrap_or((head, ""));
    Some((name, params, value))
}

fn ics_unescape(s: &str) -> String {
    s.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

/// Converts an ICS `DTSTART`-style value (`20260315`, `20260315T140000`,
/// or `20260315T130000Z`) into this app's `(date, time)` shape. A
/// trailing `Z` (UTC) is treated as already-local — see module doc's
/// honest caveat: no real timezone database here, so a UTC or
/// TZID-qualified time is taken at face value rather than converted.
fn ics_datetime_to_local(value: &str) -> (String, Option<String>) {
    let v = value.trim_end_matches('Z');
    if v.len() >= 8 {
        let date = format!("{}-{}-{}", &v[0..4], &v[4..6], &v[6..8]);
        if v.len() >= 15 && v.as_bytes().get(8) == Some(&b'T') {
            let time = format!("{}:{}", &v[9..11], &v[11..13]);
            return (date, Some(time));
        }
        return (date, None);
    }
    (value.to_string(), None)
}

/// Best-effort `RRULE` value (`FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;COUNT=10`)
/// -> this app's `RecurrenceRule`. Unrecognized/unsupported parts (e.g.
/// `BYMONTHDAY`, `BYSETPOS`) are silently dropped rather than rejecting
/// the whole event — a recurring event with a rule this app can't fully
/// represent still shows up as *something* (usually its first
/// occurrence, or a simplified recurrence) rather than not appearing at
/// all, which is the more useful failure mode for a calendar.
fn parse_ics_rrule(value: &str) -> Option<RecurrenceRule> {
    let mut freq = None;
    let mut interval = 1u32;
    let mut by_day = None;
    let mut until = None;
    let mut count = None;
    for part in value.split(';') {
        let Some((k, v)) = part.split_once('=') else { continue };
        match k {
            "FREQ" => freq = Some(match v {
                "DAILY" => "daily", "WEEKLY" => "weekly", "MONTHLY" => "monthly", "YEARLY" => "yearly",
                _ => return None, // unsupported frequency (e.g. SECONDLY) — bail rather than guess
            }.to_string()),
            "INTERVAL" => interval = v.parse().unwrap_or(1),
            "BYDAY" => by_day = Some(v.split(',').map(|d| d.to_string()).collect()),
            "UNTIL" => until = Some(ics_datetime_to_local(v).0),
            "COUNT" => count = v.parse().ok(),
            _ => {}
        }
    }
    Some(RecurrenceRule { freq: freq?, interval, by_day, until, count })
}

fn parse_ics(raw: &str, subscription_id: &str, default_color: &str) -> Vec<CalendarEvent> {
    let lines = unfold_ics_lines(raw);
    let mut events = Vec::new();
    let mut in_event = false;
    let mut summary = String::new();
    let mut dtstart: Option<String> = None;
    let mut uid = String::new();
    let mut description = String::new();
    let mut rrule: Option<RecurrenceRule> = None;

    for line in &lines {
        if line == "BEGIN:VEVENT" {
            in_event = true;
            summary.clear(); dtstart = None; uid.clear(); description.clear(); rrule = None;
            continue;
        }
        if line == "END:VEVENT" {
            if in_event {
                if let Some(dt) = dtstart.take() {
                    let (date, time) = ics_datetime_to_local(&dt);
                    events.push(CalendarEvent {
                        id: if uid.is_empty() { format!("{subscription_id}-{}", events.len()) } else { format!("{subscription_id}-{uid}") },
                        title: if summary.is_empty() { "(untitled)".to_string() } else { summary.clone() },
                        date, time,
                        duration_minutes: None,
                        description: description.clone(),
                        color: default_color.to_string(),
                        recurrence: rrule.clone(),
                        subscription_id: Some(subscription_id.to_string()),
                    });
                }
            }
            in_event = false;
            if events.len() >= 500 { break } // sane cap against a pathological feed
            continue;
        }
        if !in_event { continue }
        let Some((name, _params, value)) = ics_prop(line) else { continue };
        match name {
            "SUMMARY" => summary = ics_unescape(value),
            "DESCRIPTION" => description = ics_unescape(value),
            "UID" => uid = value.to_string(),
            "DTSTART" => dtstart = Some(value.to_string()),
            "RRULE" => rrule = parse_ics_rrule(value),
            _ => {}
        }
    }
    events
}
