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
    pub fn new(api_key: String, base_url: Option<String>, model: String) -> Self {
        Self {
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
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
            let parsed: serde_json::Value = serde_json::from_str(text)
                .map_err(|e| LLMError::Parse(format!("Failed to parse JSON string from LLM: {}", e)))?;
            Ok(parsed)
        } else {
            Err(LLMError::Parse("Unexpected API response format for JSON".to_string()))
        }
    }
}
