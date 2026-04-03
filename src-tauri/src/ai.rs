// src-tauri/src/ai.rs
// Handles AI API calls for ChatGPT, Gemini, DeepSeek, Grok, Claude, and local models.

use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;

// ────────────────────────────────────────────────────────────────────────────
// Brakujące definicje typów (dodane)
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AIConfig {
    pub service: String,
    pub model: String,
    pub api_key: Option<String>,
    pub temperature: f32,
    pub max_tokens: u32,
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

// ────────────────────────────────────────────────────────────────────────────
// Struktury dla żądań i odpowiedzi API
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct AICallRequest {
    pub service: String,
    pub api_key: Option<String>,
    pub model: String,
    pub messages: Vec<AIMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AIMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<AIMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: AIMessage,
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContentResponse,
}

#[derive(Debug, Deserialize)]
struct GeminiContentResponse {
    parts: Vec<GeminiPartResponse>,
}

#[derive(Debug, Deserialize)]
struct GeminiPartResponse {
    text: String,
}

// ────────────────────────────────────────────────────────────────────────────
// Główna funkcja wysyłająca zapytanie do wybranego serwisu AI
// ────────────────────────────────────────────────────────────────────────────

pub async fn ai_call(request: AICallRequest) -> Result<String, String> {
    let client = Client::builder()
    .timeout(Duration::from_secs(60))
    .build()
    .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    match request.service.as_str() {
        "chatgpt" => chatgpt_call(&client, &request).await,
        "gemini" => gemini_call(&client, &request).await,
        "deepseek" => deepseek_call(&client, &request).await,
        "grok" => grok_call(&client, &request).await,
        "claude" => claude_call(&client, &request).await,
        "local" => local_call(&client, &request).await,
        _ => Err(format!("Unsupported AI service: {}", request.service)),
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Implementacje poszczególnych serwisów
// ────────────────────────────────────────────────────────────────────────────

async fn chatgpt_call(client: &Client, req: &AICallRequest) -> Result<String, String> {
    let api_key = req.api_key.as_ref().ok_or("Missing API key for ChatGPT")?;
    let url = "https://api.openai.com/v1/chat/completions";
    let request_body = ChatCompletionRequest {
        model: req.model.clone(),
        messages: req.messages.clone(),
        temperature: Some(0.7),
        max_tokens: Some(2000),
    };
    let response = client
    .post(url)
    .header("Authorization", format!("Bearer {}", api_key))
    .json(&request_body)
    .send()
    .await
    .map_err(|e| format!("Request failed: {}", e))?;
    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("ChatGPT API error: {}", text));
    }
    let data: ChatCompletionResponse = response
    .json()
    .await
    .map_err(|e| format!("Failed to parse response: {}", e))?;
    if let Some(choice) = data.choices.first() {
        Ok(choice.message.content.clone())
    } else {
        Err("No completion returned".to_string())
    }
}

async fn gemini_call(client: &Client, req: &AICallRequest) -> Result<String, String> {
    let api_key = req.api_key.as_ref().ok_or("Missing API key for Gemini")?;
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        req.model, api_key
    );
    let contents: Vec<GeminiContent> = req
    .messages
    .iter()
    .map(|msg| GeminiContent {
        parts: vec![GeminiPart {
            text: msg.content.clone(),
        }],
    })
    .collect();
    let request_body = GeminiRequest { contents };
    let response = client
    .post(&url)
    .json(&request_body)
    .send()
    .await
    .map_err(|e| format!("Request failed: {}", e))?;
    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Gemini API error: {}", text));
    }
    let data: GeminiResponse = response
    .json()
    .await
    .map_err(|e| format!("Failed to parse response: {}", e))?;
    if let Some(candidate) = data.candidates.first() {
        if let Some(part) = candidate.content.parts.first() {
            Ok(part.text.clone())
        } else {
            Err("No text in response".to_string())
        }
    } else {
        Err("No candidate returned".to_string())
    }
}

async fn deepseek_call(client: &Client, req: &AICallRequest) -> Result<String, String> {
    let api_key = req.api_key.as_ref().ok_or("Missing API key for DeepSeek")?;
    let url = "https://api.deepseek.com/v1/chat/completions";
    let request_body = ChatCompletionRequest {
        model: req.model.clone(),
        messages: req.messages.clone(),
        temperature: Some(0.7),
        max_tokens: Some(2000),
    };
    let response = client
    .post(url)
    .header("Authorization", format!("Bearer {}", api_key))
    .json(&request_body)
    .send()
    .await
    .map_err(|e| format!("Request failed: {}", e))?;
    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("DeepSeek API error: {}", text));
    }
    let data: ChatCompletionResponse = response
    .json()
    .await
    .map_err(|e| format!("Failed to parse response: {}", e))?;
    if let Some(choice) = data.choices.first() {
        Ok(choice.message.content.clone())
    } else {
        Err("No completion returned".to_string())
    }
}

async fn grok_call(_client: &Client, _req: &AICallRequest) -> Result<String, String> {
    // Grok API (xAI) – currently not publicly documented; placeholder.
    Err("Grok API not yet implemented. Please provide a valid implementation when API is available.".to_string())
}

async fn claude_call(client: &Client, req: &AICallRequest) -> Result<String, String> {
    let api_key = req.api_key.as_ref().ok_or("Missing API key for Claude")?;
    let url = "https://api.anthropic.com/v1/messages";
    let request_body = serde_json::json!({
        "model": req.model,
        "messages": req.messages,
        "max_tokens": 2000,
        "temperature": 0.7,
    });
    let response = client
    .post(url)
    .header("x-api-key", api_key)
    .header("anthropic-version", "2023-06-01")
    .json(&request_body)
    .send()
    .await
    .map_err(|e| format!("Request failed: {}", e))?;
    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Claude API error: {}", text));
    }
    let data: serde_json::Value = response
    .json()
    .await
    .map_err(|e| format!("Failed to parse response: {}", e))?;
    if let Some(content) = data["content"].as_array().and_then(|arr| arr.first()) {
        if let Some(text) = content["text"].as_str() {
            Ok(text.to_string())
        } else {
            Err("No text in response".to_string())
        }
    } else {
        Err("No content in response".to_string())
    }
}

async fn local_call(client: &Client, req: &AICallRequest) -> Result<String, String> {
    // Assume local Ollama endpoint: http://localhost:11434/api/generate
    let url = "http://localhost:11434/api/generate";
    let prompt = req.messages.last().map(|m| m.content.clone()).unwrap_or_default();
    let request_body = serde_json::json!({
        "model": req.model,
        "prompt": prompt,
        "stream": false,
    });
    let response = client
    .post(url)
    .json(&request_body)
    .send()
    .await
    .map_err(|e| format!("Request failed: {}", e))?;
    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama API error: {}", text));
    }
    let data: serde_json::Value = response
    .json()
    .await
    .map_err(|e| format!("Failed to parse response: {}", e))?;
    if let Some(response_text) = data["response"].as_str() {
        Ok(response_text.to_string())
    } else {
        Err("No response from local model".to_string())
    }
}
