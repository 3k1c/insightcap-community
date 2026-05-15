use futures_util::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::time::Instant;

use crate::providers::llm::model_caps::{self, ReasoningStyle};
use crate::providers::llm::{LLMError, LLMOptions, LLMProvider, StreamResult, StreamToken};

pub struct OpenAiProvider {
    api_key: String,
    base_url: String,
    model: String,
    provider_name: String,
    client: Client,
}

#[derive(Clone)]
pub(crate) struct LlmTimingTrace {
    trace_id: String,
    started_at: Instant,
}

impl LlmTimingTrace {
    pub(crate) fn new(trace_id: String) -> Self {
        Self {
            trace_id,
            started_at: Instant::now(),
        }
    }

    pub(crate) fn log(&self, stage: &str, detail: Option<&str>) {
        eprintln!(
            "{}",
            format_chat_timing_log(
                &self.trace_id,
                stage,
                self.started_at.elapsed().as_millis(),
                detail,
            )
        );
    }
}

pub(crate) fn format_chat_timing_log(
    trace_id: &str,
    stage: &str,
    elapsed_ms: u128,
    detail: Option<&str>,
) -> String {
    match detail {
        Some(detail) if !detail.is_empty() => format!(
            "[ChatTiming] trace={} stage={} elapsedMs={} {}",
            trace_id, stage, elapsed_ms, detail
        ),
        _ => format!(
            "[ChatTiming] trace={} stage={} elapsedMs={}",
            trace_id, stage, elapsed_ms
        ),
    }
}

fn summarize_sse_event(event: &serde_json::Value) -> String {
    let choice = &event["choices"][0];
    let delta = &choice["delta"];
    let delta_keys = delta
        .as_object()
        .map(|obj| {
            let mut keys = obj.keys().map(String::as_str).collect::<Vec<_>>();
            keys.sort_unstable();
            if keys.is_empty() {
                "none".to_string()
            } else {
                keys.join(",")
            }
        })
        .unwrap_or_else(|| "none".to_string());
    let role = delta["role"].as_str().unwrap_or("none");
    let content = delta["content"].as_str();
    let reasoning = delta["reasoning_content"].as_str();
    let finish_reason = choice["finish_reason"].as_str().unwrap_or("null");

    format!(
        "deltaKeys={} role={} contentLen={} contentPreview=\"{}\" reasoningLen={} finishReason={}",
        delta_keys,
        role,
        content.map_or(0, |s| s.chars().count()),
        content.map_or_else(String::new, |s| sanitize_log_preview(s, 80)),
        reasoning.map_or(0, |s| s.chars().count()),
        finish_reason
    )
}

fn summarize_content_delta(content: &str) -> String {
    format!(
        "contentLen={} contentPreview=\"{}\"",
        content.chars().count(),
        sanitize_log_preview(content, 80)
    )
}

fn sanitize_log_preview(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut truncated = false;
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max_chars {
            truncated = true;
            break;
        }
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    if truncated {
        out.push_str("...");
    }
    out
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

    fn build_request_body(
        &self,
        messages: Vec<serde_json::Value>,
        options: &LLMOptions,
        stream: bool,
    ) -> serde_json::Value {
        let style = model_caps::detect(&self.model, &self.provider_name);

        match style {
            ReasoningStyle::OpenAiReasoning => {
                let reasoning_effort = match options.think_mode {
                    Some(true) => "high",
                    Some(false) => "low",
                    None => "high",
                };
                json!({
                    "model": self.model,
                    "messages": messages,
                    "max_completion_tokens": options.max_tokens,
                    "reasoning_effort": reasoning_effort,
                    "stream": stream,
                })
            }
            ReasoningStyle::OllamaThinkTag => {
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

    pub(crate) async fn complete_stream_traced(
        &self,
        system_prompt: &str,
        history: &[(String, String)],
        user_query: &str,
        options: LLMOptions,
        trace: Option<LlmTimingTrace>,
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

        let request_detail = format!("provider={} model={}", self.provider_name, self.model);
        if let Some(trace) = trace.as_ref() {
            trace.log("http_request_start", Some(&request_detail));
        }

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
        let mut first_chunk_logged = false;
        let mut first_event_logged = false;
        let mut first_content_delta_logged = false;

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| LLMError::Network(e.to_string()))?;
            if !first_chunk_logged {
                first_chunk_logged = true;
                if let Some(trace) = trace.as_ref() {
                    let detail = format!("bytes={}", bytes.len());
                    trace.log("first_sse_chunk", Some(&detail));
                }
            }
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(event) = pop_sse_event(&mut buffer) {
                for line in event.lines() {
                    let data = sse_data_payload(line).unwrap_or_default();
                    if data == "[DONE]" {
                        break;
                    }
                    if data.is_empty() {
                        continue;
                    }

                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                        if !first_event_logged {
                            first_event_logged = true;
                            if let Some(trace) = trace.as_ref() {
                                let detail = summarize_sse_event(&v);
                                trace.log("first_sse_event", Some(&detail));
                            }
                        }
                        let delta = &v["choices"][0]["delta"];

                        match style {
                            ReasoningStyle::OpenAiReasoning | ReasoningStyle::DeepSeekReasoning => {
                                if let Some(r) = delta["reasoning_content"].as_str() {
                                    if !r.is_empty() {
                                        result.reasoning.push_str(r);
                                        on_token(StreamToken::Reasoning(r.to_string()));
                                    }
                                }
                                if let Some(c) = delta["content"].as_str() {
                                    if !c.is_empty() {
                                        if !first_content_delta_logged {
                                            first_content_delta_logged = true;
                                            if let Some(trace) = trace.as_ref() {
                                                let detail = summarize_content_delta(c);
                                                trace.log("first_content_delta", Some(&detail));
                                            }
                                        }
                                        result.content.push_str(c);
                                        on_token(StreamToken::Content(c.to_string()));
                                    }
                                }
                            }
                            ReasoningStyle::OllamaThinkTag => {
                                if let Some(c) = delta["content"].as_str() {
                                    if !c.is_empty() {
                                        if !first_content_delta_logged {
                                            first_content_delta_logged = true;
                                            if let Some(trace) = trace.as_ref() {
                                                let detail = summarize_content_delta(c);
                                                trace.log("first_content_delta", Some(&detail));
                                            }
                                        }
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
                                if let Some(c) = delta["content"].as_str() {
                                    if !c.is_empty() {
                                        if !first_content_delta_logged {
                                            first_content_delta_logged = true;
                                            if let Some(trace) = trace.as_ref() {
                                                let detail = summarize_content_delta(c);
                                                trace.log("first_content_delta", Some(&detail));
                                            }
                                        }
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
                                        if !first_content_delta_logged {
                                            first_content_delta_logged = true;
                                            if let Some(trace) = trace.as_ref() {
                                                let detail = summarize_content_delta(token);
                                                trace.log("first_content_delta", Some(&detail));
                                            }
                                        }
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
}

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

    fn parse(&mut self, raw: &str) -> Vec<StreamToken> {
        let mut tokens = Vec::new();
        let mut chars = raw.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '<' {
                self.tag_buffer.push(ch);
            } else if !self.tag_buffer.is_empty() {
                self.tag_buffer.push(ch);

                if self.tag_buffer == "<think>" {
                    self.in_think = true;
                    self.tag_buffer.clear();
                } else if self.tag_buffer == "</think>" {
                    self.in_think = false;
                    self.tag_buffer.clear();
                } else if self.tag_buffer.len() > 8 {
                    let buf = std::mem::take(&mut self.tag_buffer);
                    self.emit(&buf, &mut tokens);
                } else {
                    let potential = if self.in_think { "</think>" } else { "<think>" };
                    if !potential.starts_with(&self.tag_buffer) {
                        let buf = std::mem::take(&mut self.tag_buffer);
                        self.emit(&buf, &mut tokens);
                    }
                }
            } else {
                let s = ch.to_string();
                self.emit(&s, &mut tokens);
            }
        }

        tokens
    }

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

struct Gemma4ChannelParser {
    in_think: bool,
    buf: String,
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
                if self.buf == Self::OPEN {
                    self.in_think = true;
                    self.skip_first_newline = false;
                    self.buf.clear();
                } else {
                    self.flush_regular_content_prefix(&mut tokens);
                }
            } else {
                if self.buf.ends_with(Self::CLOSE) {
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
                } else {
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

    fn flush_regular_content_prefix(&mut self, tokens: &mut Vec<StreamToken>) {
        if self.buf.is_empty() || Self::OPEN.starts_with(self.buf.as_str()) {
            return;
        }

        let keep_from = self
            .buf
            .char_indices()
            .find_map(|(idx, _)| {
                let suffix = &self.buf[idx..];
                if Self::OPEN.starts_with(suffix) {
                    Some(idx)
                } else {
                    None
                }
            })
            .unwrap_or(self.buf.len());

        let content = self.buf[..keep_from].to_string();
        self.buf = self.buf[keep_from..].to_string();
        if !content.is_empty() {
            tokens.push(StreamToken::Content(content));
        }
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

fn pop_sse_event(buffer: &mut String) -> Option<String> {
    let lf_pos = buffer.find("\n\n").map(|pos| (pos, 2));
    let crlf_pos = buffer.find("\r\n\r\n").map(|pos| (pos, 4));

    let (pos, delimiter_len) = match (lf_pos, crlf_pos) {
        (Some(lf), Some(crlf)) => {
            if lf.0 <= crlf.0 {
                lf
            } else {
                crlf
            }
        }
        (Some(lf), None) => lf,
        (None, Some(crlf)) => crlf,
        (None, None) => return None,
    };

    let event = buffer[..pos].to_string();
    buffer.drain(..pos + delimiter_len);
    Some(event)
}

fn sse_data_payload(line: &str) -> Option<&str> {
    line.strip_prefix("data:").map(str::trim_start)
}

#[cfg(test)]
mod tests {
    use super::{
        format_chat_timing_log, pop_sse_event, sse_data_payload, summarize_sse_event,
        Gemma4ChannelParser, OpenAiProvider,
    };
    use crate::providers::llm::LLMOptions;
    use crate::providers::llm::StreamToken;
    use serde_json::json;

    #[test]
    fn pops_crlf_sse_events_without_waiting_for_stream_end() {
        let mut buffer =
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\r\n\r\nrest".to_string();

        let event = pop_sse_event(&mut buffer).expect("event");

        assert!(event.contains("\"Hel\""));
        assert_eq!(buffer, "rest");
    }

    #[test]
    fn accepts_data_lines_without_space_after_colon() {
        assert_eq!(sse_data_payload("data:{\"x\":1}"), Some("{\"x\":1}"));
        assert_eq!(sse_data_payload("data: {\"x\":1}"), Some("{\"x\":1}"));
    }

    #[test]
    fn formats_chat_timing_log_with_trace_stage_and_elapsed_time() {
        let line = format_chat_timing_log("conv-1", "first_sse_chunk", 42, Some("bytes=128"));

        assert_eq!(
            line,
            "[ChatTiming] trace=conv-1 stage=first_sse_chunk elapsedMs=42 bytes=128"
        );
    }

    #[test]
    fn summarizes_sse_event_delta_shape_and_content_preview() {
        let event = json!({
            "choices": [{
                "delta": {
                    "role": "assistant",
                    "content": "<|channel>thought\nPlan"
                },
                "finish_reason": null
            }]
        });

        let summary = summarize_sse_event(&event);

        assert!(summary.contains("deltaKeys=content,role"));
        assert!(summary.contains("role=assistant"));
        assert!(summary.contains("contentLen=22"));
        assert!(summary.contains("contentPreview=\"<|channel>thought\\nPlan\""));
        assert!(summary.contains("finishReason=null"));
    }

    #[test]
    fn gemma4_parser_emits_regular_content_without_waiting_for_flush() {
        let mut parser = Gemma4ChannelParser::new();

        let tokens = parser.parse("你");

        assert!(matches!(tokens.as_slice(), [StreamToken::Content(s)] if s == "你"));
    }

    #[test]
    fn openai_reasoning_effort_follows_thinking_mode() {
        let provider = OpenAiProvider::new(
            "key".to_string(),
            None,
            "o3".to_string(),
            "openai".to_string(),
        );
        let messages = vec![json!({ "role": "user", "content": "hi" })];

        let normal = provider.build_request_body(
            messages.clone(),
            &LLMOptions {
                think_mode: Some(false),
                ..LLMOptions::default()
            },
            true,
        );
        let thinking = provider.build_request_body(
            messages,
            &LLMOptions {
                think_mode: Some(true),
                ..LLMOptions::default()
            },
            true,
        );

        assert_eq!(normal["reasoning_effort"], "low");
        assert_eq!(thinking["reasoning_effort"], "high");
    }
}

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
        self.complete_stream_traced(system_prompt, history, user_query, options, None, on_token)
            .await
    }

    async fn complete_json(
        &self,
        prompt: &str,
        options: LLMOptions,
    ) -> Result<serde_json::Value, LLMError> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let messages = vec![json!({ "role": "user", "content": prompt })];
        let mut req_body = self.build_request_body(messages, &options, options.stream);

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
