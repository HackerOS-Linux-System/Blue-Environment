use base64::Engine;
use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PdfInfo {
    pub pages: u32,
    pub title: String,
    pub author: String,
    pub producer: String,
    pub encrypted: bool,
    /// Size of page 1 in PDF points (1/72 inch) — used for fit-to-width.
    pub page_width: f64,
    pub page_height: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PdfHit {
    pub page: u32,
    pub snippet: String,
}

/// Limits so a hostile / enormous file cannot exhaust memory.
const MAX_DPI: u32 = 300;
const MAX_HITS: usize = 300;

pub fn parse_pdfinfo(out: &str) -> PdfInfo {
    let mut i = PdfInfo::default();
    for l in out.lines() {
        let Some((k, v)) = l.split_once(':') else { continue };
        let v = v.trim();
        match k.trim() {
            "Pages" => i.pages = v.parse().unwrap_or(0),
            "Title" => i.title = v.to_string(),
            "Author" => i.author = v.to_string(),
            "Producer" => i.producer = v.to_string(),
            "Encrypted" => i.encrypted = v.starts_with("yes"),
            "Page size" => {
                // "595.276 x 841.89 pts (A4)"
                let nums: Vec<f64> = v
                    .split(|c: char| !(c.is_ascii_digit() || c == '.'))
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if nums.len() >= 2 {
                    i.page_width = nums[0];
                    i.page_height = nums[1];
                }
            }
            _ => {}
        }
    }
    i
}

/// `pdftotext` separates pages with form feeds.
pub fn split_pages(text: &str) -> Vec<&str> {
    let mut v: Vec<&str> = text.split('\u{0c}').collect();
    if v.last().map(|s| s.trim().is_empty()).unwrap_or(false) {
        v.pop();
    }
    v
}

fn fold(s: &str) -> String {
    s.to_lowercase()
}

/// Case-insensitive search over the per-page texts. Returns at most
/// `MAX_HITS` hits with a ~100 character snippet around the match.
pub fn search_pages(pages: &[&str], query: &str) -> Vec<PdfHit> {
    let q = fold(query.trim());
    if q.is_empty() {
        return vec![];
    }
    let mut hits = Vec::new();
    'outer: for (idx, p) in pages.iter().enumerate() {
        let flat: String = p.split_whitespace().collect::<Vec<_>>().join(" ");
        let low = fold(&flat);
        let mut from = 0;
        while let Some(pos) = low[from..].find(&q) {
            let at = from + pos;
            // Work in chars so snippets never split a UTF-8 sequence.
            let chars: Vec<char> = flat.chars().collect();
            let char_at = flat[..at.min(flat.len())].chars().count();
            let start = char_at.saturating_sub(40);
            let end = (char_at + q.chars().count() + 60).min(chars.len());
            let snippet: String = chars[start..end].iter().collect();
            hits.push(PdfHit { page: idx as u32 + 1, snippet });
            if hits.len() >= MAX_HITS {
                break 'outer;
            }
            from = at + q.len().max(1);
            // One hit per ~40 chars is plenty for a results list.
            while from < low.len() && !low.is_char_boundary(from) {
                from += 1;
            }
        }
    }
    hits
}

fn check_pdf(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if !p.is_file() {
        return Err(format!("{path} is not a file"));
    }
    let mut magic = [0u8; 5];
    use std::io::Read;
    std::fs::File::open(p).and_then(|mut f| f.read_exact(&mut magic)).map_err(|e| format!("Cannot read {path}: {e}"))?;
    if &magic != b"%PDF-" {
        return Err("This does not look like a PDF file".into());
    }
    Ok(())
}

fn tool(prog: &str, args: &[&str], password: &Option<String>) -> Result<std::process::Output, String> {
    let mut c = Command::new(prog);
    if let Some(pw) = password.as_deref().filter(|p| !p.is_empty()) {
        c.args(["-upw", pw]);
    }
    c.args(args).stdin(Stdio::null()).output().map_err(|e| {
        format!("Cannot run `{prog}` — install poppler-utils ({e})")
    })
}

fn err_text(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("").trim().to_string()
}

fn is_password_error(msg: &str) -> bool {
    let l = msg.to_lowercase();
    l.contains("incorrect password") || l.contains("password")
}

#[tauri::command(async)]
pub fn pdf_info(path: String, password: Option<String>) -> Result<PdfInfo, String> {
    check_pdf(&path)?;
    let o = tool("pdfinfo", &["--", &path], &password)?;
    if !o.status.success() {
        let e = err_text(&o);
        return Err(if is_password_error(&e) { "PASSWORD_REQUIRED".into() } else { e });
    }
    Ok(parse_pdfinfo(&String::from_utf8_lossy(&o.stdout)))
}

/// Renders one page (1-based) to a PNG data URL. `dpi` is clamped to 30–300.
#[tauri::command(async)]
pub fn pdf_render_page(path: String, page: u32, dpi: u32, password: Option<String>) -> Result<String, String> {
    check_pdf(&path)?;
    if page == 0 {
        return Err("Pages start at 1".into());
    }
    let dpi = dpi.clamp(30, MAX_DPI).to_string();
    let pg = page.to_string();
    let dir = std::env::temp_dir().join(format!("blue-pdf-{}-{}", std::process::id(), page));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let root = dir.join("p");
    let root_s = root.to_string_lossy().to_string();
    let result = (|| {
        let o = tool(
            "pdftoppm",
            &["-png", "-singlefile", "-r", &dpi, "-f", &pg, "-l", &pg, "--", &path, &root_s],
            &password,
        )?;
        if !o.status.success() {
            let e = err_text(&o);
            return Err(if is_password_error(&e) { "PASSWORD_REQUIRED".to_string() } else { e });
        }
        let bytes = std::fs::read(root.with_extension("png")).map_err(|e| format!("Rendering produced no image: {e}"))?;
        Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Plain text of one page (for copy / accessibility).
#[tauri::command(async)]
pub fn pdf_page_text(path: String, page: u32, password: Option<String>) -> Result<String, String> {
    check_pdf(&path)?;
    let pg = page.max(1).to_string();
    let o = tool("pdftotext", &["-layout", "-f", &pg, "-l", &pg, "--", &path, "-"], &password)?;
    if !o.status.success() {
        return Err(err_text(&o));
    }
    Ok(String::from_utf8_lossy(&o.stdout).to_string())
}

#[tauri::command(async)]
pub fn pdf_search(path: String, query: String, password: Option<String>) -> Result<Vec<PdfHit>, String> {
    check_pdf(&path)?;
    let o = tool("pdftotext", &["-layout", "--", &path, "-"], &password)?;
    if !o.status.success() {
        return Err(err_text(&o));
    }
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    Ok(search_pages(&split_pages(&text), &query))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pdfinfo() {
        let out = "Title:          My Doc\nAuthor:         Ann\nProducer:       LaTeX\nEncrypted:      no\nPages:          12\nPage size:      595.276 x 841.89 pts (A4)\n";
        let i = parse_pdfinfo(out);
        assert_eq!(i.pages, 12);
        assert_eq!(i.title, "My Doc");
        assert!(!i.encrypted);
        assert!((i.page_width - 595.276).abs() < 1e-6);
        assert!((i.page_height - 841.89).abs() < 1e-6);
        assert!(parse_pdfinfo("Encrypted: yes (print:yes copy:no)\n").encrypted);
    }

    #[test]
    fn splits_and_searches() {
        let text = "Hello World\n\u{0c}Zażółć gęślą jaźń — World again\n\u{0c}";
        let pages = split_pages(text);
        assert_eq!(pages.len(), 2);
        let hits = search_pages(&pages, "world");
        assert_eq!(hits.iter().map(|h| h.page).collect::<Vec<_>>(), vec![1, 2]);
        assert!(hits[1].snippet.contains("World again"));
        // Unicode must not panic and must be found.
        assert_eq!(search_pages(&pages, "gęślą").len(), 1);
        assert!(search_pages(&pages, "   ").is_empty());
        assert!(search_pages(&pages, "absent").is_empty());
    }

    #[test]
    fn hit_cap() {
        let page = "a ".repeat(2000);
        let hits = search_pages(&[page.as_str()], "a");
        assert_eq!(hits.len(), MAX_HITS);
    }

    #[test]
    fn rejects_non_pdf() {
        let p = std::env::temp_dir().join("blue-not-a.pdf");
        std::fs::write(&p, b"hello").unwrap();
        assert!(check_pdf(p.to_str().unwrap()).is_err());
        std::fs::write(&p, b"%PDF-1.7\n").unwrap();
        assert!(check_pdf(p.to_str().unwrap()).is_ok());
        let _ = std::fs::remove_file(p);
    }

    /// Real poppler round trip (skips itself when poppler-utils is not installed):
    /// builds a 2-page PDF, then info → render → text → search.
    #[test]
    fn real_poppler_round_trip() {
        if Command::new("pdfinfo").arg("-v").output().is_err() || Command::new("pdftoppm").arg("-v").output().is_err() {
            eprintln!("poppler-utils not installed — skipping");
            return;
        }
        // minimal hand-written PDF: 2 pages, Helvetica text
        let mut pdf: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut offs: Vec<usize> = Vec::new();
        let mut obj = |pdf: &mut Vec<u8>, body: &str| { offs.push(pdf.len()); pdf.extend_from_slice(body.as_bytes()); };
        obj(&mut pdf, "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        obj(&mut pdf, "2 0 obj\n<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>\nendobj\n");
        let c1 = "BT /F1 24 Tf 50 700 Td (Hello Blue PDF) Tj ET";
        obj(&mut pdf, &format!("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 7 0 R >> >> >>\nendobj\n"));
        obj(&mut pdf, &format!("4 0 obj\n<< /Length {} >>\nstream\n{c1}\nendstream\nendobj\n", c1.len()));
        let c2 = "BT /F1 24 Tf 50 700 Td (Second page needle) Tj ET";
        obj(&mut pdf, "5 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 6 0 R /Resources << /Font << /F1 7 0 R >> >> >>\nendobj\n");
        obj(&mut pdf, &format!("6 0 obj\n<< /Length {} >>\nstream\n{c2}\nendstream\nendobj\n", c2.len()));
        obj(&mut pdf, "7 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");
        let xref = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 8\n0000000000 65535 f \n").as_bytes());
        for o in &offs { pdf.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes()); }
        pdf.extend_from_slice(format!("trailer\n<< /Size 8 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes());
        let path = std::env::temp_dir().join(format!("blue-pdf-test-{}.pdf", std::process::id()));
        std::fs::write(&path, &pdf).unwrap();
        let p = path.to_string_lossy().to_string();

        let info = pdf_info(p.clone(), None).unwrap();
        assert_eq!(info.pages, 2);
        assert!((info.page_width - 595.0).abs() < 1.0);

        let png = pdf_render_page(p.clone(), 2, 72, None).unwrap();
        assert!(png.starts_with("data:image/png;base64,"));
        let raw = base64::engine::general_purpose::STANDARD.decode(&png["data:image/png;base64,".len()..]).unwrap();
        assert_eq!(&raw[1..4], b"PNG");
        assert!(pdf_render_page(p.clone(), 0, 72, None).is_err());

        assert!(pdf_page_text(p.clone(), 1, None).unwrap().contains("Hello Blue PDF"));
        let hits = pdf_search(p.clone(), "NEEDLE".into(), None).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page, 2);
        let _ = std::fs::remove_file(path);
    }
}
