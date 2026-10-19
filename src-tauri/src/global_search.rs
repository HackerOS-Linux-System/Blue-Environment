use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

const FILE_BUDGET: Duration = Duration::from_millis(700);
const MAX_ENTRIES: usize = 80_000;
const MAX_DEPTH: usize = 7;
const MAX_MAIL_INDEX: usize = 5000;
const SKIP_DIRS: &[&str] = &["node_modules", "target", "__pycache__", "venv", "site-packages", "build", "dist"];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileHit {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub score: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventHit {
    pub id: String,
    pub title: String,
    pub date: String,
    pub time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MailHeader {
    pub account_id: String,
    pub folder: String,
    pub uid: String,
    pub from: String,
    pub subject: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub files: Vec<FileHit>,
    pub events: Vec<EventHit>,
    pub mails: Vec<MailHeader>,
}

// ── Matching ────────────────────────────────────────────────────────────

/// Lower-cases and strips the diacritics of the languages the shell ships in.
pub fn fold(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        match c {
            'ą' | 'ä' | 'á' | 'à' | 'â' | 'ã' | 'å' => o.push('a'),
            'ć' | 'č' | 'ç' => o.push('c'),
            'ę' | 'é' | 'è' | 'ê' | 'ë' | 'ě' => o.push('e'),
            'í' | 'ì' | 'î' | 'ï' | 'ı' => o.push('i'),
            'ł' => o.push('l'),
            'ń' | 'ñ' | 'ň' => o.push('n'),
            'ó' | 'ö' | 'ò' | 'ô' | 'õ' | 'ø' => o.push('o'),
            'ś' | 'š' | 'ş' => o.push('s'),
            'ú' | 'ù' | 'û' | 'ü' | 'ů' => o.push('u'),
            'ý' | 'ÿ' => o.push('y'),
            'ź' | 'ż' | 'ž' => o.push('z'),
            'ß' => o.push_str("ss"),
            other => o.push(other),
        }
    }
    o
}

pub fn tokens(q: &str) -> Vec<String> {
    fold(q).split_whitespace().map(|s| s.to_string()).collect()
}

/// `None` unless every token matches the name or (weakly) the path.
pub fn score_name(name: &str, path: &str, toks: &[String]) -> Option<u32> {
    if toks.is_empty() {
        return None;
    }
    let n = fold(name);
    let p = fold(path);
    let words: Vec<&str> = n.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let mut total = 0;
    for t in toks {
        total += if n == *t {
            300
        } else if n.starts_with(t.as_str()) {
            100
        } else if words.iter().any(|w| w.starts_with(t.as_str())) {
            80
        } else if n.contains(t.as_str()) {
            60
        } else if p.contains(t.as_str()) {
            15
        } else {
            return None;
        };
    }
    Some(total)
}

fn search_roots() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = [dirs::document_dir(), dirs::download_dir(), dirs::desktop_dir(), dirs::picture_dir(), dirs::audio_dir(), dirs::video_dir()]
        .into_iter()
        .flatten()
        .filter(|p| p.is_dir())
        .collect();
    if let Some(h) = dirs::home_dir() {
        if !v.iter().any(|p| *p == h) {
            v.push(h); // visited last, de-duplicated by `seen`
        }
    }
    v
}

pub fn search_files_in(roots: &[PathBuf], query: &str, limit: usize, budget: Duration) -> Vec<FileHit> {
    let toks = tokens(query);
    if toks.is_empty() {
        return vec![];
    }
    let started = Instant::now();
    let mut hits: Vec<FileHit> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut visited = 0usize;
    'roots: for root in roots {
        let walker = WalkDir::new(root).max_depth(MAX_DEPTH).follow_links(false).into_iter().filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !(name.starts_with('.') || (e.file_type().is_dir() && SKIP_DIRS.contains(&name.as_ref())))
        });
        for e in walker.filter_map(|e| e.ok()) {
            visited += 1;
            if visited > MAX_ENTRIES || started.elapsed() > budget {
                break 'roots;
            }
            if e.depth() == 0 {
                continue;
            }
            let path = e.path().to_string_lossy().to_string();
            if !seen.insert(path.clone()) {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(score) = score_name(&name, &path, &toks) {
                hits.push(FileHit { name, path, is_dir: e.file_type().is_dir(), score });
            }
        }
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.path.len().cmp(&b.path.len())));
    hits.truncate(limit);
    hits
}

pub fn search_events(events: &[(String, String, String, Option<String>, String)], query: &str, limit: usize) -> Vec<EventHit> {
    let toks = tokens(query);
    let mut scored: Vec<(u32, EventHit)> = events
        .iter()
        .filter_map(|(id, title, date, time, desc)| {
            score_name(title, desc, &toks).map(|s| (s, EventHit { id: id.clone(), title: title.clone(), date: date.clone(), time: time.clone() }))
        })
        .collect();
    // best match first; for equal scores the most recent date first
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.date.cmp(&a.1.date)));
    scored.into_iter().take(limit).map(|(_, e)| e).collect()
}

pub fn search_mail(index: &[MailHeader], query: &str, limit: usize) -> Vec<MailHeader> {
    let toks = tokens(query);
    let mut scored: Vec<(u32, &MailHeader)> =
        index.iter().filter_map(|m| score_name(&m.subject, &m.from, &toks).map(|s| (s, m))).collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().take(limit).map(|(_, m)| m.clone()).collect()
}

// ── Mail header index ───────────────────────────────────────────────────

fn mail_index_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("Blue-Environment").join("mail").join("search-index.json")
}

fn load_mail_index(p: &Path) -> Vec<MailHeader> {
    std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Merges freshly fetched headers into the index (same account+folder+uid is
/// replaced; the index is capped). Called by `mail_fetch_inbox`.
pub fn merge_mail_index(existing: Vec<MailHeader>, fresh: Vec<MailHeader>) -> Vec<MailHeader> {
    let mut v: Vec<MailHeader> = fresh;
    for old in existing {
        if !v.iter().any(|n| n.account_id == old.account_id && n.folder == old.folder && n.uid == old.uid) {
            v.push(old);
        }
    }
    v.truncate(MAX_MAIL_INDEX);
    v
}

pub fn index_mail(account_id: &str, folder: &str, headers: &[(String, String, String, String)]) {
    let p = mail_index_path();
    let fresh: Vec<MailHeader> = headers
        .iter()
        .map(|(uid, from, subject, date)| MailHeader {
            account_id: account_id.into(),
            folder: folder.into(),
            uid: uid.clone(),
            from: from.clone(),
            subject: subject.clone(),
            date: date.clone(),
        })
        .collect();
    let merged = merge_mail_index(load_mail_index(&p), fresh);
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, serde_json::to_string(&merged).unwrap_or_default());
}

// ── Command ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn global_search(query: String, limit: Option<usize>) -> Result<SearchResults, String> {
    let limit = limit.unwrap_or(8).clamp(1, 30);
    if query.trim().chars().count() < 2 {
        return Ok(SearchResults::default());
    }
    tokio::task::spawn_blocking(move || {
        let events: Vec<_> = crate::blue_calendar_app::calendar_load_events()
            .into_iter()
            .map(|e| (e.id, e.title, e.date, e.time, e.description))
            .collect();
        SearchResults {
            files: search_files_in(&search_roots(), &query, limit, FILE_BUDGET),
            events: search_events(&events, &query, limit),
            mails: search_mail(&load_mail_index(&mail_index_path()), &query, limit),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_polish_and_german() {
        assert_eq!(fold("Zażółć GĘŚLĄ jaźń"), "zazolc gesla jazn");
        assert_eq!(fold("Straße Müller"), "strasse muller");
    }

    #[test]
    fn scoring_prefers_name_over_path() {
        let t = tokens("raport");
        let a = score_name("Raport_2026.pdf", "/h/Docs/Raport_2026.pdf", &t).unwrap();
        let b = score_name("final.pdf", "/h/raport/final.pdf", &t).unwrap();
        assert!(a > b);
        assert!(score_name("x.txt", "/h/y/x.txt", &t).is_none());
        assert!(score_name("a", "a", &[]).is_none());
        // every token must match
        assert!(score_name("raport.pdf", "/raport.pdf", &tokens("raport faktura")).is_none());
        assert!(score_name("Zażółć.txt", "/Zażółć.txt", &tokens("zazolc")).is_some());
    }

    #[test]
    fn walks_a_real_tree() {
        let base = std::env::temp_dir().join(format!("blue-search-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("Docs/node_modules")).unwrap();
        std::fs::create_dir_all(base.join(".hidden")).unwrap();
        std::fs::write(base.join("Docs/budget-2026.ods"), "x").unwrap();
        std::fs::write(base.join("Docs/node_modules/budget.js"), "x").unwrap();
        std::fs::write(base.join(".hidden/budget.txt"), "x").unwrap();
        let hits = search_files_in(&[base.clone()], "budget", 10, Duration::from_secs(5));
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].path.ends_with("budget-2026.ods"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn events_and_mail() {
        let ev = vec![
            ("1".to_string(), "Dentysta".to_string(), "2026-10-20".to_string(), Some("10:00".to_string()), String::new()),
            ("2".to_string(), "Spotkanie".to_string(), "2026-10-21".to_string(), None, "u dentysty".to_string()),
        ];
        let r = search_events(&ev, "dentysta", 5);
        assert_eq!(r[0].id, "1");
        let mails = vec![MailHeader { account_id: "a".into(), folder: "INBOX".into(), uid: "5".into(), from: "Ann <ann@x.org>".into(), subject: "Faktura październik".into(), date: "d".into() }];
        assert_eq!(search_mail(&mails, "faktura pazdziernik", 5).len(), 1);
        assert!(search_mail(&mails, "nothing", 5).is_empty());
    }

    #[test]
    fn mail_index_merge_replaces_and_caps() {
        let h = |uid: &str, subj: &str| MailHeader { account_id: "a".into(), folder: "INBOX".into(), uid: uid.into(), from: "f".into(), subject: subj.into(), date: "d".into() };
        let merged = merge_mail_index(vec![h("1", "old"), h("2", "keep")], vec![h("1", "new")]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged.iter().find(|m| m.uid == "1").unwrap().subject, "new");
    }
}
