use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tracing::info;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocMeta {
    pub path:      String,
    pub name:      String,
    pub size:      u64,
    pub modified:  u64,   // Unix timestamp
    pub format:    String,
    pub word_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpellResult {
    pub word:        String,
    pub correct:     bool,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocResult {
    pub success: bool,
    pub message: Option<String>,
    pub path:    Option<String>,
}

impl DocResult {
    fn ok() -> Self { Self { success: true, message: None, path: None } }
    fn ok_path(p: impl Into<String>) -> Self { Self { success: true, message: None, path: Some(p.into()) } }
    fn err(msg: impl Into<String>) -> Self { Self { success: false, message: Some(msg.into()), path: None } }
}

fn sh(cmd: &str) -> Result<String, String> {
    Command::new("sh").arg("-c").arg(cmd)
        .output()
        .map_err(|e| e.to_string())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn expand_path(path: &str) -> String {
    if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        path.replacen('~', &home, 1)
    } else {
        path.to_string()
    }
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ─────────────────────────────────────────────────────────────────────────────
// File I/O commands
// ─────────────────────────────────────────────────────────────────────────────

/// Read a document from disk and return its contents as a string.
#[tauri::command]
pub fn docs_read_file(path: String) -> Result<String, String> {
    let expanded = expand_path(&path);
    std::fs::read_to_string(&expanded)
        .map_err(|e| format!("Failed to read '{}': {}", path, e))
}

/// Write document content to disk.
#[tauri::command]
pub fn docs_write_file(path: String, content: String) -> DocResult {
    let expanded = expand_path(&path);
    let dir = Path::new(&expanded).parent().unwrap_or(Path::new("."));

    if let Err(e) = std::fs::create_dir_all(dir) {
        return DocResult::err(format!("Cannot create directory: {}", e));
    }

    match std::fs::write(&expanded, &content) {
        Ok(_) => {
            info!("Docs: saved '{}'", expanded);
            DocResult::ok_path(expanded)
        }
        Err(e) => DocResult::err(format!("Failed to write '{}': {}", path, e)),
    }
}

/// List recently used documents from the XDG recent-files list.
#[tauri::command]
pub fn docs_get_recent() -> Vec<DocMeta> {
    let recent_file = sh(
        "echo ~/.local/share/recently-used.xbel 2>/dev/null"
    ).unwrap_or_default();

    let content = std::fs::read_to_string(recent_file.trim()).unwrap_or_default();
    let mut docs: Vec<DocMeta> = Vec::new();

    // Very simple XBEL parser — extract href attributes from <bookmark> tags
    for line in content.lines() {
        if line.contains("href=\"file://") {
            let start = line.find("href=\"file://").unwrap() + 8;
            let rest  = &line[start..];
            let end   = rest.find('"').unwrap_or(rest.len());
            let path  = urlencoding::decode(&rest[..end]).unwrap_or_default().to_string();

            let ext = Path::new(&path).extension()
                .and_then(|e| e.to_str())
                .unwrap_or("txt")
                .to_lowercase();

            if !["html","htm","md","txt","csv","json","doc","docx","odt"].contains(&ext.as_str()) {
                continue;
            }

            if let Ok(meta) = std::fs::metadata(&path) {
                let name = Path::new(&path).file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let format = match ext.as_str() {
                    "md" | "markdown"        => "markdown",
                    "csv"                    => "spreadsheet",
                    "html" | "htm" | "doc" | "docx" | "odt" => "rich",
                    _                        => "rich",
                };
                let modified = meta.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                docs.push(DocMeta {
                    path:       path.clone(),
                    name,
                    size:       meta.len(),
                    modified,
                    format:     format.to_string(),
                    word_count: 0,
                });
            }
        }

        if docs.len() >= 20 { break; }
    }

    docs
}

/// Get metadata and word count for a document on disk.
#[tauri::command]
pub fn docs_get_meta(path: String) -> Result<DocMeta, String> {
    let expanded = expand_path(&path);
    let meta = std::fs::metadata(&expanded)
        .map_err(|e| format!("Cannot stat '{}': {}", path, e))?;

    let name = Path::new(&expanded).file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let ext = Path::new(&expanded).extension()
        .and_then(|e| e.to_str())
        .unwrap_or("txt")
        .to_lowercase();

    let format = match ext.as_str() {
        "md" | "markdown" => "markdown",
        "csv"             => "spreadsheet",
        _                 => "rich",
    }.to_string();

    // Count words
    let content = std::fs::read_to_string(&expanded).unwrap_or_default();
    let plain = content.replace(|c: char| c == '<', " ")
        .split_whitespace()
        .count() as u64;

    let modified = meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(DocMeta {
        path: expanded,
        name,
        size: meta.len(),
        modified,
        format,
        word_count: plain,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Export commands
// ─────────────────────────────────────────────────────────────────────────────

/// Export an HTML document to PDF using chromium/wkhtmltopdf.
#[tauri::command]
pub fn docs_export_pdf(html_path: String, output_path: String) -> DocResult {
    let html = expand_path(&html_path);
    let out  = expand_path(&output_path);

    // Try chromium headless first (best quality)
    let chromium_cmd = format!(
        "chromium --headless --disable-gpu --print-to-pdf={} {} 2>/dev/null \
         || google-chrome --headless --disable-gpu --print-to-pdf={} {} 2>/dev/null",
        shell_escape(&out), shell_escape(&html),
        shell_escape(&out), shell_escape(&html),
    );

    if sh(&chromium_cmd).is_ok() && Path::new(&out).exists() {
        info!("Docs: exported PDF via chromium to '{}'", out);
        return DocResult::ok_path(out);
    }

    // Fallback: wkhtmltopdf
    let wk_cmd = format!("wkhtmltopdf {} {} 2>/dev/null", shell_escape(&html), shell_escape(&out));
    if sh(&wk_cmd).is_ok() && Path::new(&out).exists() {
        info!("Docs: exported PDF via wkhtmltopdf to '{}'", out);
        return DocResult::ok_path(out);
    }

    DocResult::err("PDF export failed — install chromium or wkhtmltopdf")
}

/// Convert an HTML file to plain text.
#[tauri::command]
pub fn docs_export_plaintext(html: String) -> String {
    // Strip tags and decode common HTML entities
    html
        .replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n")
        .replace("</p>", "\n").replace("</div>", "\n")
        .replace("</h1>", "\n\n").replace("</h2>", "\n\n").replace("</h3>", "\n\n")
        .replace("</li>", "\n")
        .replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">")
        .replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'")
        .split_inclusive('>')
        .map(|s| {
            if let Some(pos) = s.rfind('>') {
                s[pos + 1..].to_string()
            } else {
                s.to_string()
            }
        })
        .collect::<String>()
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert rich HTML content to Markdown approximation.
#[tauri::command]
pub fn docs_export_markdown(html: String) -> String {
    let md = html
        .replace("<h1>", "# ").replace("</h1>", "\n\n")
        .replace("<h2>", "## ").replace("</h2>", "\n\n")
        .replace("<h3>", "### ").replace("</h3>", "\n\n")
        .replace("<strong>", "**").replace("</strong>", "**")
        .replace("<b>", "**").replace("</b>", "**")
        .replace("<em>", "*").replace("</em>", "*")
        .replace("<i>", "*").replace("</i>", "*")
        .replace("<u>", "_").replace("</u>", "_")
        .replace("<code>", "`").replace("</code>", "`")
        .replace("<li>", "- ").replace("</li>", "\n")
        .replace("<ul>", "").replace("</ul>", "\n")
        .replace("<ol>", "").replace("</ol>", "\n")
        .replace("<p>", "").replace("</p>", "\n\n")
        .replace("<br>", "\n").replace("<br/>", "\n")
        .replace("<blockquote>", "> ").replace("</blockquote>", "\n")
        .replace("<hr>", "\n---\n")
        .replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">")
        .replace("&amp;", "&");

    // Strip remaining tags
    let re_like: String = md.chars()
        .collect::<String>()
        .split_inclusive('>')
        .map(|s| if let Some(p) = s.rfind('>') { s[p + 1..].to_string() } else { s.to_string() })
        .collect();

    re_like
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

// ─────────────────────────────────────────────────────────────────────────────
// Spell check (hunspell)
// ─────────────────────────────────────────────────────────────────────────────

/// Spell-check a list of words using hunspell.
/// Returns results for every word (correct + suggestions for wrong ones).
#[tauri::command]
pub fn docs_spellcheck(words: Vec<String>, lang: Option<String>) -> Vec<SpellResult> {
    let lang = lang.unwrap_or_else(|| "en_US".to_string());
    let mut results = Vec::new();

    // Check if hunspell is available
    if sh("which hunspell 2>/dev/null").map(|s| s.is_empty()).unwrap_or(true) {
        // No hunspell — return all as correct (graceful fallback)
        return words.into_iter().map(|w| SpellResult { word: w, correct: true, suggestions: vec![] }).collect();
    }

    // Feed words to hunspell via stdin, one per line
    let input = words.join("\n");
    let cmd   = format!("printf '%s' {} | hunspell -l -d {} 2>/dev/null", shell_escape(&input), lang);
    let wrong_raw = sh(&cmd).unwrap_or_default();
    let wrong: std::collections::HashSet<&str> = wrong_raw.lines().collect();

    for word in &words {
        let correct = !wrong.contains(word.as_str());
        let suggestions = if !correct {
            // Get suggestions for this word
            let sug_cmd = format!("echo {} | hunspell -d {} 2>/dev/null | grep '^&' | head -1 | cut -d: -f2", shell_escape(word), lang);
            sh(&sug_cmd)
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .take(8)
                .collect()
        } else {
            vec![]
        };
        results.push(SpellResult { word: word.clone(), correct, suggestions });
    }

    results
}

// ─────────────────────────────────────────────────────────────────────────────
// Template library
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct DocTemplate {
    pub id:      String,
    pub name:    String,
    pub format:  String,
    pub preview: String,   // short description
    pub content: String,
}

/// Return a list of built-in document templates.
#[tauri::command]
pub fn docs_get_templates() -> Vec<DocTemplate> {
    vec![
        DocTemplate {
            id: "blank-rich".into(),
            name: "Blank Document".into(),
            format: "rich".into(),
            preview: "Empty rich-text document".into(),
            content: "<p></p>".into(),
        },
        DocTemplate {
            id: "letter".into(),
            name: "Letter".into(),
            format: "rich".into(),
            preview: "Formal letter with sender/recipient block".into(),
            content: r#"<p style="text-align:right"><strong>Your Name</strong><br>Your Address<br>City, State ZIP<br>Date</p>
<p><br></p>
<p><strong>Recipient Name</strong><br>Recipient Address<br>City, State ZIP</p>
<p><br></p>
<p>Dear [Recipient],</p>
<p><br></p>
<p>I am writing to you regarding…</p>
<p><br></p>
<p>Yours sincerely,<br><strong>Your Name</strong></p>"#.into(),
        },
        DocTemplate {
            id: "meeting-notes".into(),
            name: "Meeting Notes".into(),
            format: "rich".into(),
            preview: "Structured meeting notes template".into(),
            content: r#"<h1>Meeting Notes</h1>
<p><strong>Date:</strong> [Date]<br><strong>Attendees:</strong> [Names]<br><strong>Location:</strong> [Place / Link]</p>
<h2>Agenda</h2>
<ul><li>Item 1</li><li>Item 2</li></ul>
<h2>Discussion</h2>
<p>[Key points discussed]</p>
<h2>Action Items</h2>
<ul><li>[ ] Task — Owner — Due date</li></ul>
<h2>Next Meeting</h2>
<p>[Date and time]</p>"#.into(),
        },
        DocTemplate {
            id: "readme".into(),
            name: "README".into(),
            format: "markdown".into(),
            preview: "GitHub-style README.md".into(),
            content: "# Project Name\n\nShort description of the project.\n\n## Installation\n\n```bash\n# installation commands\n```\n\n## Usage\n\n```bash\n# usage example\n```\n\n## Contributing\n\nPull requests are welcome.\n\n## License\n\nMIT\n".into(),
        },
        DocTemplate {
            id: "budget".into(),
            name: "Budget Spreadsheet".into(),
            format: "spreadsheet".into(),
            preview: "Simple income/expense tracker".into(),
            content: {
                let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
                // Header row
                rows.push(vec!["Category","Item","Budgeted","Actual","Difference"].iter()
                    .map(|s| serde_json::json!({ "raw": s })).collect());
                // Income section
                rows.push(vec!["Income","Salary","","",""].iter()
                    .map(|s| serde_json::json!({ "raw": s })).collect());
                // Expense rows
                for item in &["Rent","Groceries","Transport","Entertainment","Other"] {
                    rows.push(vec!["Expense", item, "", "", ""].iter()
                        .map(|s| serde_json::json!({ "raw": s })).collect());
                }
                // Total row
                rows.push(vec!["Total","","=SUM(C2:C9)","=SUM(D2:D9)","=C10-D10"].iter()
                    .map(|s| serde_json::json!({ "raw": s })).collect());
                serde_json::to_string(&rows).unwrap_or_default()
            },
        },
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Auto-save to XDG cache
// ─────────────────────────────────────────────────────────────────────────────

/// Save a document to the Blue Docs autosave cache.
#[tauri::command]
pub fn docs_autosave(doc_id: String, content: String, name: String) -> bool {
    let cache_dir = sh("echo ${XDG_CACHE_HOME:-~/.cache}/Blue-Environment/docs").unwrap_or_default();
    if sh(&format!("mkdir -p {}", cache_dir)).is_err() { return false; }

    let path = format!("{}/{}.autosave.html", cache_dir.trim(), doc_id);
    let meta_path = format!("{}/{}.meta.json", cache_dir.trim(), doc_id);

    let meta = serde_json::json!({ "id": doc_id, "name": name, "saved_at": chrono::Utc::now().timestamp() });

    std::fs::write(&path, &content).is_ok()
        && std::fs::write(&meta_path, meta.to_string()).is_ok()
}

/// List all autosaved documents.
#[tauri::command]
pub fn docs_list_autosaved() -> Vec<DocMeta> {
    let cache_dir = sh("echo ${XDG_CACHE_HOME:-~/.cache}/Blue-Environment/docs").unwrap_or_default();
    let cache_dir = cache_dir.trim();

    std::fs::read_dir(cache_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("html"))
        .filter_map(|e| {
            let path = e.path();
            let fname = path.file_name()?.to_str()?.to_string();
            let meta = std::fs::metadata(&path).ok()?;
            Some(DocMeta {
                path:       path.to_string_lossy().to_string(),
                name:       fname.replace(".autosave.html", ""),
                size:       meta.len(),
                modified:   meta.modified().ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()).unwrap_or(0),
                format:     "rich".into(),
                word_count: 0,
            })
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// DOCX import (via python-docx or pandoc)
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a .docx file to HTML for display in the rich editor.
/// Tries pandoc first (best quality), then python-docx, then returns raw XML fallback.
#[tauri::command]
pub fn docs_read_docx(path: String) -> Result<String, String> {
    let expanded = expand_path(&path);

    // Try pandoc (best conversion quality)
    let pandoc = sh(&format!(
        "pandoc -f docx -t html --wrap=none {} 2>/dev/null",
        shell_escape(&expanded)
    ));
    if let Ok(html) = pandoc {
        if !html.is_empty() {
            return Ok(html);
        }
    }

    // Try python-docx via inline script
    let py_script = format!(r#"
python3 -c "
import sys
try:
    from docx import Document
    from docx.oxml.ns import qn
    doc = Document(sys.argv[1])
    parts = []
    for p in doc.paragraphs:
        style = p.style.name.lower()
        text = p.text.replace('<','&lt;').replace('>','&gt;')
        if not text.strip():
            parts.append('<p><br></p>')
            continue
        if 'heading 1' in style:
            parts.append(f'<h1>{{text}}</h1>')
        elif 'heading 2' in style:
            parts.append(f'<h2>{{text}}</h2>')
        elif 'heading 3' in style:
            parts.append(f'<h3>{{text}}</h3>')
        else:
            parts.append(f'<p>{{text}}</p>')
    print(''.join(parts))
except Exception as e:
    print(f'<p>Import error: {{e}}</p>')
" {} 2>/dev/null
"#, shell_escape(&expanded));

    let py_result = sh(py_script.trim());
    if let Ok(html) = py_result {
        if !html.is_empty() && !html.contains("Import error") {
            return Ok(html);
        }
    }

    Err(format!(
        "Cannot open '{}': install pandoc (recommended) or python3-docx for DOCX support",
        path
    ))
}

/// Save the HTML content of the editor back to a .docx file via pandoc.
#[tauri::command]
pub fn docs_write_docx(html_content: String, output_path: String) -> DocResult {
    let expanded = expand_path(&output_path);

    // Write HTML to temp file then convert with pandoc
    let tmp = format!("/tmp/blue-docs-export-{}.html", std::process::id());
    if let Err(e) = std::fs::write(&tmp, &html_content) {
        return DocResult::err(format!("Temp write failed: {}", e));
    }

    let cmd = format!(
        "pandoc -f html -t docx -o {} {} 2>&1",
        shell_escape(&expanded), shell_escape(&tmp)
    );
    let result = sh(&cmd);
    let _ = std::fs::remove_file(&tmp);

    match result {
        Ok(_) if std::path::Path::new(&expanded).exists() => {
            info!("Docs: exported DOCX to '{}'", expanded);
            DocResult::ok_path(expanded)
        }
        Ok(err_msg) => DocResult::err(format!("pandoc: {} — install pandoc for DOCX export", err_msg)),
        Err(e)      => DocResult::err(format!("DOCX export failed: {}", e)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PDF import (text extraction)
// ─────────────────────────────────────────────────────────────────────────────

/// Extract text from a PDF and return it as HTML paragraphs.
/// Uses pdftohtml (poppler), then pdftotext, then mutool as fallbacks.
#[tauri::command]
pub fn docs_read_pdf(path: String) -> Result<String, String> {
    let expanded = expand_path(&path);

    // Try pdftohtml (best structure preservation)
    let tmp_dir = format!("/tmp/blue-docs-pdf-{}", std::process::id());
    std::fs::create_dir_all(&tmp_dir).ok();
    let html_out = format!("{}/output.html", tmp_dir);

    let pdftohtml_result = sh(&format!(
        "pdftohtml -noframes -nodrm {} {} 2>/dev/null && cat {}",
        shell_escape(&expanded), shell_escape(&html_out), shell_escape(&html_out)
    ));

    std::fs::remove_dir_all(&tmp_dir).ok();

    if let Ok(html) = pdftohtml_result {
        if !html.is_empty() {
            // Clean up pdftohtml output (remove meta, style, etc.)
            let body_start = html.find("<body").unwrap_or(0);
            let body_end   = html.rfind("</body>").map(|i| i + 7).unwrap_or(html.len());
            let body = &html[body_start..body_end];
            return Ok(format!("<div class=\"pdf-import\">{}</div>", body));
        }
    }

    // Try pdftotext (plain text fallback)
    let text = sh(&format!("pdftotext {} - 2>/dev/null", shell_escape(&expanded)));
    if let Ok(txt) = text {
        if !txt.is_empty() {
            let html: String = txt.split("\n\n")
                .map(|para| {
                    let p = para.replace('\n', " ")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;");
                    if p.trim().is_empty() { String::new() }
                    else { format!("<p>{}</p>", p.trim()) }
                })
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(html);
        }
    }

    // Try mutool (MuPDF)
    let mutool = sh(&format!("mutool draw -F text {} 2>/dev/null", shell_escape(&expanded)));
    if let Ok(txt) = mutool {
        if !txt.is_empty() {
            let html: String = txt.lines()
                .map(|l| format!("<p>{}</p>", l.replace('<', "&lt;").replace('>', "&gt;")))
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(html);
        }
    }

    Err(format!(
        "Cannot read '{}': install poppler-utils (pdftotext/pdftohtml) or mupdf-tools",
        path
    ))
}

/// Export an HTML document to DOCX using pandoc.
#[tauri::command]
pub fn docs_export_docx(html: String, path: String) -> DocResult {
    docs_write_docx(html, path)
}
