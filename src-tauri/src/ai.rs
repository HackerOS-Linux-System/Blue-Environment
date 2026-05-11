use serde::{Deserialize, Serialize};
use reqwest::Client;
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

pub async fn ai_call(request: AICallRequest) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    match request.service.as_str() {
        "chatgpt" => chatgpt_call(&client, &request).await,
        "gemini"  => gemini_call(&client, &request).await,
        "deepseek"=> deepseek_call(&client, &request).await,
        "grok"    => grok_call(&client, &request).await,
        "claude"  => claude_call(&client, &request).await,
        "local"   => local_call(&client, &request).await,
        other     => Err(format!("Unsupported AI service: {}", other)),
    }
}

async fn chatgpt_call(client: &Client, req: &AICallRequest) -> Result<String, String> {
    let api_key = req.api_key.as_deref().ok_or("Missing ChatGPT API key")?;
    let body = ChatCompletionRequest {
        model: req.model.clone(),
        messages: req.messages.clone(),
        temperature: Some(0.7),
        max_tokens: Some(2000),
    };
    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("ChatGPT API error: {}", text));
    }

    let data: ChatCompletionResponse = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    data.choices.first()
        .map(|c| c.message.content.clone())
        .ok_or("No choices in response".to_string())
}

async fn gemini_call(client: &Client, req: &AICallRequest) -> Result<String, String> {
    let api_key = req.api_key.as_deref().ok_or("Missing Gemini API key")?;
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        req.model, api_key
    );
    let contents: Vec<GeminiContent> = req.messages.iter().map(|m| GeminiContent {
        parts: vec![GeminiPart { text: m.content.clone() }],
    }).collect();

    let resp = client
        .post(&url)
        .json(&GeminiRequest { contents })
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Gemini error: {}", resp.text().await.unwrap_or_default()));
    }

    let data: GeminiResponse = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    data.candidates.first()
        .and_then(|c| c.content.parts.first())
        .map(|p| p.text.clone())
        .ok_or("No response from Gemini".to_string())
}

async fn deepseek_call(client: &Client, req: &AICallRequest) -> Result<String, String> {
    let api_key = req.api_key.as_deref().ok_or("Missing DeepSeek API key")?;
    let body = ChatCompletionRequest {
        model: req.model.clone(),
        messages: req.messages.clone(),
        temperature: Some(0.7),
        max_tokens: Some(2000),
    };
    let resp = client
        .post("https://api.deepseek.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("DeepSeek error: {}", resp.text().await.unwrap_or_default()));
    }

    let data: ChatCompletionResponse = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    data.choices.first()
        .map(|c| c.message.content.clone())
        .ok_or("No choices".to_string())
}

async fn grok_call(_client: &Client, _req: &AICallRequest) -> Result<String, String> {
    Err("Grok API not yet publicly available — check xAI docs for the latest endpoint.".to_string())
}

async fn claude_call(client: &Client, req: &AICallRequest) -> Result<String, String> {
    let api_key = req.api_key.as_deref().ok_or("Missing Claude API key")?;
    let body = serde_json::json!({
        "model": req.model,
        "messages": req.messages,
        "max_tokens": 2000,
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Claude error: {}", resp.text().await.unwrap_or_default()));
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    data["content"].as_array()
        .and_then(|a| a.first())
        .and_then(|c| c["text"].as_str())
        .map(|s| s.to_string())
        .ok_or("No content in Claude response".to_string())
}

async fn local_call(client: &Client, req: &AICallRequest) -> Result<String, String> {
    let prompt = req.messages.last().map(|m| m.content.clone()).unwrap_or_default();
    let body = serde_json::json!({
        "model": req.model,
        "prompt": prompt,
        "stream": false,
    });

    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama request error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Ollama error: {}", resp.text().await.unwrap_or_default()));
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    data["response"].as_str()
        .map(|s| s.to_string())
        .ok_or("No response from local model".to_string())
}
