use calamine::{open_workbook_auto, Data, Reader};
use docx_rs::{
    read_docx, DocumentChild, Docx, Paragraph, ParagraphChild, Run, RunChild, TableCellContent, TableChild,
    TableRowChild,
};
use regex::Regex;
use serde::Serialize;

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// ───────────────────────── arkusze (calamine) ─────────────────────────

#[derive(Serialize, Debug, PartialEq)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

fn cell_to_string(c: &Data) -> String {
    match c {
        Data::Empty => String::new(),
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Data::Float(f) => f.to_string(),
        other => other.to_string(),
    }
}

pub fn read_sheets(path: &str) -> Result<Vec<Sheet>, String> {
    let mut wb = open_workbook_auto(path).map_err(|e| format!("Nie można otworzyć arkusza: {e}"))?;
    let names = wb.sheet_names().to_vec();
    let mut out = Vec::new();
    for name in names {
        let range = wb.worksheet_range(&name).map_err(|e| format!("Arkusz „{name}”: {e}"))?;
        let rows = range.rows().map(|r| r.iter().map(cell_to_string).collect()).collect();
        out.push(Sheet { name, rows });
    }
    Ok(out)
}

pub fn sheets_to_html(sheets: &[Sheet]) -> String {
    let mut h = String::new();
    for s in sheets {
        h.push_str(&format!("<h2>{}</h2>\n<table border=\"1\" cellspacing=\"0\" cellpadding=\"4\">\n", html_escape(&s.name)));
        for r in &s.rows {
            h.push_str("<tr>");
            for c in r {
                h.push_str(&format!("<td>{}</td>", html_escape(c)));
            }
            h.push_str("</tr>\n");
        }
        h.push_str("</table>\n");
    }
    h
}

pub fn is_spreadsheet_ext(ext: &str) -> bool {
    matches!(ext.to_lowercase().as_str(), "xlsx" | "xlsm" | "xlsb" | "xls" | "ods")
}

#[tauri::command(async)]
pub fn docs_read_spreadsheet(path: String) -> Result<Vec<Sheet>, String> {
    read_sheets(&super::expand_path(&path))
}

// ───────────────────────── DOCX → HTML (docx-rs) ─────────────────────────

fn run_html(run: &Run) -> String {
    let mut text = String::new();
    for c in &run.children {
        match c {
            RunChild::Text(t) => text.push_str(&html_escape(&t.text)),
            RunChild::Tab(_) => text.push_str("&emsp;"),
            RunChild::Break(_) => text.push_str("<br>"),
            _ => {}
        }
    }
    if text.is_empty() {
        return text;
    }
    // `bold`/`italic` w RunProperty to Option; ich obecność oznacza włączone.
    if run.run_property.bold.is_some() { text = format!("<strong>{text}</strong>"); }
    if run.run_property.italic.is_some() { text = format!("<em>{text}</em>"); }
    if run.run_property.underline.is_some() { text = format!("<u>{text}</u>"); }
    text
}

fn paragraph_html(p: &Paragraph) -> String {
    let mut inner = String::new();
    for c in &p.children {
        if let ParagraphChild::Run(r) = c {
            inner.push_str(&run_html(r));
        }
    }
    let style = p.property.style.as_ref().map(|s| s.val.to_lowercase()).unwrap_or_default();
    let is_list = p.property.numbering_property.is_some();
    let tag = match style.as_str() {
        "heading1" | "title" => "h1",
        "heading2" => "h2",
        "heading3" => "h3",
        "heading4" => "h4",
        _ if is_list => "li",
        _ => "p",
    };
    if inner.is_empty() && tag == "p" {
        return "<p><br></p>".into();
    }
    format!("<{tag}>{inner}</{tag}>")
}

pub fn docx_to_html(bytes: &[u8]) -> Result<String, String> {
    let docx = read_docx(bytes).map_err(|e| format!("Nieprawidłowy plik DOCX: {e:?}"))?;
    let mut out = String::new();
    for child in &docx.document.children {
        match child {
            DocumentChild::Paragraph(p) => {
                out.push_str(&paragraph_html(p));
                out.push('\n');
            }
            DocumentChild::Table(t) => {
                out.push_str("<table border=\"1\" cellspacing=\"0\" cellpadding=\"4\">");
                for TableChild::TableRow(row) in &t.rows {
                    out.push_str("<tr>");
                    for TableRowChild::TableCell(cell) in &row.cells {
                        out.push_str("<td>");
                        for content in &cell.children {
                            if let TableCellContent::Paragraph(p) = content {
                                out.push_str(&paragraph_html(p));
                            }
                        }
                        out.push_str("</td>");
                    }
                    out.push_str("</tr>");
                }
                out.push_str("</table>\n");
            }
            _ => {}
        }
    }
    Ok(out)
}

// ───────────────────────── HTML → DOCX (docx-rs) ─────────────────────────

#[derive(Debug, PartialEq, Clone)]
pub struct Block {
    pub style: Option<&'static str>, // "Heading1".. albo None
    pub runs: Vec<(String, bool, bool)>, // (tekst, bold, italic)
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

/// Prosty parser edytorowego HTML: bloki h1-h4/p/li/div, w nich <strong>/<b>/<em>/<i>.
pub fn html_to_blocks(html: &str) -> Vec<Block> {
    let block_re = Regex::new(r"(?is)<(h[1-4]|p|li|div)[^>]*>(.*?)</(?:h[1-4]|p|li|div)>").unwrap();
    let tok_re = Regex::new(r"(?is)<(/?)(strong|b|em|i)\b[^>]*>|<br\s*/?>|<[^>]+>|([^<]+)").unwrap();
    let mut blocks = Vec::new();
    for cap in block_re.captures_iter(html) {
        let style = match cap[1].to_lowercase().as_str() {
            "h1" => Some("Heading1"), "h2" => Some("Heading2"), "h3" => Some("Heading3"), "h4" => Some("Heading4"),
            _ => None,
        };
        let (mut bold, mut italic) = (0i32, 0i32);
        let mut runs = Vec::new();
        for t in tok_re.captures_iter(&cap[2]) {
            if let Some(tag) = t.get(2) {
                let closing = !t[1].is_empty();
                let d = if closing { -1 } else { 1 };
                match tag.as_str().to_lowercase().as_str() {
                    "strong" | "b" => bold = (bold + d).max(0),
                    _ => italic = (italic + d).max(0),
                }
            } else if let Some(txt) = t.get(3) {
                let s = decode_entities(txt.as_str());
                if !s.trim().is_empty() || !runs.is_empty() {
                    runs.push((s, bold > 0, italic > 0));
                }
            }
        }
        if !runs.is_empty() || style.is_some() {
            blocks.push(Block { style, runs });
        }
    }
    if blocks.is_empty() {
        // zwykły tekst bez znaczników
        for line in decode_entities(&Regex::new(r"(?s)<[^>]+>").unwrap().replace_all(html, "")).lines() {
            if !line.trim().is_empty() {
                blocks.push(Block { style: None, runs: vec![(line.to_string(), false, false)] });
            }
        }
    }
    blocks
}

pub fn html_to_docx_bytes(html: &str) -> Result<Vec<u8>, String> {
    let mut docx = Docx::new();
    for b in html_to_blocks(html) {
        let mut p = Paragraph::new();
        if let Some(s) = b.style {
            p = p.style(s);
        }
        for (text, bold, italic) in b.runs {
            let mut r = Run::new().add_text(text);
            if bold { r = r.bold(); }
            if italic { r = r.italic(); }
            p = p.add_run(r);
        }
        docx = docx.add_paragraph(p);
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    docx.build().pack(&mut buf).map_err(|e| format!("Zapis DOCX nie powiódł się: {e}"))?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_blocks_with_formatting() {
        let b = html_to_blocks("<h1>Tytuł</h1><p>Zwykły <strong>gruby</strong> i <em>kursywa</em> &amp; więcej</p>");
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].style, Some("Heading1"));
        assert!(b[1].runs.iter().any(|(t, bold, _)| t == "gruby" && *bold));
        assert!(b[1].runs.iter().any(|(t, _, it)| t == "kursywa" && *it));
        assert!(b[1].runs.iter().any(|(t, _, _)| t.contains("& więcej")));
    }

    #[test]
    fn docx_round_trip_keeps_text_headings_and_bold() {
        let html = "<h1>Raport</h1><p>Wartość: <strong>42</strong></p><p>Ostatni akapit</p>";
        let bytes = html_to_docx_bytes(html).unwrap();
        assert!(bytes.starts_with(b"PK")); // prawdziwy ZIP
        let back = docx_to_html(&bytes).unwrap();
        assert!(back.contains("<h1>Raport</h1>"), "{back}");
        assert!(back.contains("<strong>42</strong>"), "{back}");
        assert!(back.contains("Ostatni akapit"));
    }

    #[test]
    fn rejects_garbage_docx() {
        assert!(docx_to_html(b"to nie jest docx").is_err());
    }

    #[test]
    fn spreadsheet_html_and_ext() {
        let sheets = vec![Sheet { name: "A<1>".into(), rows: vec![vec!["x".into(), "1".into()]] }];
        let h = sheets_to_html(&sheets);
        assert!(h.contains("<h2>A&lt;1&gt;</h2>") && h.contains("<td>1</td>"));
        assert!(is_spreadsheet_ext("XLSX") && is_spreadsheet_ext("ods") && !is_spreadsheet_ext("docx"));
        assert_eq!(cell_to_string(&Data::Float(3.0)), "3");
        assert_eq!(cell_to_string(&Data::Float(2.5)), "2.5");
    }

    #[test]
    fn missing_workbook_is_an_error() {
        assert!(read_sheets("/nie/ma/takiego.xlsx").is_err());
    }
}
  
