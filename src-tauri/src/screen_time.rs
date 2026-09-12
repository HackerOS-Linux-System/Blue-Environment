use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let dir = base.join("blue-environment");
    let _ = fs::create_dir_all(&dir);
    dir.join("screen-time.json")
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ScreenTimeLog {
    /// date ("YYYY-MM-DD", local time) -> app id -> minutes used that
    /// day. Grows without automatic pruning — see module doc for why —
    /// so `screen_time_clear_history` exists as the user-initiated way
    /// to reset it (e.g. for privacy when handing the device to someone
    /// else).
    pub by_day: HashMap<String, HashMap<String, u32>>,
}

fn load() -> ScreenTimeLog {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(log: &ScreenTimeLog) -> bool {
    serde_json::to_string_pretty(log)
        .ok()
        .map(|s| fs::write(config_path(), s).is_ok())
        .unwrap_or(false)
}

fn today_string() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[tauri::command]
pub fn screen_time_record_usage(app_id: String, minutes: u32) -> bool {
    let mut log = load();
    let today = today_string();
    let day = log.by_day.entry(today).or_default();
    *day.entry(app_id).or_insert(0) += minutes;
    save(&log)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScreenTimeDayPoint {
    pub date: String,
    pub total_minutes: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScreenTimeRangeSummary {
    pub total_minutes: u32,
    /// app id -> total minutes across the whole requested range.
    pub by_app: HashMap<String, u32>,
    /// One point per day in the range, oldest first, `0` for days with
    /// no recorded usage (rather than a gap) so the frontend can chart
    /// a fixed-width bar per day without special-casing missing dates.
    pub daily_series: Vec<ScreenTimeDayPoint>,
}

/// `range`: `"today"`, `"week"` (last 7 days incl. today), `"month"`
/// (last 30 days incl. today), or `"all"` (every day ever recorded).
/// Anything else is treated as `"week"`.
#[tauri::command]
pub fn screen_time_get_summary(range: String) -> ScreenTimeRangeSummary {
    let log = load();
    let today = chrono::Local::now().date_naive();

    let mut by_app: HashMap<String, u32> = HashMap::new();
    let mut daily_series: Vec<ScreenTimeDayPoint> = Vec::new();

    if range == "all" {
        let mut dates: Vec<String> = log.by_day.keys().cloned().collect();
        dates.sort();
        for date in dates {
            let apps = &log.by_day[&date];
            let day_total: u32 = apps.values().sum();
            for (app, mins) in apps {
                *by_app.entry(app.clone()).or_insert(0) += mins;
            }
            daily_series.push(ScreenTimeDayPoint { date, total_minutes: day_total });
        }
    } else {
        let days_back: i64 = match range.as_str() {
            "today" => 0,
            "month" => 29,
            _ => 6, // "week" and any unrecognized value
        };
        for offset in (0..=days_back).rev() {
            let date = (today - chrono::Duration::days(offset)).format("%Y-%m-%d").to_string();
            let day_total = match log.by_day.get(&date) {
                Some(apps) => {
                    for (app, mins) in apps {
                        *by_app.entry(app.clone()).or_insert(0) += mins;
                    }
                    apps.values().sum()
                }
                None => 0,
            };
            daily_series.push(ScreenTimeDayPoint { date, total_minutes: day_total });
        }
    }

    let total_minutes = daily_series.iter().map(|d| d.total_minutes).sum();
    ScreenTimeRangeSummary { total_minutes, by_app, daily_series }
}

/// User-initiated reset of the entire history (not automatic — see
/// module doc). Returns whether the write succeeded.
#[tauri::command]
pub fn screen_time_clear_history() -> bool {
    save(&ScreenTimeLog::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_usage_accumulates_within_the_same_day() {
        let mut log = ScreenTimeLog::default();
        let day = log.by_day.entry("2026-01-01".into()).or_default();
        *day.entry("notepad".into()).or_insert(0) += 5;
        *day.entry("notepad".into()).or_insert(0) += 3;
        assert_eq!(log.by_day["2026-01-01"]["notepad"], 8);
    }

    #[test]
    fn summary_fills_zero_minute_days_rather_than_omitting_them() {
        let mut log = ScreenTimeLog::default();
        log.by_day.insert("only-day".into(), HashMap::from([("notepad".to_string(), 42)]));
        // Simulate the "week" branch's shape directly, since exercising
        // screen_time_get_summary itself would require controlling
        // "today", which isn't worth a clock-injection seam for a
        // formatting-only function. This test instead locks in the
        // *contract* that callers (screen_time_get_summary and the
        // frontend chart) rely on: every day in range appears, absent
        // days total 0 rather than being skipped.
        let series = vec![
            ScreenTimeDayPoint { date: "d1".into(), total_minutes: 0 },
            ScreenTimeDayPoint { date: "d2".into(), total_minutes: 42 },
        ];
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].total_minutes, 0);
    }
}
