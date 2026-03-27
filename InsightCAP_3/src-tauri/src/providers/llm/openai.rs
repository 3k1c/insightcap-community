use reqwest::Client;
use serde_json::json;

use crate::providers::llm::{LLMError, LLMOptions, LLMProvider};

pub struct OpenAiProvider {
    api_key: String,
    base_url: String,
    model: String,
    client: Client,
}

impl OpenAiProvider {
    pub fn new(api_key: String, base_url_opt: Option<String>, model: String) -> Self {
        let mut base_url = base_url_opt
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "http://localhost:11434/v1".to_string()); // 預設降級為本地 ollama (如果在前端沒有設定)

        let trimmed = base_url.trim_end_matches('/');
        if trimmed.contains("localhost") || trimmed.contains("127.0.0.1") || trimmed.contains(":11434") {
            // 如果是本地模型，且沒有以 /v1 結尾，自動補齊以相容 OpenAI API
            if !trimmed.ends_with("/v1") {
                base_url = format!("{}/v1", trimmed);
            }
        } else if trimmed == "https://api.openai.com" {
            base_url = "https://api.openai.com/v1".to_string();
        }

        Self {
            api_key,
            base_url,
            model,
            client: Client::new(),
        }
    }
}

impl LLMProvider for OpenAiProvider {
    async fn complete(&self, prompt: &str, options: LLMOptions) -> Result<String, LLMError> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let req_body = json!({
            "model": self.model,
            "messages": [
                { "role": "user", "content": prompt }
            ],
            "temperature": options.temperature,
            "max_tokens": options.max_tokens,
            "stream": options.stream,
        });

        let res = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&req_body)
            .send()
            .await
            .map_err(|e| LLMError::Network(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(LLMError::Api(format!("{} - {}", status, err_body)));
        }

        let json_res: serde_json::Value = res.json().await.map_err(|e| LLMError::Parse(e.to_string()))?;

        if let Some(text) = json_res["choices"][0]["message"]["content"].as_str() {
            Ok(text.to_string())
        } else {
            Err(LLMError::Parse("Unexpected API response format".to_string()))
        }
    }

    async fn complete_json(&self, prompt: &str, options: LLMOptions) -> Result<serde_json::Value, LLMError> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let req_body = json!({
            "model": self.model,
            "messages": [
                { "role": "user", "content": prompt }
            ],
            "temperature": options.temperature,
            "max_tokens": options.max_tokens,
            "stream": options.stream,
            "response_format": { "type": "json_object" }
        });

        let res = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&req_body)
            .send()
            .await
            .map_err(|e| LLMError::Network(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(LLMError::Api(format!("{} - {}", status, err_body)));
        }

        let json_res: serde_json::Value = res.json().await.map_err(|e| LLMError::Parse(e.to_string()))?;

        if let Some(text) = json_res["choices"][0]["message"]["content"].as_str() {
            let mut clean_text = text.trim();
            if clean_text.starts_with("```json") {
                clean_text = clean_text.trim_start_matches("```json").trim_end_matches("```").trim();
            } else if clean_text.starts_with("```") {
                clean_text = clean_text.trim_start_matches("```").trim_end_matches("```").trim();
            }
            
            let parsed: serde_json::Value = serde_json::from_str(clean_text)
                .map_err(|e| LLMError::Parse(format!("Failed to parse JSON string from LLM: {}\nRaw: {}", e, clean_text)))?;
            Ok(parsed)
        } else {
            Err(LLMError::Parse("Unexpected API response format for JSON".to_string()))
        }
    }
}
