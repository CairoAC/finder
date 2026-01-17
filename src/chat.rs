use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::mpsc;

const API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const MODEL: &str = "google/gemini-3-flash-preview";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Debug, Deserialize)]
struct StreamResponse {
    choices: Vec<Choice>,
}

pub fn find_api_key() -> Option<String> {
    if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
        if !key.is_empty() {
            return Some(key);
        }
    }

    let paths = [
        std::env::current_dir().ok().map(|p| p.join(".env")),
        dirs::home_dir().map(|p| p.join(".env")),
    ];

    for path in paths.into_iter().flatten() {
        if let Some(key) = read_env_file(&path) {
            return Some(key);
        }
    }

    None
}

fn read_env_file(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("OPENROUTER_API_KEY") {
            if let Some(value) = line.split('=').nth(1) {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

pub async fn stream_chat(
    api_key: &str,
    messages: Vec<ChatMessage>,
    tx: mpsc::UnboundedSender<String>,
) -> Result<(), String> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": MODEL,
        "messages": messages,
        "stream": true,
        "max_tokens": 4096,
    });

    let response = client
        .post(API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("API error {}: {}", status, text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if line.starts_with("data: ") {
                let data = &line[6..];
                if data == "[DONE]" {
                    let _ = tx.send("\n[DONE]".to_string());
                    return Ok(());
                }

                if let Ok(parsed) = serde_json::from_str::<StreamResponse>(data) {
                    if let Some(choice) = parsed.choices.first() {
                        if let Some(content) = &choice.delta.content {
                            let _ = tx.send(content.clone());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
