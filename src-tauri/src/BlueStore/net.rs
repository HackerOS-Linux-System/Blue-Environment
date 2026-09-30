use super::error::{StoreError, StoreResult};
use super::manifest::{parse_manifest, url_is_acceptable, validate_manifest_url, Manifest, PackageKind, MAX_MANIFEST_BYTES};
use futures_util::StreamExt;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

pub const STORE_BASE_URL: &str = "https://raw.githubusercontent.com/HackerOS-Linux-System/Blue-Environment/main/config/stores";
const MAX_INDEX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_ARCHIVE_BYTES: u64 = 200 * 1024 * 1024;

fn build_client(total_timeout: Duration) -> StoreResult<reqwest::Client> {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 5 {
            attempt.error("too many redirects")
        } else if !url_is_acceptable(attempt.url()) {
            attempt.error("redirect to a disallowed location")
        } else {
            attempt.follow()
        }
    });
    reqwest::Client::builder()
        .user_agent(concat!("Blue-Environment/", env!("CARGO_PKG_VERSION"), " (blue-store)"))
        .connect_timeout(Duration::from_secs(12))
        .timeout(total_timeout)
        .redirect(policy)
        .build()
        .map_err(|e| StoreError::new("network", "Could not initialise the HTTP client").detail(e.to_string()))
}

fn net_error(url: &Url, e: reqwest::Error) -> StoreError {
    let host = url.host_str().unwrap_or("?");
    let hint = if e.is_timeout() {
        "The server took too long to answer. Check your connection and try again."
    } else if e.is_connect() {
        "Could not connect. Check your internet connection (and that the host is reachable)."
    } else {
        "Check your internet connection and the URL."
    };
    StoreError::new("network", format!("Could not fetch {host}")).hint(hint).detail(e.to_string())
}

fn status_error(url: &Url, status: reqwest::StatusCode) -> StoreError {
    let mut e = StoreError::new("http_status", format!("{} answered HTTP {}", url.host_str().unwrap_or("server"), status.as_u16())).detail(url.to_string());
    if status.as_u16() == 404 {
        e = e.hint("The file does not exist at that URL (check the branch/tag and file name).");
    }
    e
}

async fn fetch_text(url: &Url, max: usize) -> StoreResult<String> {
    let client = build_client(Duration::from_secs(45))?;
    let resp = client.get(url.clone()).send().await.map_err(|e| net_error(url, e))?;
    if !resp.status().is_success() {
        return Err(status_error(url, resp.status()));
    }
    if resp.content_length().map(|l| l as usize > max).unwrap_or(false) {
        return Err(StoreError::new("too_large", "The response is unreasonably large"));
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| net_error(url, e))?;
        if buf.len() + chunk.len() > max {
            return Err(StoreError::new("too_large", "The response is unreasonably large"));
        }
        buf.extend_from_slice(&chunk);
    }
    String::from_utf8(buf).map_err(|_| StoreError::new("bad_encoding", "The response is not valid UTF-8 text"))
}

/// Downloads and validates a `blue.hk` from a store `downloadUrl` (or a URL
/// the person typed in).
pub async fn fetch_manifest(url_str: &str) -> StoreResult<(Manifest, Url)> {
    let url = validate_manifest_url(url_str)?;
    let text = fetch_text(&url, MAX_MANIFEST_BYTES).await?;
    let manifest = parse_manifest(&text, Some(&url))?;
    Ok((manifest, url))
}

/// Streams `url` into `dest` (created/truncated), reporting 0–100.
pub async fn download_archive(url: &Url, dest: &Path, progress: &(dyn Fn(u8, &str) + Send + Sync)) -> StoreResult<()> {
    let client = build_client(Duration::from_secs(600))?;
    let resp = client.get(url.clone()).send().await.map_err(|e| net_error(url, e))?;
    if !resp.status().is_success() {
        return Err(status_error(url, resp.status()));
    }
    let total = resp.content_length();
    if total.map(|t| t > MAX_ARCHIVE_BYTES).unwrap_or(false) {
        return Err(StoreError::new("too_large", "The package is larger than 200 MiB").hint("Blue Store refuses archives above 200 MiB."));
    }
    let tmp = dest.with_extension("blue.part");
    let mut file = std::fs::File::create(&tmp).map_err(|e| StoreError::io("creating the cache file", e).hint("Check that /var/cache/blue-environment (or ~/.cache/blue-environment) is writable."))?;
    let mut downloaded: u64 = 0;
    let mut last_pct = 0u8;
    let mut stream = resp.bytes_stream();
    let result: StoreResult<()> = async {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| net_error(url, e))?;
            downloaded += chunk.len() as u64;
            if downloaded > MAX_ARCHIVE_BYTES {
                return Err(StoreError::new("too_large", "The package is larger than 200 MiB"));
            }
            file.write_all(&chunk).map_err(|e| StoreError::io("writing the cache file", e))?;
            if let Some(t) = total.filter(|t| *t > 0) {
                let pct = ((downloaded * 100) / t).min(100) as u8;
                if pct != last_pct {
                    last_pct = pct;
                    progress(pct, "Downloading…");
                }
            }
        }
        file.flush().map_err(|e| StoreError::io("writing the cache file", e))?;
        Ok(())
    }
    .await;
    drop(file);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, dest).map_err(|e| StoreError::io("finalising the cache file", e))
}

// ── community indexes (apps-store.json …) ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    pub name: String,
    pub description: String,
    pub author: String,
    pub icon: String,
    pub preview: String,
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexResult {
    pub kind: PackageKind,
    pub entries: Vec<IndexEntry>,
    /// Entries that were skipped and why (invalid URL, wrong file name …).
    pub warnings: Vec<String>,
    pub source_url: String,
}

/// The community JSON files are hand-edited on GitHub and routinely end up
/// with trailing commas (`"downloadUrl": "",\n}`), which strict JSON
/// rejects. Drop any comma that is directly followed (ignoring whitespace)
/// by `}` or `]` — outside of string literals.
pub fn strip_trailing_commas(input: &str) -> String {
    let chars: Vec<char> = input.trim_start_matches('\u{feff}').chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut in_str = false;
    let mut escaped = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
            out.push(c);
        } else if c == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                // skip this comma
            } else {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

pub fn parse_index(kind: PackageKind, text: &str, source_url: &str) -> StoreResult<IndexResult> {
    let cleaned = strip_trailing_commas(text);
    let json: serde_json::Value = serde_json::from_str(&cleaned)
        .map_err(|e| StoreError::new("index_parse", "The community index is not valid JSON").detail(e.to_string()))?;
    let list = kind
        .index_keys()
        .iter()
        .find_map(|k| json.get(*k).and_then(|v| v.as_array()))
        .cloned()
        .unwrap_or_default();

    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for (i, item) in list.iter().enumerate() {
        let name = s(item, "name");
        let url = s(item, "downloadUrl");
        if name.is_empty() && url.is_empty() {
            continue; // the empty placeholder entry shipped in the template
        }
        let label = if name.is_empty() { format!("entry #{}", i + 1) } else { name.clone() };
        if name.is_empty() {
            warnings.push(format!("{label}: missing `name`"));
            continue;
        }
        match validate_manifest_url(&url) {
            Ok(u) => entries.push(IndexEntry {
                name,
                description: s(item, "description"),
                author: s(item, "author"),
                icon: s(item, "icon"),
                preview: s(item, "preview"),
                download_url: u.to_string(),
            }),
            Err(e) => warnings.push(format!("{label}: {}", e.message)),
        }
    }
    Ok(IndexResult { kind, entries, warnings, source_url: source_url.to_string() })
}

pub async fn fetch_index(kind: PackageKind) -> StoreResult<IndexResult> {
    let base = if !super::install::is_root() {
        std::env::var("BLUE_STORE_BASE_URL").unwrap_or_else(|_| STORE_BASE_URL.to_string())
    } else {
        STORE_BASE_URL.to_string()
    };
    let url_str = format!("{}/{}", base.trim_end_matches('/'), kind.store_file());
    let url = Url::parse(&url_str).map_err(|e| StoreError::new("bad_url", "Invalid store URL").detail(e.to_string()))?;
    let text = fetch_text(&url, MAX_INDEX_BYTES).await?;
    parse_index(kind, &text, &url_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_commas_are_removed_but_strings_are_untouched() {
        let raw = r#"{ "a": "x,}", "list": [ { "n": "", "u": "", }, ], }"#;
        let cleaned = strip_trailing_commas(raw);
        let v: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(v["a"], "x,}");
        assert_eq!(v["list"][0]["n"], "");
    }

    #[test]
    fn empty_placeholder_entry_is_skipped_and_plugins_key_works_for_apps() {
        let text = r#"{
          "description": "Community applications for Blue Environment.",
          "plugins": [ { "name": "", "description": "", "author": "", "icon": "", "downloadUrl": "", } ]
        }"#;
        let r = parse_index(PackageKind::App, text, "x").unwrap();
        assert!(r.entries.is_empty());
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn entries_are_validated_and_bad_ones_reported() {
        let text = r#"{ "apps": [
          { "name": "Good", "downloadUrl": "https://github.com/o/r/blob/main/blue.hk", "author": "me" },
          { "name": "WrongFile", "downloadUrl": "https://example.com/app.zip" },
          { "name": "Insecure", "downloadUrl": "http://example.com/blue.hk" }
        ] }"#;
        let r = parse_index(PackageKind::App, text, "x").unwrap();
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].download_url, "https://raw.githubusercontent.com/o/r/main/blue.hk");
        assert_eq!(r.warnings.len(), 2);
    }
}
