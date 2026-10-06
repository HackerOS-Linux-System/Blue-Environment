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
    /// Dates (`YYYY-MM-DD`) of single occurrences that were deleted/moved out of a
    /// recurring series (iCalendar `EXDATE`). They still count towards `count`.
    #[serde(default)]
    pub exdates: Vec<String>,
    /// Minutes BEFORE the start at which a reminder fires (`0` = at start time);
    /// `None` = no reminder. For all-day events the "start" is 09:00 that day.
    #[serde(default)]
    pub reminder_minutes: Option<u32>,
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

#[tauri::command(async)]
pub fn calendar_load_events() -> Vec<CalendarEvent> {
    read_events()
}

#[tauri::command(async)]
pub fn calendar_save_event(event: CalendarEvent) -> Result<(), String> {
    let mut events = read_events();
    if let Some(existing) = events.iter_mut().find(|e| e.id == event.id) {
        *existing = event;
    } else {
        events.push(event);
    }
    write_events(&events)
}

#[tauri::command(async)]
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

#[tauri::command(async)]
pub fn calendar_list_subscriptions() -> Vec<CalendarSubscription> {
    read_subscriptions()
}

/// `webcal://` is just `https://` with a different label (that is how Google, Nextcloud,
/// Apple… hand out "subscribe" links); anything but http(s) is rejected.
fn normalize_feed_url(url: &str) -> Result<String, String> {
    let u = url.trim();
    let u = if let Some(rest) = u.strip_prefix("webcal://") { format!("https://{rest}") }
        else if let Some(rest) = u.strip_prefix("WEBCAL://") { format!("https://{rest}") }
        else { u.to_string() };
    let lower = u.to_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err("The calendar address must start with https:// (or webcal://)".to_string());
    }
    if u.len() > 2048 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("Invalid calendar address".to_string());
    }
    Ok(u)
}

#[tauri::command(async)]
pub fn calendar_add_subscription(name: String, url: String, color: String) -> Result<CalendarSubscription, String> {
    let url = normalize_feed_url(&url)?;
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Give the calendar a name".to_string()); }
    let sub = CalendarSubscription {
        id: format!("sub{}", chrono::Local::now().timestamp_millis()),
        name, url, color, enabled: true, last_synced: None,
    };
    let mut subs = read_subscriptions();
    subs.push(sub.clone());
    write_subscriptions(&subs)?;
    Ok(sub)
}

#[tauri::command(async)]
pub fn calendar_remove_subscription(id: String) -> Result<(), String> {
    let mut subs = read_subscriptions();
    subs.retain(|s| s.id != id);
    write_subscriptions(&subs)?;
    let _ = fs::remove_file(subscription_cache_path(&id));
    Ok(())
}

#[tauri::command(async)]
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
#[tauri::command(async)]
pub fn calendar_cached_subscription_events(id: String) -> Vec<CalendarEvent> {
    fs::read_to_string(subscription_cache_path(&id)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Fetches `sub.url`, parses `VEVENT` blocks, converts each into a
/// `CalendarEvent` (with `subscription_id` set), caches the result, and
/// returns it. See module doc's "External calendars" section for
/// exactly what this does and doesn't implement.
// <network>
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
    let events = parse_ics(&body, Some(&sub.id), &sub.color);

    fs::create_dir_all(calendar_dir()).map_err(|e| e.to_string())?;
    fs::write(subscription_cache_path(&id), serde_json::to_string_pretty(&events).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    if let Some(s) = subs.iter_mut().find(|s| s.id == id) {
        s.last_synced = Some(chrono::Local::now().to_rfc3339());
    }
    write_subscriptions(&subs)?;

    Ok(events)
}
// </network>

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

/// RFC 5545 TEXT unescape, one pass (the old chained `replace` turned a literal
/// backslash followed by `n` into a newline).
fn ics_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' { out.push(c); continue; }
        match it.next() {
            Some('n') | Some('N') => out.push('\n'),
            Some(other) => out.push(other), // \, \; \\ and anything else → the char itself
            None => out.push('\\'),
        }
    }
    out
}

fn ics_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// `20260315T130000Z` (UTC) → the user's local date/time. Google Calendar and most servers
/// write every timed event in UTC, so taking the digits at face value (as this used to)
/// shifted every event by the timezone offset.
fn utc_digits_to_local(v: &str) -> Option<(String, Option<String>)> {
    use chrono::TimeZone;
    let naive = chrono::NaiveDateTime::parse_from_str(&v[..15], "%Y%m%dT%H%M%S").ok()?;
    let local = chrono::Utc.from_utc_datetime(&naive).with_timezone(&chrono::Local);
    Some((local.format("%Y-%m-%d").to_string(), Some(local.format("%H:%M").to_string())))
}

/// Converts an ICS `DTSTART`-style value (`20260315`, `20260315T140000`, `20260315T130000Z`)
/// into this app's `(date, time)`. `Z` values are converted to local time. A `TZID=`-qualified
/// time is taken at face value (there is no timezone database here), i.e. assumed to be in
/// the user's own timezone.
fn ics_datetime_to_local(value: &str) -> (String, Option<String>) {
    let is_utc = value.ends_with('Z');
    let v = value.trim_end_matches('Z');
    let digits = |r: std::ops::Range<usize>| v.get(r).map_or(false, |x| x.chars().all(|c| c.is_ascii_digit()));
    if v.len() >= 8 && digits(0..8) {
        if v.len() >= 15 && v.as_bytes().get(8) == Some(&b'T') && digits(9..15) {
            if is_utc {
                if let Some(local) = utc_digits_to_local(v) { return local; }
            }
            return (format!("{}-{}-{}", &v[0..4], &v[4..6], &v[6..8]), Some(format!("{}:{}", &v[9..11], &v[11..13])));
        }
        return (format!("{}-{}-{}", &v[0..4], &v[4..6], &v[6..8]), None);
    }
    (value.to_string(), None)
}

fn minutes_of(time: &str) -> Option<i64> {
    let (h, m) = time.split_once(':')?;
    Some(h.parse::<i64>().ok()? * 60 + m.parse::<i64>().ok()?)
}

/// Minutes from `(d1,t1)` to `(d2,t2)`; `None` if either is malformed or the span is not positive.
fn span_minutes(d1: &str, t1: &str, d2: &str, t2: &str) -> Option<u32> {
    let a = chrono::NaiveDate::parse_from_str(d1, "%Y-%m-%d").ok()?;
    let b = chrono::NaiveDate::parse_from_str(d2, "%Y-%m-%d").ok()?;
    let total = (b - a).num_days() * 24 * 60 + minutes_of(t2)? - minutes_of(t1)?;
    if total > 0 && total <= 60 * 24 * 31 { Some(total as u32) } else { None }
}

/// ISO-8601 duration as used by `DURATION` / `TRIGGER` (`PT15M`, `-PT1H30M`, `-P1D`, `P1W`)
/// → signed minutes.
fn parse_ics_duration(v: &str) -> Option<i64> {
    let v = v.trim();
    let (neg, v) = match v.strip_prefix('-') { Some(r) => (true, r), None => (false, v.strip_prefix('+').unwrap_or(v)) };
    let v = v.strip_prefix('P')?;
    let (mut total, mut num, mut in_time) = (0i64, String::new(), false);
    for c in v.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' => num.push(c),
            _ => {
                let n: i64 = num.parse().ok()?; num.clear();
                total += match (c, in_time) {
                    ('W', false) => n * 7 * 24 * 60, ('D', false) => n * 24 * 60,
                    ('H', true) => n * 60, ('M', true) => n, ('S', true) => n / 60,
                    _ => return None,
                };
            }
        }
    }
    if !num.is_empty() { return None; }
    Some(if neg { -total } else { total })
}

/// Best-effort `RRULE` (`FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;COUNT=10`) → `RecurrenceRule`.
/// Ordinal weekday prefixes (`1MO`, `-1FR`) are dropped; unsupported parts (`BYMONTHDAY`,
/// `BYSETPOS`…) are ignored rather than rejecting the event, so a recurring event with a rule
/// this app can't fully represent still shows up as a simplified recurrence.
fn parse_ics_rrule(value: &str) -> Option<RecurrenceRule> {
    let (mut freq, mut interval, mut by_day, mut until, mut count) = (None, 1u32, None, None, None);
    for part in value.split(';') {
        let Some((k, v)) = part.split_once('=') else { continue };
        match k {
            "FREQ" => freq = Some(match v {
                "DAILY" => "daily", "WEEKLY" => "weekly", "MONTHLY" => "monthly", "YEARLY" => "yearly",
                _ => return None,
            }.to_string()),
            "INTERVAL" => interval = v.parse().unwrap_or(1).max(1),
            "BYDAY" => by_day = Some(v.split(',').map(|d| d.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '+').to_string()).filter(|d| !d.is_empty()).collect()),
            "UNTIL" => until = Some(ics_datetime_to_local(v).0),
            "COUNT" => count = v.parse().ok(),
            _ => {}
        }
    }
    Some(RecurrenceRule { freq: freq?, interval, by_day, until, count })
}

fn safe_id(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.@".contains(c) { c } else { '_' }).collect()
}

#[derive(Default)]
struct RawEvent {
    summary: String, description: String, uid: String,
    dtstart: Option<(String, Option<String>)>, dtend: Option<(String, Option<String>)>,
    duration: Option<i64>, rrule: Option<RecurrenceRule>,
    exdates: Vec<String>, recurrence_id: Option<String>,
    reminder: Option<u32>, cancelled: bool,
}

/// Parses `VEVENT`s. With `subscription_id` the events are read-only subscription events
/// (`id = <sub>-<uid>`); without it they are being imported as the user's own (`id = ics-<uid>`).
fn parse_ics(raw: &str, subscription_id: Option<&str>, default_color: &str) -> Vec<CalendarEvent> {
    let lines = unfold_ics_lines(raw);
    let mut out: Vec<CalendarEvent> = Vec::new();
    // (uid, original date) of single occurrences that were rewritten by RECURRENCE-ID overrides
    let mut overridden: Vec<(String, String)> = Vec::new();
    let mut cur: Option<RawEvent> = None;
    let mut in_alarm = false;

    for line in &lines {
        match line.as_str() {
            "BEGIN:VEVENT" => { cur = Some(RawEvent::default()); in_alarm = false; continue; }
            "BEGIN:VALARM" => { in_alarm = true; continue; }
            "END:VALARM" => { in_alarm = false; continue; }
            "END:VEVENT" => {
                in_alarm = false;
                let Some(ev) = cur.take() else { continue };
                if ev.cancelled { continue; }
                let Some((date, time)) = ev.dtstart.clone() else { continue };
                let duration_minutes = if time.is_none() { None } else if let Some(d) = ev.duration { (d > 0).then_some(d as u32) }
                    else if let Some((d2, Some(t2))) = &ev.dtend { span_minutes(&date, time.as_deref().unwrap(), d2, t2) } else { None };
                let base = if ev.uid.is_empty() { format!("n{}", out.len()) } else { safe_id(&ev.uid) };
                let is_override = ev.recurrence_id.is_some();
                let id_core = match &ev.recurrence_id { Some(r) => format!("{base}-{}", safe_id(r)), None => base.clone() };
                if let Some(rid) = &ev.recurrence_id { overridden.push((base.clone(), ics_datetime_to_local(rid).0)); }
                out.push(CalendarEvent {
                    id: match subscription_id { Some(sub) => format!("{sub}-{id_core}"), None => format!("ics-{id_core}") },
                    title: if ev.summary.is_empty() { "(untitled)".to_string() } else { ev.summary },
                    date, time, duration_minutes,
                    description: ev.description,
                    color: default_color.to_string(),
                    recurrence: if is_override { None } else { ev.rrule },
                    subscription_id: subscription_id.map(|s| s.to_string()),
                    exdates: ev.exdates,
                    reminder_minutes: ev.reminder,
                });
                if out.len() >= 3000 { break; }
                continue;
            }
            _ => {}
        }
        let Some(ev) = cur.as_mut() else { continue };
        let Some((name, params, value)) = ics_prop(line) else { continue };
        if in_alarm {
            // Only the trigger matters; the alarm's own DESCRIPTION/SUMMARY must NOT overwrite the event's.
            if name == "TRIGGER" && !params.contains("RELATED=END") {
                if let Some(m) = parse_ics_duration(value) {
                    if m <= 0 { let mins = (-m) as u32; ev.reminder = Some(ev.reminder.map_or(mins, |r| r.min(mins))); }
                }
            }
            continue;
        }
        match name {
            "SUMMARY" => ev.summary = ics_unescape(value),
            "DESCRIPTION" => ev.description = ics_unescape(value),
            "UID" => ev.uid = value.to_string(),
            "DTSTART" => ev.dtstart = Some(ics_datetime_to_local(value)),
            "DTEND" => ev.dtend = Some(ics_datetime_to_local(value)),
            "DURATION" => ev.duration = parse_ics_duration(value),
            "RRULE" => ev.rrule = parse_ics_rrule(value),
            "EXDATE" => ev.exdates.extend(value.split(',').map(|d| ics_datetime_to_local(d.trim()).0)),
            "RECURRENCE-ID" => ev.recurrence_id = Some(value.to_string()),
            "STATUS" => ev.cancelled = value.eq_ignore_ascii_case("CANCELLED"),
            _ => {}
        }
    }
    // A series whose single occurrence was replaced by an override must not ALSO show the original.
    for (uid, date) in overridden {
        let master_id = match subscription_id { Some(sub) => format!("{sub}-{uid}"), None => format!("ics-{uid}") };
        if let Some(m) = out.iter_mut().find(|e| e.id == master_id) {
            if !m.exdates.contains(&date) { m.exdates.push(date); }
        }
    }
    out
}

// ── Export ──────────────────────────────────────────────────────────────

/// RFC 5545 §3.1: fold at 75 octets, never inside a UTF-8 character.
fn fold_ics_line(line: &str) -> String {
    let mut out = String::new();
    let mut octets = 0usize;
    for c in line.chars() {
        let w = c.len_utf8();
        if octets + w > 75 { out.push_str("\r\n "); octets = 1; }
        out.push(c); octets += w;
    }
    out
}

fn compact_date(d: &str) -> String { d.replace('-', "") }

fn events_to_ics(events: &[CalendarEvent]) -> String {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let mut lines: Vec<String> = vec!["BEGIN:VCALENDAR".into(), "VERSION:2.0".into(), "PRODID:-//Blue Environment//Blue Calendar//EN".into(), "CALSCALE:GREGORIAN".into()];
    for e in events.iter().filter(|e| e.subscription_id.is_none()) {
        lines.push("BEGIN:VEVENT".into());
        lines.push(format!("UID:{}@blue-environment", e.id));
        lines.push(format!("DTSTAMP:{stamp}"));
        match &e.time {
            None => lines.push(format!("DTSTART;VALUE=DATE:{}", compact_date(&e.date))),
            Some(t) => {
                lines.push(format!("DTSTART:{}T{}00", compact_date(&e.date), t.replace(':', "")));
                if let (Some(mins), Ok(d), Some(tm)) = (e.duration_minutes, chrono::NaiveDate::parse_from_str(&e.date, "%Y-%m-%d"), minutes_of(t)) {
                    if let Some(start) = d.and_hms_opt((tm / 60) as u32, (tm % 60) as u32, 0) {
                        let end = start + chrono::Duration::minutes(mins as i64);
                        lines.push(format!("DTEND:{}", end.format("%Y%m%dT%H%M%S")));
                    }
                }
            }
        }
        lines.push(format!("SUMMARY:{}", ics_escape(&e.title)));
        if !e.description.is_empty() { lines.push(format!("DESCRIPTION:{}", ics_escape(&e.description))); }
        if let Some(r) = &e.recurrence {
            let mut parts = vec![format!("FREQ={}", r.freq.to_uppercase()), format!("INTERVAL={}", r.interval.max(1))];
            if let Some(days) = &r.by_day { if !days.is_empty() { parts.push(format!("BYDAY={}", days.join(","))); } }
            if let Some(u) = &r.until {
                parts.push(if e.time.is_some() { format!("UNTIL={}T235959", compact_date(u)) } else { format!("UNTIL={}", compact_date(u)) });
            }
            if let Some(c) = r.count { parts.push(format!("COUNT={c}")); }
            lines.push(format!("RRULE:{}", parts.join(";")));
        }
        for x in &e.exdates {
            lines.push(match &e.time {
                None => format!("EXDATE;VALUE=DATE:{}", compact_date(x)),
                Some(t) => format!("EXDATE:{}T{}00", compact_date(x), t.replace(':', "")),
            });
        }
        if let Some(m) = e.reminder_minutes {
            lines.push("BEGIN:VALARM".into());
            lines.push("ACTION:DISPLAY".into());
            lines.push(format!("DESCRIPTION:{}", ics_escape(&e.title)));
            lines.push(format!("TRIGGER:-PT{m}M"));
            lines.push("END:VALARM".into());
        }
        lines.push("END:VEVENT".into());
    }
    lines.push("END:VCALENDAR".into());
    let mut out = lines.iter().map(|l| fold_ics_line(l)).collect::<Vec<_>>().join("\r\n");
    out.push_str("\r\n");
    out
}

#[derive(Serialize)]
pub struct ImportSummary { pub added: usize, pub skipped: usize }

/// Adds the events of an `.ics` file to the user's calendar. Events whose UID is already
/// present are skipped, so importing the same file twice is harmless.
fn import_into(existing: &mut Vec<CalendarEvent>, content: &str) -> Result<ImportSummary, String> {
    if !content.contains("BEGIN:VCALENDAR") { return Err("This is not an iCalendar (.ics) file".to_string()); }
    let parsed = parse_ics(content, None, "#3b82f6");
    if parsed.is_empty() { return Err("No events found in the file".to_string()); }
    let (mut added, mut skipped) = (0, 0);
    for ev in parsed {
        if existing.iter().any(|e| e.id == ev.id) { skipped += 1; } else { existing.push(ev); added += 1; }
    }
    Ok(ImportSummary { added, skipped })
}

#[tauri::command(async)]
pub fn calendar_import_ics(content: String) -> Result<ImportSummary, String> {
    if content.len() > 8 * 1024 * 1024 { return Err("The file is too large".to_string()); }
    let mut events = read_events();
    let summary = import_into(&mut events, &content)?;
    write_events(&events)?;
    Ok(summary)
}

/// The user's own events (not subscriptions) as an `.ics` document.
#[tauri::command(async)]
pub fn calendar_export_ics() -> String {
    events_to_ics(&read_events())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str) -> CalendarEvent {
        CalendarEvent { id: id.into(), title: "T".into(), date: "2026-03-15".into(), time: None, duration_minutes: None,
            description: String::new(), color: "#3b82f6".into(), recurrence: None, subscription_id: None, exdates: vec![], reminder_minutes: None }
    }

    #[test]
    fn feed_urls() {
        assert_eq!(normalize_feed_url("webcal://cal.example.com/a.ics").unwrap(), "https://cal.example.com/a.ics");
        assert_eq!(normalize_feed_url("  https://x.org/a.ics ").unwrap(), "https://x.org/a.ics");
        for bad in ["file:///etc/passwd", "ftp://x", "javascript:alert(1)", "", "https://a b", "cal.example.com"] {
            assert!(normalize_feed_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn text_escaping_round_trips() {
        let s = "a, b; c\\d\nline2";
        assert_eq!(ics_unescape(&ics_escape(s)), s);
        assert_eq!(ics_unescape("x\\\\n"), "x\\n", "backslash + n must stay literal");
        assert_eq!(ics_unescape("a\\nb"), "a\nb");
    }

    #[test]
    fn durations() {
        assert_eq!(parse_ics_duration("PT15M"), Some(15));
        assert_eq!(parse_ics_duration("-PT1H30M"), Some(-90));
        assert_eq!(parse_ics_duration("-P1D"), Some(-1440));
        assert_eq!(parse_ics_duration("P1W"), Some(10080));
        assert_eq!(parse_ics_duration("PT0S"), Some(0));
        assert_eq!(parse_ics_duration("15M"), None);
        assert_eq!(parse_ics_duration("PT"), Some(0));
        assert_eq!(parse_ics_duration("P1H"), None);
    }

    #[test]
    fn floating_and_date_values() {
        assert_eq!(ics_datetime_to_local("20260315"), ("2026-03-15".into(), None));
        assert_eq!(ics_datetime_to_local("20260315T140000"), ("2026-03-15".into(), Some("14:00".into())));
        assert_eq!(ics_datetime_to_local("garbage").1, None);
    }

    #[test]
    fn utc_values_are_converted_to_local_time() {
        // Run with TZ=Asia/Tokyo (UTC+9, no DST) to pin the expectation.
        if std::env::var("TZ").as_deref() == Ok("Asia/Tokyo") {
            assert_eq!(ics_datetime_to_local("20260315T140000Z"), ("2026-03-15".into(), Some("23:00".into())));
            assert_eq!(ics_datetime_to_local("20260315T200000Z"), ("2026-03-16".into(), Some("05:00".into())), "date must roll over");
        }
        // Whatever the zone: a Z value must never be left untouched unless the offset is 0.
        use chrono::{Local, TimeZone, Offset};
        let off = Local.offset_from_utc_datetime(&chrono::NaiveDate::from_ymd_opt(2026, 3, 15).unwrap().and_hms_opt(14, 0, 0).unwrap()).fix().local_minus_utc();
        let got = ics_datetime_to_local("20260315T140000Z").1.unwrap();
        let expect = format!("{:02}:{:02}", (14 * 3600 + off).rem_euclid(86400) / 3600, ((14 * 3600 + off).rem_euclid(86400) % 3600) / 60);
        assert_eq!(got, expect);
    }

    const GOOGLE_LIKE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n\
BEGIN:VEVENT\r\nUID:weekly@x\r\nDTSTART;TZID=Europe/Warsaw:20260302T100000\r\nDTEND;TZID=Europe/Warsaw:20260302T113000\r\n\
RRULE:FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,WE;UNTIL=20260401\r\nEXDATE;TZID=Europe/Warsaw:20260309T100000\r\n\
SUMMARY:Stand-up\\, daily\r\nDESCRIPTION:Real description\r\n\
BEGIN:VALARM\r\nTRIGGER:-PT10M\r\nACTION:DISPLAY\r\nDESCRIPTION:This is an event reminder\r\nEND:VALARM\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:weekly@x\r\nRECURRENCE-ID;TZID=Europe/Warsaw:20260311T100000\r\nDTSTART;TZID=Europe/Warsaw:20260311T150000\r\nSUMMARY:Stand-up (moved)\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:gone@x\r\nDTSTART:20260320T090000\r\nSTATUS:CANCELLED\r\nSUMMARY:Cancelled\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:allday@x\r\nDTSTART;VALUE=DATE:20260325\r\nSUMMARY:Holiday\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:dur@x\r\nDTSTART:20260326T080000\r\nDURATION:PT45M\r\nSUMMARY:Short\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn parses_a_realistic_feed() {
        let evs = parse_ics(GOOGLE_LIKE, Some("sub1"), "#fff");
        assert_eq!(evs.len(), 4, "cancelled event dropped, override kept: {:?}", evs.iter().map(|e| &e.id).collect::<Vec<_>>());
        let m = evs.iter().find(|e| e.id == "sub1-weekly@x").unwrap();
        assert_eq!(m.title, "Stand-up, daily");
        assert_eq!(m.description, "Real description", "VALARM description must not overwrite the event's");
        assert_eq!((m.date.as_str(), m.time.as_deref(), m.duration_minutes), ("2026-03-02", Some("10:00"), Some(90)));
        assert_eq!(m.reminder_minutes, Some(10));
        let r = m.recurrence.as_ref().unwrap();
        assert_eq!((r.freq.as_str(), r.interval, r.by_day.clone(), r.until.clone()), ("weekly", 1, Some(vec!["MO".into(), "WE".into()]), Some("2026-04-01".into())));
        assert!(m.exdates.contains(&"2026-03-09".to_string()), "EXDATE kept");
        assert!(m.exdates.contains(&"2026-03-11".to_string()), "overridden occurrence excluded from the series");
        let moved = evs.iter().find(|e| e.title == "Stand-up (moved)").unwrap();
        assert!(moved.recurrence.is_none() && moved.time.as_deref() == Some("15:00"));
        assert_ne!(moved.id, m.id, "override must not collide with the master id");
        assert_eq!(evs.iter().find(|e| e.title == "Holiday").unwrap().time, None);
        assert_eq!(evs.iter().find(|e| e.title == "Short").unwrap().duration_minutes, Some(45));
        assert!(evs.iter().all(|e| e.subscription_id.as_deref() == Some("sub1")));
    }

    #[test]
    fn export_then_import_round_trips() {
        let mut a = ev("a1");
        a.title = "Plan, review; \"go\"".into(); a.time = Some("09:30".into()); a.duration_minutes = Some(45);
        a.description = "line1\nline2".into(); a.reminder_minutes = Some(15);
        a.recurrence = Some(RecurrenceRule { freq: "weekly".into(), interval: 2, by_day: Some(vec!["MO".into(), "FR".into()]), until: Some("2026-12-31".into()), count: None });
        a.exdates = vec!["2026-03-27".into()];
        let mut b = ev("b2"); b.title = "Holiday".into();
        let mut sub = ev("s3"); sub.subscription_id = Some("subX".into());
        let ics = events_to_ics(&[a.clone(), b.clone(), sub]);
        assert!(ics.contains("\r\n") && ics.starts_with("BEGIN:VCALENDAR"));
        assert!(!ics.contains("s3@"), "subscription events are not exported");
        assert!(ics.lines().all(|l| l.len() <= 75 + 1), "lines folded");

        let mut store = Vec::new();
        let summary = import_into(&mut store, &ics).unwrap();
        assert_eq!((summary.added, summary.skipped), (2, 0));
        let a2 = store.iter().find(|e| e.title.starts_with("Plan")).unwrap();
        assert_eq!(a2.title, a.title);
        assert_eq!((a2.date.as_str(), a2.time.as_deref(), a2.duration_minutes), ("2026-03-15", Some("09:30"), Some(45)));
        assert_eq!(a2.description, "line1\nline2");
        assert_eq!(a2.reminder_minutes, Some(15));
        assert_eq!(a2.recurrence, a.recurrence);
        assert_eq!(a2.exdates, a.exdates);
        assert!(a2.subscription_id.is_none());
        // importing again is a no-op
        let again = import_into(&mut store, &ics).unwrap();
        assert_eq!((again.added, again.skipped), (0, 2));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn import_rejects_non_calendars() {
        let mut s = Vec::new();
        assert!(import_into(&mut s, "hello").is_err());
        assert!(import_into(&mut s, "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n").is_err());
    }

    #[test]
    fn folds_long_lines_without_splitting_utf8() {
        let long = format!("SUMMARY:{}", "zażółć gęślą jaźń ".repeat(12));
        let folded = fold_ics_line(&long);
        for l in folded.split("\r\n") { assert!(l.len() <= 76); }
        assert_eq!(unfold_ics_lines(&folded.replace("\r\n", "\n"))[0], long);
    }
}
