use serde::Serialize;
use std::fs;
use std::path::Path;

/// Estimates above this are sensor noise (a near-zero rate), not an ETA.
const MAX_REASONABLE_MINUTES: f64 = 99.0 * 60.0;
/// Below this the "power draw" is just noise too (watts).
const MIN_REPORTED_WATTS: f64 = 0.05;

/// Lightweight battery probe. Unlike `get_battery_info` (which pretends a
/// desktop is "100 % charging"), `present` says whether a battery exists at all
/// so the UI can hide the indicator on desktops.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatteryStatus {
    pub present: bool,
    pub percentage: f32,
    pub charging: bool,
    /// "Charging" | "Discharging" | "Full" | "Not charging" | "Unknown"
    pub status: String,
    /// A mains / USB-PD supply reports `online` — i.e. the machine is plugged in.
    pub ac_online: bool,
    /// Current charge (+) / discharge (−) magnitude in watts, when the driver exposes it.
    pub power_watts: Option<f32>,
    /// Minutes until full (while charging) or empty (while discharging).
    pub minutes_remaining: Option<u32>,
    /// Full-charge capacity relative to the design capacity, percent.
    pub health_percent: Option<f32>,
}

impl BatteryStatus {
    fn none(ac_online: bool) -> Self {
        BatteryStatus {
            present: false,
            percentage: 0.0,
            charging: false,
            status: "Unknown".into(),
            ac_online,
            power_watts: None,
            minutes_remaining: None,
            health_percent: None,
        }
    }
}

fn read_trim(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn read_num(path: &Path) -> Option<f64> {
    read_trim(path)?.parse::<f64>().ok()
}

fn is_peripheral(dir: &Path) -> bool {
    // Wireless mice, headsets, … report scope "Device".
    read_trim(&dir.join("scope")).map(|s| s == "Device").unwrap_or(false)
}

/// Is any non-battery supply (mains, USB, USB-PD…) online?
fn any_adapter_online(dirs: &[std::path::PathBuf]) -> bool {
    dirs.iter().any(|d| {
        let kind = read_trim(&d.join("type")).unwrap_or_default();
        kind != "Battery" && !is_peripheral(d) && read_trim(&d.join("online")).as_deref() == Some("1")
    })
}

/// Minutes until full/empty from the amounts and the (positive) rate, both in
/// the same unit family (µWh & µW, or µAh & µA).
fn minutes_for(status: &str, now: f64, full: f64, rate: f64) -> Option<u32> {
    if rate <= 0.0 {
        return None;
    }
    let remaining = match status {
        "Discharging" => now,
        "Charging" => (full - now).max(0.0),
        _ => return None,
    };
    let minutes = remaining / rate * 60.0;
    if !minutes.is_finite() || minutes > MAX_REASONABLE_MINUTES {
        return None;
    }
    Some(minutes.round() as u32)
}

/// Reads the first system battery under `root` (normally `/sys/class/power_supply`).
pub fn read_battery_status_in(root: &Path) -> BatteryStatus {
    let Ok(entries) = fs::read_dir(root) else { return BatteryStatus::none(false) };
    let mut dirs: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    dirs.sort(); // BAT0 before BAT1, stable between polls
    let ac_online = any_adapter_online(&dirs);

    for dir in &dirs {
        if read_trim(&dir.join("type")).as_deref() != Some("Battery") || is_peripheral(dir) {
            continue;
        }
        let Some(percentage) = read_num(&dir.join("capacity")) else { continue };
        let status = read_trim(&dir.join("status")).unwrap_or_else(|| "Unknown".into());
        let charging = status == "Charging";

        // Two families of drivers: energy (µWh / µW) and charge (µAh / µA).
        let voltage = read_num(&dir.join("voltage_now")); // µV
        let power_uw = read_num(&dir.join("power_now")).map(f64::abs);
        let current_ua = read_num(&dir.join("current_now")).map(f64::abs);

        let energy = (read_num(&dir.join("energy_now")), read_num(&dir.join("energy_full")), read_num(&dir.join("energy_full_design")));
        let charge = (read_num(&dir.join("charge_now")), read_num(&dir.join("charge_full")), read_num(&dir.join("charge_full_design")));

        // Watts: reported directly, or current × voltage.
        let watts = power_uw
            .map(|p| p / 1e6)
            .or_else(|| match (current_ua, voltage) {
                (Some(i), Some(v)) => Some(i * v / 1e12),
                _ => None,
            })
            .filter(|w| *w >= MIN_REPORTED_WATTS);

        let minutes_remaining = match (energy, charge) {
            ((Some(now), Some(full), _), _) if power_uw.is_some() => minutes_for(&status, now, full, power_uw.unwrap_or(0.0)),
            (_, (Some(now), Some(full), _)) if current_ua.is_some() => minutes_for(&status, now, full, current_ua.unwrap_or(0.0)),
            ((Some(now), Some(full), _), _) if watts.is_some() => minutes_for(&status, now, full, watts.unwrap_or(0.0) * 1e6),
            _ => None,
        };

        let health_percent = match (energy, charge) {
            ((_, Some(full), Some(design)), _) if design > 0.0 => Some(full / design * 100.0),
            (_, (_, Some(full), Some(design))) if design > 0.0 => Some(full / design * 100.0),
            _ => None,
        }
        .map(|h| h.clamp(0.0, 100.0) as f32);

        return BatteryStatus {
            present: true,
            percentage: percentage as f32,
            charging,
            status,
            ac_online,
            power_watts: watts.map(|w| w as f32),
            minutes_remaining,
            health_percent,
        };
    }
    BatteryStatus::none(ac_online)
}

pub fn read_battery_status() -> BatteryStatus {
    read_battery_status_in(Path::new("/sys/class/power_supply"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A throw-away `/sys/class/power_supply` built from `(device, [(file, value)])`.
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(name: &str, devices: &[(&str, &[(&str, &str)])]) -> Self {
            let root = std::env::temp_dir().join(format!("battery-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            for (dev, files) in devices {
                let d = root.join(dev);
                fs::create_dir_all(&d).unwrap();
                for (f, v) in *files {
                    fs::write(d.join(f), format!("{v}\n")).unwrap();
                }
            }
            Fixture(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const ADAPTER_ON: &[(&str, &str)] = &[("type", "Mains"), ("online", "1")];
    const ADAPTER_OFF: &[(&str, &str)] = &[("type", "Mains"), ("online", "0")];

    #[test]
    fn charging_laptop_with_energy_counters() {
        // 30 Wh missing of 50 Wh at 30 W → 60 min
        let f = Fixture::new("chg", &[
            ("AC", ADAPTER_ON),
            ("BAT0", &[("type", "Battery"), ("capacity", "40"), ("status", "Charging"),
                ("energy_now", "20000000"), ("energy_full", "50000000"), ("energy_full_design", "55000000"), ("power_now", "30000000")]),
        ]);
        let b = read_battery_status_in(&f.0);
        assert!(b.present && b.charging && b.ac_online);
        assert_eq!(b.percentage, 40.0);
        assert_eq!(b.status, "Charging");
        assert_eq!(b.minutes_remaining, Some(60));
        assert_eq!(b.power_watts, Some(30.0));
        assert!((b.health_percent.unwrap() - 90.909).abs() < 0.01);
    }

    #[test]
    fn discharging_laptop_estimates_time_left() {
        // 20 Wh at 10 W → 120 min
        let f = Fixture::new("dis", &[
            ("AC", ADAPTER_OFF),
            ("BAT0", &[("type", "Battery"), ("capacity", "40"), ("status", "Discharging"),
                ("energy_now", "20000000"), ("energy_full", "50000000"), ("power_now", "10000000")]),
        ]);
        let b = read_battery_status_in(&f.0);
        assert!(!b.charging && !b.ac_online);
        assert_eq!(b.minutes_remaining, Some(120));
        assert_eq!(b.power_watts, Some(10.0));
        assert_eq!(b.health_percent, None); // no design capacity reported
    }

    #[test]
    fn charge_counters_with_current_and_voltage() {
        // 2 Ah left at 1 A → 120 min; 1 A × 12 V = 12 W
        let f = Fixture::new("chargefam", &[
            ("BAT1", &[("type", "Battery"), ("capacity", "50"), ("status", "Discharging"),
                ("charge_now", "2000000"), ("charge_full", "4000000"), ("charge_full_design", "5000000"),
                ("current_now", "1000000"), ("voltage_now", "12000000")]),
        ]);
        let b = read_battery_status_in(&f.0);
        assert_eq!(b.minutes_remaining, Some(120));
        assert_eq!(b.power_watts, Some(12.0));
        assert_eq!(b.health_percent, Some(80.0));
    }

    #[test]
    fn full_and_plugged_in_has_no_estimate() {
        let f = Fixture::new("full", &[
            ("AC", ADAPTER_ON),
            ("BAT0", &[("type", "Battery"), ("capacity", "100"), ("status", "Full"),
                ("energy_now", "50000000"), ("energy_full", "50000000"), ("power_now", "0")]),
        ]);
        let b = read_battery_status_in(&f.0);
        assert!(!b.charging && b.ac_online);
        assert_eq!(b.status, "Full");
        assert_eq!(b.minutes_remaining, None);
        assert_eq!(b.power_watts, None); // 0 W is not a reading worth showing
    }

    #[test]
    fn plugged_in_but_not_charging() {
        let f = Fixture::new("nc", &[
            ("AC", ADAPTER_ON),
            ("BAT0", &[("type", "Battery"), ("capacity", "80"), ("status", "Not charging")]),
        ]);
        let b = read_battery_status_in(&f.0);
        assert!(b.present && !b.charging && b.ac_online);
        assert_eq!(b.status, "Not charging");
    }

    #[test]
    fn absurd_estimates_are_dropped() {
        // 20 Wh at 1 mW → years
        let f = Fixture::new("noise", &[
            ("BAT0", &[("type", "Battery"), ("capacity", "40"), ("status", "Discharging"),
                ("energy_now", "20000000"), ("energy_full", "50000000"), ("power_now", "1000")]),
        ]);
        assert_eq!(read_battery_status_in(&f.0).minutes_remaining, None);
    }

    #[test]
    fn peripheral_batteries_are_ignored() {
        let f = Fixture::new("periph", &[
            ("hid-mouse", &[("type", "Battery"), ("scope", "Device"), ("capacity", "55"), ("status", "Discharging")]),
        ]);
        assert!(!read_battery_status_in(&f.0).present);
    }

    #[test]
    fn desktop_without_battery() {
        let f = Fixture::new("desk", &[("AC", ADAPTER_ON)]);
        let b = read_battery_status_in(&f.0);
        assert!(!b.present);
        assert!(b.ac_online);
        assert!(!read_battery_status_in(&f.0.join("missing")).present);
    }

    #[test]
    fn peripheral_adapters_do_not_count_as_the_charger() {
        let f = Fixture::new("devac", &[
            ("usb-dev", &[("type", "USB"), ("scope", "Device"), ("online", "1")]),
            ("BAT0", &[("type", "Battery"), ("capacity", "70"), ("status", "Discharging")]),
        ]);
        assert!(!read_battery_status_in(&f.0).ac_online);
    }

    #[test]
    fn first_system_battery_wins_in_stable_order() {
        let f = Fixture::new("two", &[
            ("BAT1", &[("type", "Battery"), ("capacity", "11"), ("status", "Discharging")]),
            ("BAT0", &[("type", "Battery"), ("capacity", "99"), ("status", "Discharging")]),
        ]);
        assert_eq!(read_battery_status_in(&f.0).percentage, 99.0);
    }

    #[test]
    fn serialises_camel_case_for_the_frontend() {
        let f = Fixture::new("json", &[("BAT0", &[("type", "Battery"), ("capacity", "50"), ("status", "Charging")])]);
        let v = serde_json::to_value(read_battery_status_in(&f.0)).unwrap();
        for key in ["present", "percentage", "charging", "status", "acOnline", "powerWatts", "minutesRemaining", "healthPercent"] {
            assert!(v.get(key).is_some(), "missing {key}");
        }
    }
}
