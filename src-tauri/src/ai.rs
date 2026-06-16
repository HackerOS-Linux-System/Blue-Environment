use serde::{Deserialize, Serialize};
use tauri_plugin_http::reqwest::Client;
use std::time::Duration;

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

pub async fn ai_call(request: AICallRequest) -> Result<String, String> {
    let api_key = request.api_key.clone().unwrap_or_default();
    let client = Client::builder()
    .timeout(Duration::from_secs(60))
    .build()
    .map_err(|e| e.to_string())?;

    match request.service.as_str() {
        "chatgpt" => {
            let body = serde_json::json!({
                "model": request.model,
                "messages": request.messages,
            });
            let resp = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&body).map_err(|e| e.to_string())?)
            .send()
            .await
            .map_err(|e| e.to_string())?;
            let text = resp.text().await.map_err(|e| e.to_string())?;
            let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| json["error"]["message"].as_str().unwrap_or("Unknown error").to_string())
        }
        "claude" => {
            let body = serde_json::json!({
                "model": request.model,
                "max_tokens": 4096,
                "messages": request.messages,
            });
            let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&body).map_err(|e| e.to_string())?)
            .send()
            .await
            .map_err(|e| e.to_string())?;
            let text = resp.text().await.map_err(|e| e.to_string())?;
            let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            json["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| json["error"]["message"].as_str().unwrap_or("Unknown error").to_string())
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
            let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&body).map_err(|e| e.to_string())?)
            .send()
            .await
            .map_err(|e| e.to_string())?;
            let text = resp.text().await.map_err(|e| e.to_string())?;
            let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Gemini error".to_string())
        }
        "deepseek" => {
            let body = serde_json::json!({
                "model": request.model,
                "messages": request.messages,
            });
            let resp = client
            .post("https://api.deepseek.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&body).map_err(|e| e.to_string())?)
            .send()
            .await
            .map_err(|e| e.to_string())?;
            let text = resp.text().await.map_err(|e| e.to_string())?;
            let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| json["error"]["message"].as_str().unwrap_or("Unknown error").to_string())
        }
        "grok" => {
            let body = serde_json::json!({
                "model": request.model,
                "messages": request.messages,
            });
            let resp = client
            .post("https://api.x.ai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&body).map_err(|e| e.to_string())?)
            .send()
            .await
            .map_err(|e| e.to_string())?;
            let text = resp.text().await.map_err(|e| e.to_string())?;
            let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| json["error"]["message"].as_str().unwrap_or("Unknown error").to_string())
        }
        "local" => {
            let body = serde_json::json!({
                "model": request.model,
                "messages": request.messages,
                "stream": false,
            });
            let resp = client
            .post("http://localhost:11434/api/chat")
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&body).map_err(|e| e.to_string())?)
            .send()
            .await
            .map_err(|e| e.to_string())?;
            let text = resp.text().await.map_err(|e| e.to_string())?;
            let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            json["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Ollama error".to_string())
        }
        other => Err(format!("Unknown AI service: {}", other)),
    }
}
