use super::error::{StoreError, StoreResult};
use hk_parser::{parse_hk, HkConfig, HkValue};
use reqwest::Url;
use serde::{Deserialize, Serialize};

pub const MANIFEST_FILE: &str = "blue.hk";
pub const RECEIPT_FILE: &str = ".blue-install.json";
pub const ARCHIVE_EXT: &str = ".blue";
pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;

/// Permission vocabulary understood by the Blue API bridge. Anything else in
/// a manifest is rejected so a typo can't silently mean "no restriction".
pub const KNOWN_PERMISSIONS: &[&str] =
    &["notifications", "clipboard", "storage", "network", "files", "system"];

/// Ids the shell itself uses (`AppId` in src/lib/types.ts). A community
/// package must never shadow a built-in application.
const RESERVED_IDS: &[&str] = &[
    "terminal", "ai_assistant", "explorer", "settings", "about", "calculator", "notepad",
    "mail", "camera", "external", "installer", "welcome", "help",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    App,
    Plugin,
    Theme,
}

impl PackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::App => "app",
            PackageKind::Plugin => "plugin",
            PackageKind::Theme => "theme",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "app" | "apps" => Some(PackageKind::App),
            "plugin" | "plugins" => Some(PackageKind::Plugin),
            "theme" | "themes" => Some(PackageKind::Theme),
            _ => None,
        }
    }
    /// File name of the community index for this kind, under `config/stores/`.
    pub fn store_file(self) -> &'static str {
        match self {
            PackageKind::App => "apps-store.json",
            PackageKind::Plugin => "plugins-store.json",
            PackageKind::Theme => "themes-store.json",
        }
    }
    /// Array keys accepted in the index JSON, most specific first. The
    /// published stores historically use `"plugins"` for every kind, so all
    /// three spellings are tolerated for all three kinds.
    pub fn index_keys(self) -> [&'static str; 4] {
        match self {
            PackageKind::App => ["apps", "items", "plugins", "themes"],
            PackageKind::Plugin => ["plugins", "items", "apps", "themes"],
            PackageKind::Theme => ["themes", "items", "plugins", "apps"],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub kind: PackageKind,
    pub version: String,
    pub author: String,
    pub description: String,
    pub icon: Option<String>,
    /// Absolute https URL of the `.blue` archive (already resolved against
    /// the URL `blue.hk` was fetched from). `None` only for the copy of
    /// `blue.hk` found *inside* an archive.
    pub archive: Option<String>,
    pub sha256: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub category: Option<String>,
    pub min_blue_version: Option<String>,
    /// app / plugin: JS entry file inside the archive.
    pub entry: String,
    pub style: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub min_width: Option<u32>,
    pub min_height: Option<u32>,
    /// plugin sub-kind ("panel-widget", "extension", ...), free text.
    pub plugin_kind: Option<String>,
    pub permissions: Vec<String>,
}

// ── field helpers ──────────────────────────────────────────────────────────

fn get<'a>(cfg: &'a HkConfig, section: &str, key: &str) -> Option<&'a HkValue> {
    cfg.get(section)?.as_map().ok()?.get(key)
}

fn get_str(cfg: &HkConfig, section: &str, key: &str) -> Option<String> {
    let v = get(cfg, section, key)?.as_string().ok()?;
    let v = v.trim().to_string();
    if v.is_empty() { None } else { Some(v) }
}

fn get_u32(cfg: &HkConfig, section: &str, key: &str) -> Option<u32> {
    match get(cfg, section, key)? {
        HkValue::Number(n) if *n >= 0.0 && *n <= u32::MAX as f64 => Some(*n as u32),
        HkValue::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn get_list(cfg: &HkConfig, section: &str, key: &str) -> Vec<String> {
    match get(cfg, section, key) {
        Some(HkValue::Array(items)) => items.iter().filter_map(|v| v.as_string().ok()).collect(),
        Some(other) => other.as_string().ok().into_iter().collect(),
        None => Vec::new(),
    }
}

fn required(cfg: &HkConfig, key: &str) -> StoreResult<String> {
    get_str(cfg, "package", key).ok_or_else(|| {
        StoreError::new("manifest_missing_field", format!("blue.hk: missing required field `{key}` in [package]"))
            .hint(format!("Add `-> {key} => …` under [package] in blue.hk."))
    })
}

// ── validation ─────────────────────────────────────────────────────────────

/// `^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9])?$`, no `..`, not reserved.
pub fn validate_id(id: &str) -> StoreResult<()> {
    let bad = |why: &str| {
        Err(StoreError::new("manifest_bad_id", format!("Invalid package id \"{id}\": {why}"))
            .hint("Use lowercase letters, digits, '.', '-' or '_' (reverse-DNS such as com.example.hello is recommended)."))
    };
    if id.is_empty() || id.len() > 64 {
        return bad("must be 1–64 characters long");
    }
    let bytes = id.as_bytes();
    let alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    if !alnum(bytes[0]) || !alnum(bytes[bytes.len() - 1]) {
        return bad("must start and end with a lowercase letter or digit");
    }
    if !bytes.iter().all(|&b| alnum(b) || b == b'.' || b == b'-' || b == b'_') {
        return bad("contains a forbidden character");
    }
    if id.contains("..") {
        return bad("must not contain \"..\"");
    }
    if RESERVED_IDS.contains(&id) || id.starts_with("blue_") {
        return bad("this id is reserved for built-in Blue applications");
    }
    Ok(())
}

pub fn validate_version(v: &str) -> StoreResult<()> {
    // x.y.z with optional -prerelease / +build
    let core = v.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<&str> = core.split('.').collect();
    let ok = parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()));
    let tail_ok = v.len() <= 40 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'+');
    if ok && tail_ok {
        Ok(())
    } else {
        Err(StoreError::new("manifest_bad_version", format!("Invalid version \"{v}\": expected semver x.y.z"))
            .hint("Write the version with three parts, e.g. 1.0.0 (a bare 1.0 is parsed as a number and loses precision)."))
    }
}

/// Numeric dotted comparison, pre-release/build metadata ignored.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (mut x, mut y) = (parse(a), parse(b));
    let n = x.len().max(y.len());
    x.resize(n, 0);
    y.resize(n, 0);
    x.cmp(&y)
}

/// A relative path that stays inside the package: no absolute paths, no
/// `..`, no backslashes, no NUL, no empty components.
pub fn is_safe_relative_path(p: &str) -> bool {
    if p.is_empty() || p.len() > 240 || p.starts_with('/') || p.contains('\\') || p.contains('\0') {
        return false;
    }
    p.split('/').all(|c| !c.is_empty() && c != ".." && c != ".")
}

fn validate_hex_sha256(h: &str) -> StoreResult<String> {
    let h = h.trim().to_ascii_lowercase();
    if h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(h)
    } else {
        Err(StoreError::new("manifest_bad_sha256", "blue.hk: `sha256` must be 64 hexadecimal characters"))
    }
}

/// Rejects hosts that could only ever be an attempt to reach the person's
/// own machine or LAN through the store (localhost, *.local, private/loopback
/// /link-local IP literals).
fn host_is_acceptable(url: &Url) -> bool {
    use std::net::IpAddr;
    let Some(host) = url.host_str() else { return false };
    let h = host.trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") || h.ends_with(".local") || h.ends_with(".internal") {
        return false;
    }
    if let Ok(ip) = h.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => !(v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified() || v4.is_broadcast()),
            IpAddr::V6(v6) => !(v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80),
        };
    }
    true
}

/// True for URLs the store may follow (used by the HTTP redirect policy):
/// https on a public host.
pub fn url_is_acceptable(url: &Url) -> bool {
    url.scheme() == "https" && host_is_acceptable(url)
}

/// Rewrites forge "blob" page URLs into their raw-file equivalents so a
/// person can paste the link they see in the browser:
///   github.com/o/r/blob/REF/path/blue.hk  -> raw.githubusercontent.com/o/r/REF/path/blue.hk
///   gitlab.com/o/r/-/blob/REF/path/blue.hk -> gitlab.com/o/r/-/raw/REF/path/blue.hk
pub fn normalize_source_url(input: &str) -> String {
    let Ok(url) = Url::parse(input.trim()) else { return input.trim().to_string() };
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    let segs: Vec<&str> = url.path().trim_start_matches('/').split('/').collect();
    if (host == "github.com" || host == "www.github.com") && segs.len() >= 5 && segs[2] == "blob" {
        return format!("https://raw.githubusercontent.com/{}/{}/{}", segs[0], segs[1], segs[3..].join("/"));
    }
    if host.contains("gitlab") {
        if let Some(pos) = segs.iter().position(|s| *s == "-") {
            if segs.get(pos + 1) == Some(&"blob") {
                let mut out: Vec<&str> = segs.clone();
                out[pos + 1] = "raw";
                let mut u = url.clone();
                u.set_path(&format!("/{}", out.join("/")));
                u.set_query(None);
                return u.to_string();
            }
        }
    }
    url.to_string()
}

/// Validates a `downloadUrl` from a store index (or one typed in by the
/// person): https only, acceptable host, and — the core rule of the format —
/// the file must be named exactly `blue.hk`.
pub fn validate_manifest_url(input: &str) -> StoreResult<Url> {
    let normalized = normalize_source_url(input);
    let url = Url::parse(&normalized)
        .map_err(|e| StoreError::new("bad_url", format!("Not a valid URL: {input}")).detail(e.to_string()))?;
    if url.scheme() != "https" {
        return Err(StoreError::new("bad_url", "Only https:// URLs are accepted").hint("Host the package on GitHub, GitLab or any https server."));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(StoreError::new("bad_url", "URLs with embedded credentials are not accepted"));
    }
    if !host_is_acceptable(&url) {
        return Err(StoreError::new("bad_url", "That host is not allowed (localhost / private network)"));
    }
    if input.len() > 2048 {
        return Err(StoreError::new("bad_url", "URL is too long"));
    }
    let last = url.path_segments().and_then(|s| s.last()).unwrap_or("");
    if last != MANIFEST_FILE {
        return Err(StoreError::new("bad_manifest_name", format!("The download URL must point to a file named \"{MANIFEST_FILE}\" (got \"{last}\")"))
            .hint("Blue Store only ever loads blue.hk — it in turn names the .blue archive to download."));
    }
    Ok(url)
}

/// Resolves and validates the `archive` value against the manifest URL.
pub fn resolve_archive_url(base: &Url, archive: &str) -> StoreResult<Url> {
    let url = base
        .join(archive.trim())
        .map_err(|e| StoreError::new("manifest_bad_archive", format!("blue.hk: invalid `archive` value \"{archive}\"")).detail(e.to_string()))?;
    if url.scheme() != "https" || !host_is_acceptable(&url) || !url.username().is_empty() {
        return Err(StoreError::new("manifest_bad_archive", "blue.hk: `archive` must be an https URL on a public host"));
    }
    let last = url.path_segments().and_then(|s| s.last()).unwrap_or("");
    if !last.ends_with(ARCHIVE_EXT) || last.len() <= ARCHIVE_EXT.len() {
        return Err(StoreError::new("manifest_bad_archive", format!("blue.hk: `archive` must end in \"{ARCHIVE_EXT}\" (got \"{last}\")"))
            .hint("Create the archive with `blue-dev pack`, it produces <id>-<version>.blue."));
    }
    Ok(url)
}

// ── parsing ────────────────────────────────────────────────────────────────

/// Parses a `blue.hk`. `source` is the URL it was fetched from — `Some` for a
/// remote manifest (then `archive` is required and resolved), `None` for the
/// copy found inside an archive.
pub fn parse_manifest(text: &str, source: Option<&Url>) -> StoreResult<Manifest> {
    if text.len() > MAX_MANIFEST_BYTES {
        return Err(StoreError::new("manifest_too_large", "blue.hk is unreasonably large"));
    }
    let cfg = parse_hk(text.trim_start_matches('\u{feff}')).map_err(|e| {
        StoreError::new("manifest_parse", "blue.hk could not be parsed").detail(e.render(text)).hint("Check the syntax: [section] headers and `-> key => value` lines.")
    })?;

    let id = required(&cfg, "id")?;
    validate_id(&id)?;
    let name = required(&cfg, "name")?;
    if name.chars().count() > 80 {
        return Err(StoreError::new("manifest_bad_field", "blue.hk: `name` is longer than 80 characters"));
    }
    let kind_raw = required(&cfg, "type")?;
    let kind = PackageKind::parse(&kind_raw).ok_or_else(|| {
        StoreError::new("manifest_bad_type", format!("blue.hk: unknown package type \"{kind_raw}\"")).hint("`type` must be one of: app, plugin, theme.")
    })?;
    let version = required(&cfg, "version")?;
    validate_version(&version)?;

    let archive = match (get_str(&cfg, "package", "archive"), source) {
        (Some(a), Some(base)) => Some(resolve_archive_url(base, &a)?.to_string()),
        (Some(a), None) => Some(a),
        (None, Some(_)) => {
            return Err(StoreError::new("manifest_missing_field", "blue.hk: missing required field `archive` in [package]")
                .hint("Set `-> archive => <id>-<version>.blue` (absolute https URL or a path relative to blue.hk)."))
        }
        (None, None) => None,
    };

    let sha256 = match get_str(&cfg, "package", "sha256") {
        Some(h) => Some(validate_hex_sha256(&h)?),
        None => None,
    };

    let icon = get_str(&cfg, "package", "icon");
    if let Some(i) = &icon {
        let is_lucide = i.bytes().all(|b| b.is_ascii_alphanumeric());
        if !is_lucide && !is_safe_relative_path(i) {
            return Err(StoreError::new("manifest_bad_field", format!("blue.hk: `icon` \"{i}\" is neither a Lucide icon name nor a safe relative path")));
        }
    }

    let sect = match kind {
        PackageKind::App => "app",
        PackageKind::Plugin => "plugin",
        PackageKind::Theme => "theme",
    };
    let entry = get_str(&cfg, sect, "entry").unwrap_or_else(|| "index.js".to_string());
    let style = get_str(&cfg, sect, "style");
    for (label, p) in [("entry", Some(&entry)), ("style", style.as_ref())] {
        if let Some(p) = p {
            if !is_safe_relative_path(p) {
                return Err(StoreError::new("manifest_bad_field", format!("blue.hk: [{sect}] `{label}` must be a relative path inside the archive")));
            }
        }
    }

    let permissions = get_list(&cfg, "permissions", "list");
    for p in &permissions {
        if !KNOWN_PERMISSIONS.contains(&p.as_str()) {
            return Err(StoreError::new("manifest_bad_permission", format!("blue.hk: unknown permission \"{p}\""))
                .hint(format!("Known permissions: {}.", KNOWN_PERMISSIONS.join(", "))));
        }
    }

    Ok(Manifest {
        id,
        name,
        kind,
        version,
        author: get_str(&cfg, "package", "author").unwrap_or_else(|| "Unknown".to_string()),
        description: get_str(&cfg, "package", "description").unwrap_or_default(),
        icon,
        archive,
        sha256,
        homepage: get_str(&cfg, "package", "homepage"),
        license: get_str(&cfg, "package", "license"),
        category: get_str(&cfg, "package", "category"),
        min_blue_version: get_str(&cfg, "package", "min_blue_version"),
        entry,
        style,
        width: get_u32(&cfg, "app", "width").map(|v| v.clamp(200, 4000)),
        height: get_u32(&cfg, "app", "height").map(|v| v.clamp(150, 4000)),
        min_width: get_u32(&cfg, "app", "min_width"),
        min_height: get_u32(&cfg, "app", "min_height"),
        plugin_kind: get_str(&cfg, "plugin", "kind"),
        permissions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"! demo
[package]
-> id => com.example.hello
-> name => Hello
-> type => app
-> version => 1.2.3
-> author => Jane
-> description => Tiny
-> icon => Sparkles
-> archive => hello-1.2.3.blue
-> sha256 => 3f2a9c1d3f2a9c1d3f2a9c1d3f2a9c1d3f2a9c1d3f2a9c1d3f2a9c1d3f2a9c1d

[app]
-> entry => index.js
-> width => 900

[permissions]
-> list => ["notifications", "storage"]
"#;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn parses_a_complete_manifest_and_resolves_relative_archive() {
        let m = parse_manifest(GOOD, Some(&url("https://raw.githubusercontent.com/u/r/main/blue.hk"))).unwrap();
        assert_eq!(m.id, "com.example.hello");
        assert_eq!(m.kind, PackageKind::App);
        assert_eq!(m.archive.as_deref(), Some("https://raw.githubusercontent.com/u/r/main/hello-1.2.3.blue"));
        assert_eq!(m.width, Some(900));
        assert_eq!(m.permissions, vec!["notifications", "storage"]);
    }

    #[test]
    fn manifest_url_must_be_named_blue_hk() {
        assert!(validate_manifest_url("https://github.com/u/r/blob/main/blue.hk").is_ok());
        assert!(validate_manifest_url("https://example.com/other.json").is_err());
        assert!(validate_manifest_url("https://example.com/blue.hk.evil").is_err());
        assert!(validate_manifest_url("http://example.com/blue.hk").is_err());
        assert!(validate_manifest_url("https://localhost/blue.hk").is_err());
        assert!(validate_manifest_url("https://192.168.1.5/blue.hk").is_err());
        assert!(validate_manifest_url("https://user:pw@example.com/blue.hk").is_err());
    }

    #[test]
    fn blob_urls_are_rewritten_to_raw() {
        assert_eq!(
            normalize_source_url("https://github.com/o/r/blob/main/pkg/blue.hk"),
            "https://raw.githubusercontent.com/o/r/main/pkg/blue.hk"
        );
        assert_eq!(
            normalize_source_url("https://gitlab.com/o/r/-/blob/main/blue.hk"),
            "https://gitlab.com/o/r/-/raw/main/blue.hk"
        );
    }

    #[test]
    fn archive_must_end_in_dot_blue() {
        let base = url("https://example.com/x/blue.hk");
        assert!(resolve_archive_url(&base, "a-1.0.0.blue").is_ok());
        assert!(resolve_archive_url(&base, "a-1.0.0.tar.zst").is_err());
        assert!(resolve_archive_url(&base, "http://example.com/a.blue").is_err());
        assert!(resolve_archive_url(&base, ".blue").is_err());
    }

    #[test]
    fn ids_versions_and_paths_are_validated() {
        assert!(validate_id("com.example.hello").is_ok());
        assert!(validate_id("Hello").is_err());
        assert!(validate_id("a/b").is_err());
        assert!(validate_id("..").is_err());
        assert!(validate_id("terminal").is_err());
        assert!(validate_id("blue_code").is_err());
        assert!(validate_version("1.0.0").is_ok());
        assert!(validate_version("1.0").is_err());
        assert!(validate_version("1.0.0-beta.1").is_ok());
        assert!(is_safe_relative_path("dist/index.js"));
        assert!(!is_safe_relative_path("../x"));
        assert!(!is_safe_relative_path("/etc/passwd"));
        assert!(!is_safe_relative_path("a//b"));
    }

    #[test]
    fn unknown_permission_and_missing_fields_are_reported() {
        let bad = GOOD.replace("\"storage\"", "\"root\"");
        let e = parse_manifest(&bad, Some(&url("https://example.com/blue.hk"))).unwrap_err();
        assert_eq!(e.code, "manifest_bad_permission");
        let e = parse_manifest("[package]\n-> name => x\n", None).unwrap_err();
        assert_eq!(e.code, "manifest_missing_field");
    }

    #[test]
    fn version_comparison_is_numeric() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_versions("1.10.0", "1.9.0"), Greater);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Equal);
        assert_eq!(compare_versions("0.9.9", "1.0.0"), Less);
    }
}
