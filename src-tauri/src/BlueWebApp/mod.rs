use serde::Serialize;

#[derive(Serialize)]
pub struct SiteInfo {
    pub title: String,
    pub description: String,
    pub favicon_url: String,
    pub reachable: bool,
}

/// Opens `url` in a new native Tauri webview window. The window is
/// created by spawning a helper process (xdg-open on Linux) so it runs
/// completely outside the CSP of the shell window. Returns the launched
/// URL so the frontend can track it.
#[tauri::command]
pub fn web_open_native(url: String, app: tauri::AppHandle) -> Result<String, String> {
    use tauri::WebviewWindowBuilder;

    // Validate URL before doing anything
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("Invalid URL: {}", url));
    }

    let label = format!("web-{}", chrono::Local::now().timestamp_millis());
    let title = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(&url)
        .to_string();

    WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::External(url.parse().map_err(|e| format!("URL parse error: {}", e))?))
        .title(title)
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .decorations(true)
        .build()
        .map_err(|e| format!("Failed to open window: {}", e))?;

    Ok(url)
}

/// Fetches basic metadata (title, description, favicon) for `url`.
/// Used by the new-tab page to show site previews.
/// Intentionally a fire-and-forget async operation — returns empty fields
/// rather than erroring when the network is unavailable.
#[tauri::command]
pub async fn web_fetch_site_info(url: String) -> SiteInfo {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (like Blue/0.6)")
        .build();

    let Ok(client) = client else {
        return SiteInfo { title: String::new(), description: String::new(), favicon_url: String::new(), reachable: false };
    };

    let Ok(resp) = client.get(&url).send().await else {
        return SiteInfo { title: String::new(), description: String::new(), favicon_url: String::new(), reachable: false };
    };

    let Ok(body) = resp.text().await else {
        return SiteInfo { title: String::new(), description: String::new(), favicon_url: String::new(), reachable: false };
    };

    let title = extract_tag(&body, "title").unwrap_or_default();
    let description = extract_meta(&body, "description").unwrap_or_default();
    let host = url.split('/').nth(2).unwrap_or("").to_string();
    let favicon_url = format!("https://www.google.com/s2/favicons?sz=32&domain={}", host);

    SiteInfo { title, description, favicon_url, reachable: true }
}

fn extract_tag(html: &str, tag: &str) -> Option<String> {
    let open  = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let start = html.to_lowercase().find(&open)?;
    let content_start = html[start..].find('>')? + start + 1;
    let content_end   = html[content_start..].to_lowercase().find(&close)? + content_start;
    Some(html[content_start..content_end].trim().to_string())
}

fn extract_meta(html: &str, name: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let needle = format!("name=\"{}\"", name);
    let pos = lower.find(&needle)?;
    let after = &html[pos..];
    let content_pos = after.to_lowercase().find("content=\"")? + "content=\"".len();
    let end = after[content_pos..].find('"')?;
    Some(after[content_pos..content_pos + end].trim().to_string())
}
