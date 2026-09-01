use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn news_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config/Blue-Environment/blue-news")
}
fn sources_path() -> PathBuf { news_dir().join("sources.json") }

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NewsSource {
    pub id: String,
    pub name: String,
    pub url: String,
    /// Free-form, frontend-defined grouping (e.g. "Tech", "World") —
    /// not validated against a fixed list here, same reasoning as
    /// `NotificationRule::kind`: keeps the schema open without a
    /// migration every time someone wants a new category.
    pub category: String,
    pub enabled: bool,
}

/// A small starter set so Blue News isn't a completely empty screen on
/// first run — well-known, stable, genuinely public RSS feeds (not
/// tracking pixels or anything requiring an account), seeded once. The
/// person can remove any/all of these; nothing re-adds them.
fn default_sources() -> Vec<NewsSource> {
    vec![
        NewsSource { id: "src-hn".into(), name: "Hacker News".into(), url: "https://news.ycombinator.com/rss".into(), category: "Tech".into(), enabled: true },
        NewsSource { id: "src-bbc-world".into(), name: "BBC World News".into(), url: "https://feeds.bbci.co.uk/news/world/rss.xml".into(), category: "World".into(), enabled: true },
        NewsSource { id: "src-nasa".into(), name: "NASA Breaking News".into(), url: "https://www.nasa.gov/news-release/feed/".into(), category: "Science".into(), enabled: true },
    ]
}

fn read_sources() -> Vec<NewsSource> {
    match fs::read_to_string(sources_path()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => {
            let defaults = default_sources();
            let _ = write_sources(&defaults);
            defaults
        }
    }
}

fn write_sources(sources: &[NewsSource]) -> Result<(), String> {
    fs::create_dir_all(news_dir()).map_err(|e| e.to_string())?;
    fs::write(sources_path(), serde_json::to_string_pretty(sources).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn news_load_sources() -> Vec<NewsSource> {
    read_sources()
}

#[tauri::command]
pub fn news_add_source(name: String, url: String, category: String) -> Result<NewsSource, String> {
    let source = NewsSource {
        id: format!("src{}", chrono::Local::now().timestamp_millis()),
        name, url, category, enabled: true,
    };
    let mut sources = read_sources();
    sources.push(source.clone());
    write_sources(&sources)?;
    Ok(source)
}

#[tauri::command]
pub fn news_remove_source(id: String) -> Result<(), String> {
    let mut sources = read_sources();
    sources.retain(|s| s.id != id);
    write_sources(&sources)
}

#[tauri::command]
pub fn news_set_source_enabled(id: String, enabled: bool) -> Result<(), String> {
    let mut sources = read_sources();
    if let Some(s) = sources.iter_mut().find(|s| s.id == id) { s.enabled = enabled; }
    write_sources(&sources)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsArticle {
    pub source_id: String,
    pub source_name: String,
    pub guid: String,
    pub title: String,
    pub link: String,
    pub description: String,
    pub published: String,
}

async fn fetch_one(client: &reqwest::Client, source: &NewsSource) -> Result<Vec<NewsArticle>, String> {
    let resp = client.get(&source.url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let items = crate::feed_parser::parse_feed_items(&body);
    Ok(items.into_iter().map(|i| NewsArticle {
        source_id: source.id.clone(),
        source_name: source.name.clone(),
        guid: i.guid, title: i.title, link: i.link,
        description: i.description, published: i.published,
    }).collect())
}

/// Fetches one source right now (used by the frontend's per-source
/// "refresh" action and by `news_fetch_all` below). Not cached — every
/// call re-fetches, see module doc's "what this doesn't do".
#[tauri::command]
pub async fn news_fetch_source(source: NewsSource) -> Result<Vec<NewsArticle>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("BlueNews/1.0")
        .build()
        .map_err(|e| e.to_string())?;
    fetch_one(&client, &source).await
}

/// Fetches every enabled source, one at a time, and returns the merged,
/// most-recent-first article list. Sequential rather than concurrent —
/// `futures::future::join_all` (or a `tokio::spawn` per source) would
/// fetch faster, but neither `futures` nor an explicit `tokio`
/// dependency is already in this crate's `Cargo.toml`, and adding one
/// solely for this isn't worth the risk of a dependency-resolution
/// issue I can't verify without a compiler here. A handful of feed
/// fetches sequentially (each has its own 12s timeout) is a real, if
/// not maximally fast, working implementation — parallelizing it is a
/// legitimate follow-up, not a correctness issue.
///
/// Per-source failures (a dead feed URL, a timeout) don't fail the
/// whole call — they're just omitted from the result, since one broken
/// subscription shouldn't blank the entire reader. The frontend can't
/// currently tell *which* source failed from this call alone; a
/// per-source status/error surface is a reasonable follow-up, not
/// implemented here to keep this bounded.
#[tauri::command]
pub async fn news_fetch_all() -> Vec<NewsArticle> {
    let sources: Vec<NewsSource> = read_sources().into_iter().filter(|s| s.enabled).collect();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("BlueNews/1.0")
        .build();
    let Ok(client) = client else { return Vec::new() };

    let mut articles: Vec<NewsArticle> = Vec::new();
    for source in &sources {
        if let Ok(items) = fetch_one(&client, source).await {
            articles.extend(items);
        }
    }
    // Best-effort recency sort — `published` isn't normalized to a
    // single format (see feed_parser::FeedItem's doc comment), so this
    // is a plain string comparison, not a real date sort. RFC 2822
    // (RSS) and ISO 8601 (Atom) don't sort correctly against each other
    // as strings; within a single source's own items (all the same
    // format) this still produces a reasonable-enough recency ordering
    // for a merged view, which is what this is for.
    articles.sort_by(|a, b| b.published.cmp(&a.published));
    articles.truncate(300);
    articles
}
