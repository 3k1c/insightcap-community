use futures_util::StreamExt;
use reqwest::Client;
use serde_json::json;

use crate::providers::llm::model_caps::{self, ReasoningStyle};
use crate::providers::llm::{LLMError, LLMOptions, LLMProvider, StreamResult, StreamToken};

pub struct OpenAiProvider {
    api_key: String,
    base_url: String,
    model: String,
    provider_name: String,
    client: Client,
}

impl OpenAiProvider {
    pub fn new(
        api_key: String,
        base_url_opt: Option<String>,
        model: String,
        provider_name: String,
    ) -> Self {
        let mut base_url = base_url_opt
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| match provider_name.as_str() {
                "openai" => "https://api.openai.com/v1".to_string(),
                "google" => "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
                "xai" => "https://api.x.ai/v1".to_string(),
                "openrouter" => "https://openrouter.ai/api/v1".to_string(),
                _ => "http://localhost:11434/v1".to_string(),
            });

        let trimmed = base_url.trim_end_matches('/');
        if trimmed.contains("localhost")
            || trimmed.contains("127.0.0.1")
            || trimmed.contains(":11434")
        {
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
            provider_name,
            client: Client::new(),
        }
    }

    /// 根據模型能力建構 messages 陣列（system vs developer role）
    fn build_messages(
        &self,
        system_prompt: &str,
        history: &[(String, String)],
        user_query: &str,
        think_mode: bool,
    ) -> Vec<serde_json::Value> {
        let style = model_caps::detect(&self.model, &self.provider_name);
        let system_role = if style == ReasoningStyle::OpenAiReasoning {
            "developer"
        } else {
            "system"
        };

        // Gemma4Think：只有 think_mode=true 時才在 system prompt 開頭加 <|think|>
        let effective_system_prompt;
        let system_content = if style == ReasoningStyle::Gemma4Think && think_mode {
            effective_system_prompt = format!("<|think|>{}", system_prompt);
            effective_system_prompt.as_str()
        } else {
            system_prompt
        };

        let mut messages = vec![json!({ "role": system_role, "content": system_content })];
        for (role, content) in history {
            messages.push(json!({ "role": role, "content": content }));
        }
        messages.push(json!({ "role": "user", "content": user_query }));
        messages
    }

    /// 根據模型能力建構 request body 參數
    fn build_request_body(
        &self,
        messages: Vec<serde_json::Value>,
        options: &LLMOptions,
        stream: bool,
    ) -> serde_json::Value {
        let style = model_caps::detect(&self.model, &self.provider_name);

        match style {
            ReasoningStyle::OpenAiReasoning => {
                // o-series：不支援 temperature，用 max_completion_tokens，加入 reasoning_effort
                json!({
                    "model": self.model,
                    "messages": messages,
                    "max_completion_tokens": options.max_tokens,
                    "reasoning_effort": "high",
                    "stream": stream,
                })
            }
            ReasoningStyle::OllamaThinkTag => {
                // Ollama think-capable 模型（DeepSeek-R1、Qwen3 等）
                // think 參數放在 options 物件內
                let mut body = json!({
                    "model": self.model,
                    "messages": messages,
                    "temperature": options.temperature,
                    "max_tokens": options.max_tokens,
                    "stream": stream,
                });
                if let Some(think) = options.think_mode {
                    body["options"] = json!({ "think": think });
                }
                body
            }
            ReasoningStyle::Gemma4Think => {
                // Gemma4 via Ollama：thinking 由 system prompt 的 <|think|> 控制
                // 建議 sampling 參數：temperature=1.0, top_p=0.95, top_k=64
                json!({
                    "model": self.model,
                    "messages": messages,
                    "temperature": 1.0,
                    "top_p": 0.95,
                    "options": { "top_k": 64 },
                    "max_tokens": options.max_tokens,
                    "stream": stream,
                })
            }
            _ => {
                json!({
                    "model": self.model,
                    "messages": messages,
                    "temperature": options.temperature,
                    "max_tokens": options.max_tokens,
                    "stream": stream,
                })
            }
        }
    }

    fn reasoning_style(&self) -> ReasoningStyle {
        model_caps::detect(&self.model, &self.provider_name)
    }
}

// ── Ollama <think> tag 狀態機 ─────────────────────────────────────────────

/// 解析 Ollama 模型 content 中的 <think>...</think> tag
struct ThinkTagParser {
    in_think: bool,
    tag_buffer: String,
}

impl ThinkTagParser {
    fn new() -> Self {
        Self {
            in_think: false,
            tag_buffer: String::new(),
        }
    }

    /// 處理一段 content token，回傳分類後的 StreamToken 列表
    fn parse(&mut self, raw: &str) -> Vec<StreamToken> {
        let mut tokens = Vec::new();
        let mut chars = raw.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '<' {
                // 開始可能是 tag，先緩衝
                self.tag_buffer.push(ch);
            } else if !self.tag_buffer.is_empty() {
                self.tag_buffer.push(ch);

                // 檢查是否已完成 tag
                if self.tag_buffer == "<think>" {
                    self.in_think = true;
                    self.tag_buffer.clear();
                } else if self.tag_buffer == "</think>" {
                    self.in_think = false;
                    self.tag_buffer.clear();
                } else if self.tag_buffer.len() > 8 {
                    // 不是有效 tag，flush buffer 為內容
                    let buf = std::mem::take(&mut self.tag_buffer);
                    self.emit(&buf, &mut tokens);
                } else {
                    // 可能是 partial tag，看看是否還有可能匹配
                    let potential = if self.in_think { "</think>" } else { "<think>" };
                    if !potential.starts_with(&self.tag_buffer) {
                        let buf = std::mem::take(&mut self.tag_buffer);
                        self.emit(&buf, &mut tokens);
                    }
                }
            } else {
                // 一般字元
                let s = ch.to_string();
                self.emit(&s, &mut tokens);
            }
        }

        tokens
    }

    /// 串流結束時 flush 殘留 buffer
    fn flush(&mut self) -> Vec<StreamToken> {
        if self.tag_buffer.is_empty() {
            return vec![];
        }
        let buf = std::mem::take(&mut self.tag_buffer);
        let mut tokens = Vec::new();
        self.emit(&buf, &mut tokens);
        tokens
    }

    fn emit(&self, text: &str, tokens: &mut Vec<StreamToken>) {
        if text.is_empty() {
            return;
        }
        if self.in_think {
            tokens.push(StreamToken::Reasoning(text.to_string()));
        } else {
            tokens.push(StreamToken::Content(text.to_string()));
        }
    }
}

// ── Gemma4 <|channel>thought\n...<channel|> 狀態機 ───────────────────────
//
// 格式：<|channel>thought\n[thinking content]<channel|>[final answer]
// 策略：以逐字元方式掃描，找到開頭 token 進入 reasoning 模式，
//       找到結尾 token 退出。

struct Gemma4ChannelParser {
    in_think: bool,
    buf: String,
    // 在開頭 token 結束後，first newline 要丟棄（\n 是格式分隔，不是內容）
    skip_first_newline: bool,
}

impl Gemma4ChannelParser {
    const OPEN: &'static str = "<|channel>thought\n";
    const CLOSE: &'static str = "<channel|>";

    fn new() -> Self {
        Self {
            in_think: false,
            buf: String::new(),
            skip_first_newline: false,
        }
    }

    fn parse(&mut self, raw: &str) -> Vec<StreamToken> {
        let mut tokens = Vec::new();
        for ch in raw.chars() {
            self.buf.push(ch);

            if !self.in_think {
                // 等待 OPEN token
                if self.buf.ends_with(Self::OPEN) {
                    self.in_think = true;
                    self.skip_first_newline = false;
                    self.buf.clear();
                } else if Self::OPEN.starts_with(&self.buf as &str) {
                    // partial match，繼續等待
                } else {
                    // 不匹配，flush 為 content
                    let s = std::mem::take(&mut self.buf);
                    if !s.is_empty() {
                        tokens.push(StreamToken::Content(s));
                    }
                }
            } else {
                // 等待 CLOSE token
                if self.buf.ends_with(Self::CLOSE) {
                    // 移除尾巴的 CLOSE token，flush 剩餘為 reasoning
                    let reasoning_part = &self.buf[..self.buf.len() - Self::CLOSE.len()];
                    if !reasoning_part.is_empty() {
                        tokens.push(StreamToken::Reasoning(reasoning_part.to_string()));
                    }
                    self.in_think = false;
                    self.buf.clear();
                } else if Self::CLOSE.starts_with(
                    &*self
                        .buf
                        .chars()
                        .rev()
                        .take(Self::CLOSE.len())
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>(),
                ) {
                    // partial CLOSE — 保留在 buf 繼續等待
                } else {
                    // 沒有 partial CLOSE 風險，可以 flush 到 buf[-CLOSE.len()+1] 為止
                    let safe_len = self.buf.len().saturating_sub(Self::CLOSE.len() - 1);
                    if safe_len > 0 {
                        let flush = self.buf[..safe_len].to_string();
                        self.buf = self.buf[safe_len..].to_string();
                        tokens.push(StreamToken::Reasoning(flush));
                    }
                }
            }
        }
        tokens
    }

    fn flush(&mut self) -> Vec<StreamToken> {
        if self.buf.is_empty() {
            return vec![];
        }
        let s = std::mem::take(&mut self.buf);
        if self.in_think {
            vec![StreamToken::Reasoning(s)]
        } else {
            vec![StreamToken::Content(s)]
        }
    }
}

// ── LLMProvider 實作 ─────────────────────────────────────────────────────

impl LLMProvider for OpenAiProvider {
    async fn complete(&self, prompt: &str, options: LLMOptions) -> Result<String, LLMError> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let messages = vec![json!({ "role": "user", "content": prompt })];
        let req_body = self.build_request_body(messages, &options, options.stream);

        let res = self
            .client
            .post(&url)
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

        let json_res: serde_json::Value = res
            .json()
            .await
            .map_err(|e| LLMError::Parse(e.to_string()))?;

        if let Some(text) = json_res["choices"][0]["message"]["content"].as_str() {
            Ok(text.to_string())
        } else {
            Err(LLMError::Parse(
                "Unexpected API response format".to_string(),
            ))
        }
    }

    async fn complete_with_history(
        &self,
        system_prompt: &str,
        history: &[(String, String)],
        user_query: &str,
        options: LLMOptions,
    ) -> Result<String, LLMError> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let messages = self.build_messages(
            system_prompt,
            history,
            user_query,
            options.think_mode.unwrap_or(false),
        );
        let req_body = self.build_request_body(messages, &options, false);

        let res = self
            .client
            .post(&url)
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

        let json_res: serde_json::Value = res
            .json()
            .await
            .map_err(|e| LLMError::Parse(e.to_string()))?;
        if let Some(text) = json_res["choices"][0]["message"]["content"].as_str() {
            Ok(text.to_string())
        } else {
            Err(LLMError::Parse(
                "Unexpected API response format".to_string(),
            ))
        }
    }

    async fn complete_stream(
        &self,
        system_prompt: &str,
        history: &[(String, String)],
        user_query: &str,
        options: LLMOptions,
        on_token: impl Fn(StreamToken) + Send + 'static,
    ) -> Result<StreamResult, LLMError> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let style = self.reasoning_style();

        let messages = self.build_messages(
            system_prompt,
            history,
            user_query,
            options.think_mode.unwrap_or(false),
        );
        let req_body = self.build_request_body(messages, &options, true);

        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "text/event-stream")
            .json(&req_body)
            .send()
            .await
            .map_err(|e| LLMError::Network(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(LLMError::Api(format!("{} - {}", status, err_body)));
        }

        let mut stream = res.bytes_stream();
        let mut result = StreamResult::default();
        let mut buffer = String::new();
        let mut think_parser = ThinkTagParser::new();
        let mut gemma4_parser = Gemma4ChannelParser::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| LLMError::Network(e.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            // SSE 以 \n\n 分隔事件
            while let Some(pos) = buffer.find("\n\n") {
                let event = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                for line in event.lines() {
                    let data = line.strip_prefix("data: ").unwrap_or_default();
                    if data == "[DONE]" {
                        break;
                    }
                    if data.is_empty() {
                        continue;
                    }

                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                        let delta = &v["choices"][0]["delta"];

                        match style {
                            ReasoningStyle::OpenAiReasoning | ReasoningStyle::DeepSeekReasoning => {
                                // 先處理 reasoning_content
                                if let Some(r) = delta["reasoning_content"].as_str() {
                                    if !r.is_empty() {
                                        result.reasoning.push_str(r);
                                        on_token(StreamToken::Reasoning(r.to_string()));
                                    }
                                }
                                // 再處理 content
                                if let Some(c) = delta["content"].as_str() {
                                    if !c.is_empty() {
                                        result.content.push_str(c);
                                        on_token(StreamToken::Content(c.to_string()));
                                    }
                                }
                            }
                            ReasoningStyle::OllamaThinkTag => {
                                // Ollama：reasoning 包在 <think> tag 中
                                if let Some(c) = delta["content"].as_str() {
                                    if !c.is_empty() {
                                        let parsed = think_parser.parse(c);
                                        for tok in parsed {
                                            match &tok {
                                                StreamToken::Reasoning(r) => {
                                                    result.reasoning.push_str(r)
                                                }
                                                StreamToken::Content(ct) => {
                                                    result.content.push_str(ct)
                                                }
                                            }
                                            on_token(tok);
                                        }
                                    }
                                }
                            }
                            ReasoningStyle::Gemma4Think => {
                                // Gemma4：reasoning 包在 <|channel>thought\n...<channel|> 中
                                if let Some(c) = delta["content"].as_str() {
                                    if !c.is_empty() {
                                        let parsed = gemma4_parser.parse(c);
                                        for tok in parsed {
                                            match &tok {
                                                StreamToken::Reasoning(r) => {
                                                    result.reasoning.push_str(r)
                                                }
                                                StreamToken::Content(ct) => {
                                                    result.content.push_str(ct)
                                                }
                                            }
                                            on_token(tok);
                                        }
                                    }
                                }
                            }
                            ReasoningStyle::None => {
                                if let Some(token) = delta["content"].as_str() {
                                    if !token.is_empty() {
                                        result.content.push_str(token);
                                        on_token(StreamToken::Content(token.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Flush tag parser 殘留
        let flush_tokens = match style {
            ReasoningStyle::OllamaThinkTag => think_parser.flush(),
            ReasoningStyle::Gemma4Think => gemma4_parser.flush(),
            _ => vec![],
        };
        for tok in flush_tokens {
            match &tok {
                StreamToken::Reasoning(r) => result.reasoning.push_str(r),
                StreamToken::Content(c) => result.content.push_str(c),
            }
            on_token(tok);
        }

        Ok(result)
    }

    async fn complete_json(
        &self,
        prompt: &str,
        options: LLMOptions,
    ) -> Result<serde_json::Value, LLMError> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let messages = vec![json!({ "role": "user", "content": prompt })];
        let mut req_body = self.build_request_body(messages, &options, options.stream);

        // Ollama 的 JSON mode 有時會與 think 模型衝突導致輸出空字串，因此在 Ollama 避開強制 json_object
        if self.provider_name != "ollama" {
            req_body["response_format"] = json!({ "type": "json_object" });
        }

        let res = self
            .client
            .post(&url)
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

        let json_res: serde_json::Value = res
            .json()
            .await
            .map_err(|e| LLMError::Parse(e.to_string()))?;

        if let Some(text) = json_res["choices"][0]["message"]["content"].as_str() {
            let mut clean_text = text.trim();

            // 移除可能干擾 JSON 解析的 reasoning tags
            if let Some(end_idx) = clean_text.find("</think>") {
                clean_text = clean_text[end_idx + "</think>".len()..].trim();
            }
            if let Some(end_idx) = clean_text.find("<channel|>") {
                clean_text = clean_text[end_idx + "<channel|>".len()..].trim();
            }

            if clean_text.starts_with("```json") {
                clean_text = clean_text
                    .trim_start_matches("```json")
                    .trim_end_matches("```")
                    .trim();
            } else if clean_text.starts_with("```") {
                clean_text = clean_text
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();
            }

            // 防呆處理：有時候 LLM 會在 JSON 外面再包一層或是前面有奇怪的話
            // 我們直接找第一個 { 和最後一個 }
            let start = clean_text.find('{').unwrap_or(0);
            let end = clean_text
                .rfind('}')
                .map(|i| i + 1)
                .unwrap_or(clean_text.len());
            let final_clean_text = if start < end {
                &clean_text[start..end]
            } else {
                clean_text
            };

            let parsed: serde_json::Value =
                serde_json::from_str(final_clean_text).map_err(|e| {
                    LLMError::Parse(format!(
                        "Failed to parse JSON string from LLM: {}\nRaw: {}",
                        e, final_clean_text
                    ))
                })?;
            Ok(parsed)
        } else {
            Err(LLMError::Parse(
                "Unexpected API response format for JSON".to_string(),
            ))
        }
    }
}
