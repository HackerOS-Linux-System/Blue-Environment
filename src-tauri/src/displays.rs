use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh: f64,
    pub preferred: bool,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    /// `normal`, `90`, `180`, `270`, `flipped`, `flipped-90`, …
    pub transform: String,
    pub modes: Vec<Mode>,
}

/// What the UI sends back for each output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutputConfig {
    pub name: String,
    pub enabled: bool,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub refresh: Option<f64>,
}

const TRANSFORMS: &[&str] = &["normal", "90", "180", "270", "flipped", "flipped-90", "flipped-180", "flipped-270"];

pub fn parse_wlr_randr_json(json: &str) -> Result<Vec<Output>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("Cannot parse wlr-randr output: {e}"))?;
    let arr = v.as_array().ok_or("wlr-randr output is not a list")?;
    let mut out = Vec::new();
    for o in arr {
        let name = o.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let modes = o
            .get("modes")
            .and_then(|m| m.as_array())
            .map(|ms| {
                ms.iter()
                    .map(|m| Mode {
                        width: m.get("width").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                        height: m.get("height").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                        refresh: m.get("refresh").and_then(|x| x.as_f64()).unwrap_or(0.0),
                        preferred: m.get("preferred").and_then(|x| x.as_bool()).unwrap_or(false),
                        current: m.get("current").and_then(|x| x.as_bool()).unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let pos = o.get("position");
        out.push(Output {
            description: o.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            enabled: o.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true),
            x: pos.and_then(|p| p.get("x")).and_then(|x| x.as_i64()).unwrap_or(0) as i32,
            y: pos.and_then(|p| p.get("y")).and_then(|x| x.as_i64()).unwrap_or(0) as i32,
            scale: o.get("scale").and_then(|x| x.as_f64()).unwrap_or(1.0),
            transform: o.get("transform").and_then(|x| x.as_str()).unwrap_or("normal").to_string(),
            modes,
            name,
        });
    }
    Ok(out)
}

/// Output names come from the compositor, but the layout comes from the
/// webview — keep them to what a connector name can look like.
fn valid_output_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 64 && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
}

/// One `wlr-randr` invocation for the whole layout.
pub fn build_args(layout: &[OutputConfig]) -> Result<Vec<String>, String> {
    if layout.is_empty() {
        return Err("Empty layout".into());
    }
    if !layout.iter().any(|o| o.enabled) {
        return Err("At least one display must stay enabled".into());
    }
    let mut a: Vec<String> = Vec::new();
    for o in layout {
        if !valid_output_name(&o.name) {
            return Err(format!("Invalid output name \"{}\"", o.name));
        }
        a.push("--output".into());
        a.push(o.name.clone());
        if !o.enabled {
            a.push("--off".into());
            continue;
        }
        if !(0.25..=4.0).contains(&o.scale) {
            return Err(format!("Scale {} is out of range", o.scale));
        }
        if !TRANSFORMS.contains(&o.transform.as_str()) {
            return Err(format!("Unknown rotation \"{}\"", o.transform));
        }
        a.push("--on".into());
        a.push("--pos".into());
        a.push(format!("{},{}", o.x, o.y));
        a.push("--scale".into());
        a.push(format!("{}", (o.scale * 1000.0).round() / 1000.0));
        a.push("--transform".into());
        a.push(o.transform.clone());
        if let (Some(w), Some(h)) = (o.width, o.height) {
            let mode = match o.refresh {
                Some(r) if r > 0.0 => format!("{w}x{h}@{r:.3}Hz"),
                _ => format!("{w}x{h}"),
            };
            a.push("--mode".into());
            a.push(mode);
        }
    }
    Ok(a)
}

/// Lays the enabled outputs out left-to-right without gaps or overlaps
/// (used by the "Arrange in a row" button and as a repair when the saved
/// layout no longer matches the connected monitors).
pub fn arrange_in_row(outputs: &mut [OutputConfig], sizes: &dyn Fn(&str) -> (u32, u32)) {
    let mut x = 0i32;
    for o in outputs.iter_mut().filter(|o| o.enabled) {
        let (w, h) = sizes(&o.name);
        let t = o.transform.trim_start_matches("flipped").trim_start_matches('-');
        let (mut lw, _lh) = (w as f64 / o.scale.max(0.25), h as f64 / o.scale.max(0.25));
        if t == "90" || t == "270" {
            lw = h as f64 / o.scale.max(0.25);
        }
        o.x = x;
        o.y = 0;
        x += lw.round() as i32;
    }
}

fn saved_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("Blue-Environment").join("displays.json"))
}

#[tauri::command(async)]
pub fn displays_list() -> Result<Vec<Output>, String> {
    let o = Command::new("wlr-randr")
        .arg("--json")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("wlr-randr is not available ({e}). Install the `wlr-randr` package."))?;
    if !o.status.success() {
        return Err(format!(
            "wlr-randr failed: {} (is this a wlroots-based session?)",
            String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("").trim()
        ));
    }
    parse_wlr_randr_json(&String::from_utf8_lossy(&o.stdout))
}

fn apply(layout: &[OutputConfig]) -> Result<(), String> {
    let args = build_args(layout)?;
    let o = Command::new("wlr-randr").args(&args).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("wlr-randr failed").trim().to_string())
    }
}

/// Applies the layout and remembers it for the next login.
#[tauri::command(async)]
pub fn displays_apply(layout: Vec<OutputConfig>) -> Result<(), String> {
    apply(&layout)?;
    if let Some(p) = saved_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, serde_json::to_string_pretty(&layout).unwrap_or_default());
    }
    Ok(())
}

/// Re-applies the saved layout if every output it mentions is still
/// connected. Returns whether anything was applied.
#[tauri::command(async)]
pub fn displays_restore() -> Result<bool, String> {
    let Some(p) = saved_path() else { return Ok(false) };
    let Ok(txt) = std::fs::read_to_string(p) else { return Ok(false) };
    let Ok(layout) = serde_json::from_str::<Vec<OutputConfig>>(&txt) else { return Ok(false) };
    let connected = displays_list()?;
    if layout.is_empty() || !layout.iter().all(|l| connected.iter().any(|c| c.name == l.name)) {
        return Ok(false);
    }
    apply(&layout)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
      {"name":"eDP-1","description":"Laptop","enabled":true,"position":{"x":0,"y":0},"scale":1.5,"transform":"normal",
       "modes":[{"width":2560,"height":1600,"refresh":60.0,"preferred":true,"current":true},{"width":1920,"height":1200,"refresh":60.0,"preferred":false,"current":false}]},
      {"name":"HDMI-A-1","description":"Dell","enabled":false,"position":{"x":1707,"y":0},"scale":1.0,"transform":"90","modes":[]}
    ]"#;

    #[test]
    fn parses_outputs() {
        let o = parse_wlr_randr_json(SAMPLE).unwrap();
        assert_eq!(o.len(), 2);
        assert_eq!(o[0].modes.len(), 2);
        assert!(o[0].modes[0].current);
        assert_eq!((o[1].x, o[1].enabled, o[1].transform.as_str()), (1707, false, "90"));
        assert!(parse_wlr_randr_json("nope").is_err());
    }

    fn cfg(name: &str, enabled: bool, x: i32) -> OutputConfig {
        OutputConfig { name: name.into(), enabled, x, y: 0, scale: 1.0, transform: "normal".into(), width: None, height: None, refresh: None }
    }

    #[test]
    fn builds_single_invocation() {
        let mut a = cfg("DP-1", true, 1920);
        a.width = Some(1920);
        a.height = Some(1080);
        a.refresh = Some(144.0);
        let args = build_args(&[cfg("eDP-1", true, 0), a, cfg("HDMI-A-1", false, 0)]).unwrap();
        let s = args.join(" ");
        assert!(s.starts_with("--output eDP-1 --on --pos 0,0 --scale 1 --transform normal"));
        assert!(s.contains("--output DP-1 --on --pos 1920,0"));
        assert!(s.contains("--mode 1920x1080@144.000Hz"));
        assert!(s.ends_with("--output HDMI-A-1 --off"));
    }

    #[test]
    fn rejects_bad_layouts() {
        assert!(build_args(&[]).is_err());
        assert!(build_args(&[cfg("eDP-1", false, 0)]).is_err(), "must keep one display on");
        assert!(build_args(&[cfg("x; rm", true, 0)]).is_err());
        let mut bad = cfg("DP-1", true, 0);
        bad.scale = 9.0;
        assert!(build_args(&[bad]).is_err());
        let mut rot = cfg("DP-1", true, 0);
        rot.transform = "45".into();
        assert!(build_args(&[rot]).is_err());
    }

    #[test]
    fn row_arrangement_uses_scaled_and_rotated_widths() {
        let mut v = vec![cfg("A", true, 5), cfg("B", true, 9), cfg("C", false, 3)];
        v[0].scale = 2.0;
        v[1].transform = "90".into();
        arrange_in_row(&mut v, &|_| (3840, 2160));
        assert_eq!(v[0].x, 0);
        assert_eq!(v[1].x, 1920); // A is 3840/2 wide
        assert_eq!(v[2].x, 3);    // disabled outputs are left alone
    }
}
