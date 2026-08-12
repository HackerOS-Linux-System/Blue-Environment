use serde::{Deserialize, Serialize};
use tauri_plugin_http::reqwest::Client;
use std::time::Duration;
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AIConfig {
    pub service: String,
    pub model: String,
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub source: String,
    pub installed: bool,
    pub update_available: Option<bool>,
    pub icon: Option<String>,
    pub size: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AICallRequest {
    pub service: String,
    pub api_key: Option<String>,
    pub model: String,
    pub messages: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaStatus {
    pub reachable: bool,
    pub models: Vec<String>,
    /// Populated only when `reachable` is false — the raw connection
    /// error, so the Setup screen can show *why* (connection refused vs.
    /// timeout vs. something else) instead of just "not available".
    pub error: Option<String>,
}

/// Every cloud provider below previously shared one very sharp-edged
/// failure mode: none of them checked the HTTP status code before trying
/// to pull `choices`/`content` out of the response body. A 401 (bad key),
/// 404 (unknown/decommissioned model), or 429 (rate limited) response
/// would just fail the `.ok_or_else(...)` with whatever generic fallback
/// string was hardcoded for that branch — or, for Gemini, a bare "Gemini
/// error" with no indication at all of what actually went wrong. From the
/// user's side this reads as "it just doesn't work", indistinguishable
/// from a real bug, when it might just be a stale/decommissioned model
/// name or a copy-pasted key with a stray space. This helper centralizes
/// status checking so every provider surfaces a real, specific message.
async fn send_and_check(
    req: tauri_plugin_http::reqwest::RequestBuilder,
    provider: &str,
) -> Result<serde_json::Value, String> {
    let resp = req.send().await.map_err(|e| {
        format!("{provider}: couldn't reach the API ({e}). Check your internet connection.")
    })?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("{provider}: failed reading response ({e})"))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|_| serde_json::json!({ "raw": text }));

    if !status.is_success() {
        let detail = json["error"]["message"].as_str()
            .or_else(|| json["error"].as_str())
            .or_else(|| json["message"].as_str())
            .unwrap_or_else(|| json["raw"].as_str().unwrap_or("no error detail in response"));
        let hint = match status.as_u16() {
            401 | 403 => " — check that your API key is correct and active.",
            404 => " — the model name may be wrong or no longer available from this provider.",
            429 => " — you've hit a rate limit or quota; wait a bit or check your plan/billing.",
            500..=599 => " — this looks like a problem on the provider's end, not yours.",
            _ => "",
        };
        return Err(format!("{provider}: HTTP {}: {}{}", status.as_u16(), detail, hint));
    }
    Ok(json)
}

pub async fn ai_call(request: AICallRequest) -> Result<String, String> {
    let api_key = request.api_key.clone().unwrap_or_default();

    // Fail fast with a clear message instead of sending a request with a
    // blank Authorization header and waiting for a 401 to come back.
    if request.service != "local" && api_key.trim().is_empty() {
        return Err(format!(
            "No API key configured for {}. Open Settings in Blue AI and paste one in.",
            request.service
        ));
    }

    let client = Client::builder()
    .timeout(Duration::from_secs(60))
    .build()
    .map_err(|e| e.to_string())?;

    match request.service.as_str() {
        "chatgpt" => {
            let body = serde_json::json!({ "model": request.model, "messages": request.messages });
            let json = send_and_check(
                client.post("https://api.openai.com/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&body),
                "ChatGPT",
            ).await?;
            json["choices"][0]["message"]["content"].as_str().map(|s| s.to_string())
                .ok_or_else(|| "ChatGPT: response had no message content".to_string())
        }
        "claude" => {
            let body = serde_json::json!({ "model": request.model, "max_tokens": 4096, "messages": request.messages });
            let json = send_and_check(
                client.post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("Content-Type", "application/json")
                    .json(&body),
                "Claude",
            ).await?;
            json["content"][0]["text"].as_str().map(|s| s.to_string())
                .ok_or_else(|| "Claude: response had no text content".to_string())
        }
        "gemini" => {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                request.model, api_key
            );
            let contents: Vec<serde_json::Value> = request.messages.iter().map(|m| {
                serde_json::json!({
                    "role": if m["role"] == "assistant" { "model" } else { "user" },
                    "parts": [{ "text": m["content"] }]
                })
            }).collect();
            let body = serde_json::json!({ "contents": contents });
            let json = send_and_check(
                client.post(&url).header("Content-Type", "application/json").json(&body),
                "Gemini",
            ).await?;
            json["candidates"][0]["content"]["parts"][0]["text"].as_str().map(|s| s.to_string())
                .ok_or_else(|| "Gemini: response had no text content — it may have been blocked by a safety filter".to_string())
        }
        "deepseek" => {
            let body = serde_json::json!({ "model": request.model, "messages": request.messages });
            let json = send_and_check(
                client.post("https://api.deepseek.com/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&body),
                "DeepSeek",
            ).await?;
            json["choices"][0]["message"]["content"].as_str().map(|s| s.to_string())
                .ok_or_else(|| "DeepSeek: response had no message content".to_string())
        }
        "grok" => {
            let body = serde_json::json!({ "model": request.model, "messages": request.messages });
            let json = send_and_check(
                client.post("https://api.x.ai/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&body),
                "Grok",
            ).await?;
            json["choices"][0]["message"]["content"].as_str().map(|s| s.to_string())
                .ok_or_else(|| "Grok: response had no message content".to_string())
        }
        "local" => {
            let body = serde_json::json!({ "model": request.model, "messages": request.messages, "stream": false });
            let resp = client
                .post("http://localhost:11434/api/chat")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!(
                    "Local AI (Ollama): couldn't reach http://localhost:11434 ({e}). \
                     Is Ollama installed and running? Use the Setup button in Blue AI's settings to check."
                ))?;
            let status = resp.status();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(format!("Local AI (Ollama): HTTP {} — {}", status.as_u16(), text));
            }
            let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            json["message"]["content"].as_str().map(|s| s.to_string())
                .ok_or_else(|| format!(
                    "Local AI (Ollama): no content in response — is the model '{}' pulled? Try: ollama pull {}",
                    request.model, request.model
                ))
        }
        other => Err(format!("Unknown AI service: {}", other)),
    }
}

/// Powers Blue AI's local-AI Setup screen: pings Ollama's own API
/// (rather than assuming the hardcoded model list in aiServices.ts is
/// accurate) so the picker only ever offers models the user has actually
/// pulled, and can tell them plainly if Ollama isn't running at all
/// instead of failing later, mid-conversation, with a confusing error.
pub async fn check_ollama_status() -> OllamaStatus {
    let client = match Client::builder().timeout(Duration::from_secs(4)).build() {
        Ok(c) => c,
        Err(e) => return OllamaStatus { reachable: false, models: vec![], error: Some(e.to_string()) },
    };

    match client.get("http://localhost:11434/api/tags").send().await {
        Ok(resp) if resp.status().is_success() => {
            let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
            let models = json["models"].as_array()
                .map(|arr| arr.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect())
                .unwrap_or_default();
            OllamaStatus { reachable: true, models, error: None }
        }
        Ok(resp) => OllamaStatus {
            reachable: false, models: vec![],
            error: Some(format!("Ollama responded with HTTP {}", resp.status().as_u16())),
        },
        Err(e) => OllamaStatus {
            reachable: false, models: vec![],
            error: Some(if e.is_connect() {
                "Connection refused — Ollama doesn't appear to be running.".to_string()
            } else if e.is_timeout() {
                "Connection timed out.".to_string()
            } else {
                e.to_string()
            }),
        },
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct OllamaInstallProgress {
    pub line: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct OllamaInstallDone {
    pub success: bool,
    pub error: Option<String>,
}

/// Runs Ollama's own official install script (`curl -fsSL
/// https://ollama.com/install.sh | sh`) and streams its output back as
/// `blue-ai://ollama-install-progress` events. This deliberately doesn't
/// reimplement GPU/hardware detection — the official script already
/// does that itself (it prints which GPU driver it found, e.g. NVIDIA/
/// AMD ROCm, or that it's falling back to CPU-only) and streaming its
/// real output back to the Setup screen shows the user exactly that,
/// truthfully, rather than a second, separately-maintained detection
/// path that could drift out of sync with what the script actually
/// supports. Runs under `pkexec` as a single upfront privilege prompt —
/// the script needs root to install the systemd service and the
/// `/usr/local/bin/ollama` binary, and running it under `pkexec` avoids
/// it hitting a nested, non-interactive `sudo` prompt partway through
/// that would otherwise just hang.
pub async fn install_ollama(app: AppHandle) {
    std::thread::spawn(move || {
        let mut child = match Command::new("pkexec")
            .args(["sh", "-c", "curl -fsSL https://ollama.com/install.sh | sh"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit("blue-ai://ollama-install-done", OllamaInstallDone {
                    success: false, error: Some(format!("Failed to start installer: {e}")),
                });
                return;
            }
        };

        // The script writes its progress/detection messages to stderr
        // (curl's own progress meter also goes to stderr) as much as
        // stdout, so both are merged into one line stream for the UI —
        // the person watching the Setup screen doesn't care which
        // stream a given line came from.
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let app_stdout = app.clone();
        let stdout_handle = std::thread::spawn(move || {
            if let Some(out) = stdout {
                for line in BufReader::new(out).lines().flatten() {
                    let _ = app_stdout.emit("blue-ai://ollama-install-progress", OllamaInstallProgress { line });
                }
            }
        });
        let app_stderr = app.clone();
        let stderr_handle = std::thread::spawn(move || {
            if let Some(err) = stderr {
                for line in BufReader::new(err).lines().flatten() {
                    let _ = app_stderr.emit("blue-ai://ollama-install-progress", OllamaInstallProgress { line });
                }
            }
        });

        let status = child.wait();
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();

        match status {
            Ok(s) if s.success() => {
                let _ = app.emit("blue-ai://ollama-install-done", OllamaInstallDone { success: true, error: None });
            }
            Ok(s) => {
                let _ = app.emit("blue-ai://ollama-install-done", OllamaInstallDone {
                    success: false, error: Some(format!("Installer exited with status {s}")),
                });
            }
            Err(e) => {
                let _ = app.emit("blue-ai://ollama-install-done", OllamaInstallDone {
                    success: false, error: Some(e.to_string()),
                });
            }
        }
    });
}
