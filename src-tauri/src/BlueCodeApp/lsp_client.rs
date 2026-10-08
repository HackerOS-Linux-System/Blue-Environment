use lsp_types::notification::{self, Notification};
use lsp_types::request::{self, Request};
use lsp_types::*;
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

#[derive(Default)]
struct Session {
    next_id: AtomicI64,
    pending: Mutex<HashMap<i64, oneshot::Sender<Value>>>,
    initialized: AtomicBool,
    /// wersje otwartych dokumentów (uri → wersja)
    versions: Mutex<HashMap<String, i32>>,
}

static SESSIONS: Lazy<Mutex<HashMap<String, Arc<Session>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn key_of(language: &str, root: &str) -> String {
    format!("{language}::{root}")
}

fn session(key: &str) -> Arc<Session> {
    SESSIONS.lock().unwrap().entry(key.to_string()).or_default().clone()
}

/// Wołane po zatrzymaniu serwera — czyści stan, a oczekujące żądania kończą się błędem.
pub fn drop_session(key: &str) {
    SESSIONS.lock().unwrap().remove(key);
}

// ───────────────────────── URI ─────────────────────────

/// `/home/u/a b.rs` → `file:///home/u/a%20b.rs`
pub fn path_to_uri(path: &str) -> String {
    let mut out = String::from("file://");
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn uri_to_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn uri(path: &str) -> Result<Uri, String> {
    Uri::from_str(&path_to_uri(path)).map_err(|e| format!("zła ścieżka: {e}"))
}

// ───────────────────────── transport (przez mod.rs) ─────────────────────────

fn send(key: &str, message: &Value) -> Result<(), String> {
    super::send_raw(key, message)
}

fn notify<N: Notification>(key: &str, params: N::Params) -> Result<(), String>
where
    N::Params: Serialize,
{
    send(key, &json!({ "jsonrpc": "2.0", "method": N::METHOD, "params": params }))
}

async fn call<R: Request>(key: &str, params: R::Params) -> Result<Value, String>
where
    R::Params: Serialize,
{
    let s = session(key);
    let id = s.next_id.fetch_add(1, Ordering::SeqCst) + 1;
    let (tx, rx) = oneshot::channel();
    s.pending.lock().unwrap().insert(id, tx);
    if let Err(e) = send(key, &json!({ "jsonrpc": "2.0", "id": id, "method": R::METHOD, "params": params })) {
        s.pending.lock().unwrap().remove(&id);
        return Err(e);
    }
    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(_)) => Err("serwer języka został zatrzymany".into()),
        Err(_) => {
            s.pending.lock().unwrap().remove(&id);
            Err(format!("{}: serwer nie odpowiedział w 30 s", R::METHOD))
        }
    }
}

/// Wynik odpowiedzi: `result` albo błąd z `error.message`.
fn result_of(resp: Value) -> Result<Value, String> {
    if let Some(e) = resp.get("error") {
        return Err(e.get("message").and_then(|m| m.as_str()).unwrap_or("błąd serwera LSP").to_string());
    }
    Ok(resp.get("result").cloned().unwrap_or(Value::Null))
}

// ───────────────────────── wiadomości przychodzące ─────────────────────────

/// Odpowiedź na żądanie wysłane PRZEZ serwer (np. `workspace/configuration`).
pub fn answer_server_request(method: &str, params: &Value) -> Value {
    match method {
        "workspace/configuration" => {
            let n = params.get("items").and_then(|i| i.as_array()).map(|a| a.len()).unwrap_or(0);
            Value::Array(vec![Value::Null; n])
        }
        _ => Value::Null, // registerCapability, workDoneProgress/create, ...
    }
}

/// Wołane z wątku czytającego stdout serwera dla każdej wiadomości.
pub fn on_message(app: &AppHandle, key: &str, msg: &Value) {
    let method = msg.get("method").and_then(|m| m.as_str());
    let id = msg.get("id");
    match (method, id) {
        // odpowiedź na nasze żądanie
        (None, Some(id)) => {
            if let Some(id) = id.as_i64() {
                if let Some(tx) = session(key).pending.lock().unwrap().remove(&id) {
                    let _ = tx.send(msg.clone());
                }
            }
        }
        // żądanie od serwera
        (Some(m), Some(id)) => {
            let result = answer_server_request(m, msg.get("params").unwrap_or(&Value::Null));
            let _ = send(key, &json!({ "jsonrpc": "2.0", "id": id, "result": result }));
        }
        // powiadomienie
        (Some(m), None) if m == notification::PublishDiagnostics::METHOD => {
            if let Some(p) = msg.get("params").and_then(|p| serde_json::from_value::<PublishDiagnosticsParams>(p.clone()).ok()) {
                let path = uri_to_path(p.uri.as_str()).unwrap_or_else(|| p.uri.as_str().to_string());
                let out: Vec<DiagnosticOut> = p.diagnostics.iter().map(DiagnosticOut::from).collect();
                let _ = app.emit("lsp-diagnostics", DiagnosticsEvent { key: key.to_string(), path, diagnostics: out });
            }
        }
        _ => {}
    }
}

// ───────────────────────── typy dla frontendu ─────────────────────────

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticOut {
    pub message: String,
    /// 1 błąd, 2 ostrzeżenie, 3 informacja, 4 podpowiedź (jak w LSP)
    pub severity: u8,
    pub line: u32,
    pub character: u32,
    pub end_line: u32,
    pub end_character: u32,
    pub source: Option<String>,
    pub code: Option<String>,
}

impl From<&Diagnostic> for DiagnosticOut {
    fn from(d: &Diagnostic) -> Self {
        DiagnosticOut {
            message: d.message.clone(),
            severity: match d.severity {
                Some(DiagnosticSeverity::ERROR) | None => 1,
                Some(DiagnosticSeverity::WARNING) => 2,
                Some(DiagnosticSeverity::INFORMATION) => 3,
                _ => 4,
            },
            line: d.range.start.line,
            character: d.range.start.character,
            end_line: d.range.end.line,
            end_character: d.range.end.character,
            source: d.source.clone(),
            code: d.code.as_ref().map(|c| match c {
                NumberOrString::Number(n) => n.to_string(),
                NumberOrString::String(s) => s.clone(),
            }),
        }
    }
}

#[derive(Serialize, Clone)]
struct DiagnosticsEvent {
    key: String,
    path: String,
    diagnostics: Vec<DiagnosticOut>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HoverOut {
    pub contents: String,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompletionOut {
    pub label: String,
    pub kind: Option<u32>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: String,
    /// true, gdy `insert_text` jest snippetem (`$1`, `${2:x}`)
    pub snippet: bool,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocationOut {
    pub path: String,
    pub line: u32,
    pub character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

fn marked_string_text(m: &MarkedString) -> String {
    match m {
        MarkedString::String(s) => s.clone(),
        MarkedString::LanguageString(l) => format!("```{}\n{}\n```", l.language, l.value),
    }
}

pub fn hover_to_out(h: Hover) -> Option<HoverOut> {
    let text = match h.contents {
        HoverContents::Scalar(m) => marked_string_text(&m),
        HoverContents::Array(v) => v.iter().map(marked_string_text).collect::<Vec<_>>().join("\n\n"),
        HoverContents::Markup(m) => m.value,
    };
    (!text.trim().is_empty()).then_some(HoverOut { contents: text })
}

fn doc_text(d: &Documentation) -> String {
    match d {
        Documentation::String(s) => s.clone(),
        Documentation::MarkupContent(m) => m.value.clone(),
    }
}

pub fn completion_to_out(r: CompletionResponse) -> Vec<CompletionOut> {
    let items = match r {
        CompletionResponse::Array(a) => a,
        CompletionResponse::List(l) => l.items,
    };
    items
        .into_iter()
        .take(200)
        .map(|i| {
            let snippet = i.insert_text_format == Some(InsertTextFormat::SNIPPET);
            let insert = i.insert_text.clone().or_else(|| match &i.text_edit {
                Some(CompletionTextEdit::Edit(e)) => Some(e.new_text.clone()),
                Some(CompletionTextEdit::InsertAndReplace(e)) => Some(e.new_text.clone()),
                None => None,
            });
            CompletionOut {
                insert_text: insert.unwrap_or_else(|| i.label.clone()),
                kind: i.kind.map(|k| serde_json::to_value(k).ok().and_then(|v| v.as_u64()).unwrap_or(0) as u32),
                detail: i.detail,
                documentation: i.documentation.as_ref().map(doc_text),
                snippet,
                label: i.label,
            }
        })
        .collect()
}

fn loc(uri: &Uri, r: &Range) -> LocationOut {
    LocationOut {
        path: uri_to_path(uri.as_str()).unwrap_or_else(|| uri.as_str().to_string()),
        line: r.start.line,
        character: r.start.character,
        end_line: r.end.line,
        end_character: r.end.character,
    }
}

pub fn definition_to_out(r: GotoDefinitionResponse) -> Vec<LocationOut> {
    match r {
        GotoDefinitionResponse::Scalar(l) => vec![loc(&l.uri, &l.range)],
        GotoDefinitionResponse::Array(a) => a.iter().map(|l| loc(&l.uri, &l.range)).collect(),
        GotoDefinitionResponse::Link(a) => a.iter().map(|l| loc(&l.target_uri, &l.target_selection_range)).collect(),
    }
}

// ───────────────────────── parametry (typowane) ─────────────────────────

pub fn initialize_params(root_path: &str) -> Result<InitializeParams, String> {
    let root = uri(root_path)?;
    let name = root_path.trim_end_matches('/').rsplit('/').next().unwrap_or("workspace").to_string();
    let mut caps = ClientCapabilities::default();
    caps.text_document = Some(TextDocumentClientCapabilities {
        synchronization: Some(TextDocumentSyncClientCapabilities { dynamic_registration: Some(false), will_save: Some(false), will_save_wait_until: Some(false), did_save: Some(false) }),
        hover: Some(HoverClientCapabilities { dynamic_registration: Some(false), content_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]) }),
        completion: Some(CompletionClientCapabilities {
            completion_item: Some(CompletionItemCapability {
                snippet_support: Some(true),
                documentation_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
                ..Default::default()
            }),
            ..Default::default()
        }),
        definition: Some(GotoCapability { dynamic_registration: Some(false), link_support: Some(true) }),
        publish_diagnostics: Some(PublishDiagnosticsClientCapabilities::default()),
        ..Default::default()
    });
    caps.workspace = Some(WorkspaceClientCapabilities { workspace_folders: Some(true), configuration: Some(true), ..Default::default() });
    Ok(InitializeParams {
        process_id: Some(std::process::id()),
        capabilities: caps,
        client_info: Some(ClientInfo { name: "Blue Code".into(), version: Some(env!("CARGO_PKG_VERSION").into()) }),
        workspace_folders: Some(vec![WorkspaceFolder { uri: root, name }]),
        ..Default::default()
    })
}

fn position_params(path: &str, line: u32, character: u32) -> Result<TextDocumentPositionParams, String> {
    Ok(TextDocumentPositionParams { text_document: TextDocumentIdentifier { uri: uri(path)? }, position: Position { line, character } })
}

// ───────────────────────── komendy ─────────────────────────

/// Handshake `initialize` + `initialized`; idempotentny.
#[tauri::command]
pub async fn lsp_initialize(language: String, root_path: String) -> Result<(), String> {
    let key = key_of(&language, &root_path);
    if session(&key).initialized.load(Ordering::SeqCst) {
        return Ok(());
    }
    let resp = call::<request::Initialize>(&key, initialize_params(&root_path)?).await?;
    result_of(resp)?;
    notify::<notification::Initialized>(&key, InitializedParams {})?;
    session(&key).initialized.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn lsp_did_open(language: String, root_path: String, path: String, language_id: String, text: String) -> Result<(), String> {
    let key = key_of(&language, &root_path);
    let u = uri(&path)?;
    session(&key).versions.lock().unwrap().insert(u.as_str().to_string(), 1);
    notify::<notification::DidOpenTextDocument>(&key, DidOpenTextDocumentParams {
        text_document: TextDocumentItem { uri: u, language_id, version: 1, text },
    })
}

/// Pełna synchronizacja treści (najprostszy tryb, obsługiwany przez każdy serwer).
#[tauri::command]
pub fn lsp_did_change(language: String, root_path: String, path: String, text: String) -> Result<(), String> {
    let key = key_of(&language, &root_path);
    let u = uri(&path)?;
    let version = {
        let s = session(&key);
        let mut v = s.versions.lock().unwrap();
        let e = v.entry(u.as_str().to_string()).or_insert(1);
        *e += 1;
        *e
    };
    notify::<notification::DidChangeTextDocument>(&key, DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier { uri: u, version },
        content_changes: vec![TextDocumentContentChangeEvent { range: None, range_length: None, text }],
    })
}

#[tauri::command]
pub fn lsp_did_close(language: String, root_path: String, path: String) -> Result<(), String> {
    let key = key_of(&language, &root_path);
    let u = uri(&path)?;
    session(&key).versions.lock().unwrap().remove(u.as_str());
    notify::<notification::DidCloseTextDocument>(&key, DidCloseTextDocumentParams { text_document: TextDocumentIdentifier { uri: u } })
}

#[tauri::command]
pub async fn lsp_hover(language: String, root_path: String, path: String, line: u32, character: u32) -> Result<Option<HoverOut>, String> {
    let key = key_of(&language, &root_path);
    let params = HoverParams { text_document_position_params: position_params(&path, line, character)?, work_done_progress_params: Default::default() };
    let v = result_of(call::<request::HoverRequest>(&key, params).await?)?;
    Ok(serde_json::from_value::<Option<Hover>>(v).map_err(|e| e.to_string())?.and_then(hover_to_out))
}

#[tauri::command]
pub async fn lsp_completion(language: String, root_path: String, path: String, line: u32, character: u32) -> Result<Vec<CompletionOut>, String> {
    let key = key_of(&language, &root_path);
    let params = CompletionParams {
        text_document_position: position_params(&path, line, character)?,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    };
    let v = result_of(call::<request::Completion>(&key, params).await?)?;
    Ok(serde_json::from_value::<Option<CompletionResponse>>(v).map_err(|e| e.to_string())?.map(completion_to_out).unwrap_or_default())
}

#[tauri::command]
pub async fn lsp_definition(language: String, root_path: String, path: String, line: u32, character: u32) -> Result<Vec<LocationOut>, String> {
    let key = key_of(&language, &root_path);
    let params = GotoDefinitionParams {
        text_document_position_params: position_params(&path, line, character)?,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let v = result_of(call::<request::GotoDefinition>(&key, params).await?)?;
    Ok(serde_json::from_value::<Option<GotoDefinitionResponse>>(v).map_err(|e| e.to_string())?.map(definition_to_out).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_round_trip_with_spaces_and_unicode() {
        let p = "/home/u/mój projekt/a b.rs";
        let u = path_to_uri(p);
        assert_eq!(u, "file:///home/u/m%C3%B3j%20projekt/a%20b.rs");
        assert_eq!(uri_to_path(&u).as_deref(), Some(p));
        assert!(Uri::from_str(&u).is_ok());
        assert!(uri_to_path("http://x").is_none());
    }

    #[test]
    fn initialize_params_are_typed_and_complete() {
        let p = initialize_params("/home/u/proj").unwrap();
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["workspaceFolders"][0]["name"], "proj");
        assert_eq!(v["workspaceFolders"][0]["uri"], "file:///home/u/proj");
        assert_eq!(v["clientInfo"]["name"], "Blue Code");
        assert_eq!(v["capabilities"]["textDocument"]["completion"]["completionItem"]["snippetSupport"], true);
        assert!(v["capabilities"]["textDocument"]["publishDiagnostics"].is_object());
    }

    #[test]
    fn responses_are_correlated_and_server_requests_answered() {
        assert_eq!(answer_server_request("workspace/configuration", &json!({"items":[{},{},{}]})), json!([null, null, null]));
        assert_eq!(answer_server_request("client/registerCapability", &json!({})), Value::Null);
        assert!(result_of(json!({"id":1,"error":{"code":-1,"message":"boom"}})).unwrap_err().contains("boom"));
        assert_eq!(result_of(json!({"id":1,"result":null})).unwrap(), Value::Null);
    }

    #[test]
    fn hover_variants_become_text() {
        let m = Hover { contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: "**fn** foo".into() }), range: None };
        assert_eq!(hover_to_out(m).unwrap().contents, "**fn** foo");
        let a = Hover { contents: HoverContents::Array(vec![MarkedString::String("a".into()), MarkedString::LanguageString(LanguageString { language: "rust".into(), value: "x".into() })]), range: None };
        assert_eq!(hover_to_out(a).unwrap().contents, "a\n\n```rust\nx\n```");
        let e = Hover { contents: HoverContents::Scalar(MarkedString::String("  ".into())), range: None };
        assert!(hover_to_out(e).is_none());
    }

    #[test]
    fn completion_and_definition_are_mapped() {
        let r: CompletionResponse = serde_json::from_value(json!({"isIncomplete":false,"items":[
            {"label":"println!","kind":3,"insertText":"println!(\"$1\")","insertTextFormat":2,"detail":"macro"},
            {"label":"plain","textEdit":{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},"newText":"plain_text"}}]})).unwrap();
        let out = completion_to_out(r);
        assert_eq!(out.len(), 2);
        assert!(out[0].snippet && out[0].insert_text.contains("$1") && out[0].kind == Some(3));
        assert_eq!(out[1].insert_text, "plain_text");
        let d: GotoDefinitionResponse = serde_json::from_value(json!({"uri":"file:///a/b%20c.rs","range":{"start":{"line":3,"character":4},"end":{"line":3,"character":9}}})).unwrap();
        let l = definition_to_out(d);
        assert_eq!((l[0].path.as_str(), l[0].line, l[0].character), ("/a/b c.rs", 3, 4));
    }

    #[test]
    fn diagnostics_severity_mapping() {
        let d = Diagnostic { range: Range::default(), severity: Some(DiagnosticSeverity::WARNING), message: "uwaga".into(), code: Some(NumberOrString::Number(7)), ..Default::default() };
        let o = DiagnosticOut::from(&d);
        assert_eq!((o.severity, o.code.as_deref()), (2, Some("7")));
        assert_eq!(DiagnosticOut::from(&Diagnostic { message: "x".into(), ..Default::default() }).severity, 1);
    }
}
