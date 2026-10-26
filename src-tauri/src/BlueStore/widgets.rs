use serde::Serialize;
use serde_json::{Map, Value};

/// Widget engines the shell can render (mirrors `WidgetType` in `widgetModel.ts`).
pub const WIDGET_TYPES: &[&str] = &[
    "clock", "weather", "notes", "system", "calendar", "worldclock", "countdown", "quote", "links", "todo", "timer",
];
const BUILTIN_TYPES: &[&str] = &["clock", "weather", "notes", "system", "calendar"];
const MAX_ENTRIES: usize = 500;
const MAX_CONFIG_KEYS: usize = 12;
const MAX_CONFIG_STRING: usize = 2000;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WidgetEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub icon: String,
    pub category: String,
    pub version: String,
    #[serde(rename = "type")]
    pub widget_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<WidgetSize>,
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub config: Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub builtin: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct WidgetSize {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetIndexResult {
    pub entries: Vec<WidgetEntry>,
    pub warnings: Vec<String>,
    pub source_url: String,
}

fn text(v: Option<&Value>, max: usize) -> String {
    v.and_then(Value::as_str).map(|s| s.trim().chars().take(max).collect()).unwrap_or_default()
}

fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    (2..=48).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn valid_accent(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn clamp_dim(v: Option<&Value>, lo: u64, hi: u64) -> Option<u32> {
    let n = v?.as_f64()?;
    if !n.is_finite() {
        return None;
    }
    Some((n.round().max(lo as f64).min(hi as f64)) as u32)
}

/// Validates one raw entry.
pub fn sanitize_entry(raw: &Value) -> Result<WidgetEntry, String> {
    let o = raw.as_object().ok_or("entry is not an object")?;
    let id = text(o.get("id"), 64).to_lowercase();
    if !valid_id(&id) {
        return Err(format!("\"{}\": invalid id (use a-z, 0-9 and dashes)", text(o.get("id"), 40)));
    }
    let name = text(o.get("name"), 60);
    if name.is_empty() {
        return Err(format!("{id}: missing name"));
    }
    let widget_type = text(o.get("type"), 32);
    if !WIDGET_TYPES.contains(&widget_type.as_str()) {
        return Err(format!("{id}: unknown widget type \"{widget_type}\""));
    }

    let size = o.get("size").and_then(Value::as_object).and_then(|s| {
        Some(WidgetSize { w: clamp_dim(s.get("w"), 140, 800)?, h: clamp_dim(s.get("h"), 90, 800)? })
    });

    let mut config = Map::new();
    if let Some(cfg) = o.get("config").and_then(Value::as_object) {
        for (k, v) in cfg.iter().take(MAX_CONFIG_KEYS) {
            let key_ok = k.len() <= 32 && k.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !key_ok {
                continue;
            }
            match v {
                Value::String(s) => { config.insert(k.clone(), Value::String(s.chars().take(MAX_CONFIG_STRING).collect())); }
                Value::Number(_) | Value::Bool(_) => { config.insert(k.clone(), v.clone()); }
                _ => {}
            }
        }
    }

    let icon: String = text(o.get("icon"), 16).chars().take(4).collect();
    let author = text(o.get("author"), 60);
    let category = text(o.get("category"), 30);
    let version = text(o.get("version"), 20);
    let builtin = o.get("builtin").and_then(Value::as_bool).unwrap_or(false) && BUILTIN_TYPES.contains(&widget_type.as_str());

    Ok(WidgetEntry {
        id,
        name,
        description: text(o.get("description"), 300),
        author: if author.is_empty() { "Unknown".into() } else { author },
        icon: if icon.is_empty() { "🧩".into() } else { icon },
        category: if category.is_empty() { "Productivity".into() } else { category },
        version: if version.is_empty() { "1.0.0".into() } else { version },
        widget_type,
        size,
        config,
        accent: o.get("accent").and_then(Value::as_str).filter(|a| valid_accent(a)).map(str::to_lowercase),
        tags: o
            .get("tags")
            .and_then(Value::as_array)
            .map(|t| t.iter().filter_map(Value::as_str).map(|s| s.trim().chars().take(24).collect::<String>()).filter(|s| !s.is_empty()).take(6).collect())
            .unwrap_or_default(),
        builtin,
    })
}

/// Parses the whole index document (`{ "widgets": [...] }` or a bare array).
/// Bad entries and duplicate ids are skipped and reported in `warnings`.
pub fn parse_widget_index(raw_text: &str, source_url: &str, clean: impl Fn(&str) -> String) -> Result<WidgetIndexResult, String> {
    let doc: Value = serde_json::from_str(&clean(raw_text)).map_err(|e| format!("the widget index is not valid JSON: {e}"))?;
    let list = match &doc {
        Value::Array(a) => a,
        Value::Object(o) => o.get("widgets").and_then(Value::as_array).ok_or("the widget index has no \"widgets\" list")?,
        _ => return Err("the widget index has an unexpected shape".into()),
    };
    let mut entries: Vec<WidgetEntry> = Vec::new();
    let mut warnings = Vec::new();
    for item in list.iter().take(MAX_ENTRIES) {
        match sanitize_entry(item) {
            Ok(e) if entries.iter().any(|x| x.id == e.id) => warnings.push(format!("{}: duplicate id", e.id)),
            Ok(e) => entries.push(e),
            Err(w) => warnings.push(w),
        }
    }
    Ok(WidgetIndexResult { entries, warnings, source_url: source_url.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ident(s: &str) -> String { s.to_string() }

    #[test]
    fn accepts_a_good_entry_and_fills_defaults() {
        let e = sanitize_entry(&json!({"id":"world-clock","name":"World clock","type":"worldclock","config":{"zones":"Europe/Warsaw","hour12":false,"n":3}})).unwrap();
        assert_eq!(e.author, "Unknown");
        assert_eq!(e.icon, "🧩");
        assert_eq!(e.config.len(), 3);
        assert!(!e.builtin);
    }

    #[test]
    fn rejects_bad_ids_types_and_names() {
        assert!(sanitize_entry(&json!({"id":"Bad Id!","name":"x","type":"clock"})).is_err());
        assert!(sanitize_entry(&json!({"id":"ok-id","name":"","type":"clock"})).is_err());
        assert!(sanitize_entry(&json!({"id":"ok-id","name":"x","type":"shell"})).is_err());
        assert!(sanitize_entry(&json!("nope")).is_err());
    }

    #[test]
    fn clamps_sizes_and_drops_unsafe_config() {
        let e = sanitize_entry(&json!({"id":"big","name":"Big","type":"notes","size":{"w":99999,"h":1},"config":{"ok":"x","bad key":"y","nested":{"a":1},"list":[1]},"accent":"#ABCDEF"})).unwrap();
        assert_eq!(e.size, Some(WidgetSize { w: 800, h: 90 }));
        assert_eq!(e.config.keys().collect::<Vec<_>>(), vec!["ok"]);
        assert_eq!(e.accent.as_deref(), Some("#abcdef"));
    }

    #[test]
    fn only_builtin_engines_can_be_flagged_builtin() {
        assert!(sanitize_entry(&json!({"id":"c1","name":"C","type":"clock","builtin":true})).unwrap().builtin);
        assert!(!sanitize_entry(&json!({"id":"t1","name":"T","type":"todo","builtin":true})).unwrap().builtin);
    }

    #[test]
    fn index_skips_bad_and_duplicate_entries() {
        let r = parse_widget_index(r#"{"widgets":[{"id":"a1","name":"A","type":"todo"},{"id":"a1","name":"B","type":"todo"},{"id":"?","name":"C","type":"todo"}]}"#, "u", ident).unwrap();
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.warnings.len(), 2);
        assert!(parse_widget_index("[1,2", "u", ident).is_err());
        assert!(parse_widget_index(r#"{"x":1}"#, "u", ident).is_err());
        assert_eq!(parse_widget_index(r#"[{"id":"zz","name":"Z","type":"timer"}]"#, "u", ident).unwrap().entries.len(), 1);
    }
}
