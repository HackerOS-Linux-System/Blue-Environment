use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter};

#[derive(Serialize)]
pub struct LspResult {
    pub success: bool,
    pub error: Option<String>,
}

struct LspProcess {
    child: Child,
    stdin: ChildStdin,
}

static LSP_PROCESSES: Lazy<Mutex<HashMap<String, LspProcess>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn lsp_binary_for(language: &str) -> Option<(&'static str, Vec<&'static str>)> {
    match language {
        "typescript" | "javascript" => Some(("typescript-language-server", vec!["--stdio"])),
        "rust"                       => Some(("rust-analyzer", vec![])),
        "python"                     => Some(("pylsp", vec![])),
        "go"                         => Some(("gopls", vec!["serve"])),
        "cpp" | "c"                  => Some(("clangd", vec![])),
        _ => None,
    }
}

fn binary_exists(bin: &str) -> bool {
    Command::new("which").arg(bin).output().map(|o| o.status.success()).unwrap_or(false)
}

/// Reads one `Content-Length`-framed LSP message from `reader`. Returns
/// `Ok(None)` on clean EOF (server exited / stdout closed) so the
/// caller's read loop can end quietly rather than treating process exit
/// as an error.
fn read_one_lsp_message<R: Read>(reader: &mut R) -> std::io::Result<Option<Value>> {
    // Headers are ASCII, CRLF-terminated, ending with a blank line —
    // read byte-by-byte rather than via a buffered line reader so we
    // don't accidentally consume bytes belonging to the JSON body that
    // follows (a line-oriented read could over-read past the
    // header/body boundary).
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            if reader.read(&mut byte)? == 0 {
                return Ok(None); // EOF mid-header — process exited
            }
            line.push(byte[0]);
            if line.ends_with(b"\r\n") { break; }
        }
        if line == b"\r\n" {
            break; // blank line — end of headers
        }
        let line_str = String::from_utf8_lossy(&line);
        if let Some(rest) = line_str.trim().strip_prefix("Content-Length:") {
            content_length = rest.trim().parse().ok();
        }
        // Any other header (e.g. Content-Type) is read and ignored —
        // LSP servers rarely send one, and UTF-8 JSON is the spec
        // default when they don't.
    }
    let Some(len) = content_length else {
        return Ok(None); // malformed frame — no Content-Length header; give up on this message
    };
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    match serde_json::from_slice::<Value>(&body) {
        Ok(v) => Ok(Some(v)),
        Err(_) => Ok(None), // malformed JSON body — drop it rather than kill the whole reader loop
    }
}

#[tauri::command]
pub fn start_language_server(app: AppHandle, language: String, root_path: String) -> LspResult {
    let key = format!("{}::{}", language, root_path);

    if LSP_PROCESSES.lock().unwrap().contains_key(&key) {
        return LspResult { success: true, error: None };
    }

    let Some((bin, args)) = lsp_binary_for(&language) else {
        return LspResult { success: false, error: Some(format!("No LSP mapping for language '{}'", language)) };
    };

    if !binary_exists(bin) {
        return LspResult {
            success: false,
            error: Some(format!("'{}' not found on PATH — install it to enable {} IntelliSense", bin, language)),
        };
    }

    match Command::new(bin)
        .args(&args)
        .current_dir(&root_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            let stdin = child.stdin.take().expect("piped stdin");
            let stdout = child.stdout.take().expect("piped stdout");

            // Background reader: decodes each Content-Length-framed
            // message and forwards it to the frontend. This is the
            // actual fix for the bug described in this module's doc —
            // previously nothing ever read this stdout at all.
            let event_name = format!("lsp-message-{key}");
            let closed_event_name = format!("lsp-closed-{key}");
            let app_for_reader = app.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    match read_one_lsp_message(&mut reader) {
                        Ok(Some(msg)) => {
                            let _ = app_for_reader.emit(&event_name, msg);
                        }
                        Ok(None) => break, // EOF or unrecoverable frame — server is done
                        Err(_) => break,
                    }
                }
                // Let the frontend know the server's message stream
                // ended (crash, clean exit, or unreadable pipe) so it
                // can stop waiting on any still-pending requests instead
                // of hanging forever.
                let _ = app_for_reader.emit(&closed_event_name, ());
            });

            LSP_PROCESSES.lock().unwrap().insert(key, LspProcess { child, stdin });
            LspResult { success: true, error: None }
        }
        Err(e) => LspResult { success: false, error: Some(format!("Failed to spawn {}: {}", bin, e)) },
    }
}

#[tauri::command]
pub fn stop_language_server(language: String, root_path: String) -> bool {
    let key = format!("{}::{}", language, root_path);
    if let Some(mut proc) = LSP_PROCESSES.lock().unwrap().remove(&key) {
        let _ = proc.child.kill();
        true
    } else {
        false
    }
}

/// Writes one JSON-RPC message to the server's stdin, `Content-Length`
/// framed. `message` should already be a well-formed JSON-RPC 2.0
/// object (built by the frontend's `lspClient.ts`) — see this module's
/// doc comment on why framing/transport and protocol are kept separate.
#[tauri::command]
pub fn lsp_send_message(language: String, root_path: String, message: Value) -> Result<(), String> {
    let key = format!("{}::{}", language, root_path);
    let mut procs = LSP_PROCESSES.lock().unwrap();
    let proc = procs.get_mut(&key).ok_or_else(|| format!("no running language server for {key}"))?;
    let body = serde_json::to_vec(&message).map_err(|e| e.to_string())?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    proc.stdin.write_all(header.as_bytes()).map_err(|e| e.to_string())?;
    proc.stdin.write_all(&body).map_err(|e| e.to_string())?;
    proc.stdin.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn lsp_is_running(language: String, root_path: String) -> bool {
    let key = format!("{}::{}", language, root_path);
    LSP_PROCESSES.lock().unwrap().contains_key(&key)
}
