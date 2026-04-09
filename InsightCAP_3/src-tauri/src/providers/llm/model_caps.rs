/// 模型原生推理能力偵測
///
/// 根據 model name 和 provider 判斷該模型支援哪種 reasoning API，
/// 讓 OpenAiProvider 能自動適配請求格式與回應解析。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningStyle {
    /// 普通模型，無原生推理能力
    None,
    /// OpenAI o-series (o1, o3, o4-mini)：
    /// - 使用 `developer` role（非 system）
    /// - 不支援 temperature
    /// - 使用 max_completion_tokens（非 max_tokens）
    /// - reasoning 在 delta.reasoning_content
    OpenAiReasoning,
    /// DeepSeek-R1 / Gemini 2.5+ via API（非 Ollama）：
    /// - 標準 OpenAI 相容格式
    /// - reasoning 在 delta.reasoning_content
    DeepSeekReasoning,
    /// DeepSeek-R1 via Ollama 或其他本地推理模型：
    /// - reasoning 以 <think>...</think> tag 包裹在 content 中
    OllamaThinkTag,
}

/// 根據模型名稱和 provider 偵測推理風格
pub fn detect(model: &str, provider: &str) -> ReasoningStyle {
    let m = model.to_lowercase();
    let p = provider.to_lowercase();

    // OpenAI o-series：o1, o3, o4-mini（排除 gpt-4o 系列）
    if is_openai_reasoning_model(&m) && matches!(p.as_str(), "openai" | "xai") {
        return ReasoningStyle::OpenAiReasoning;
    }

    // DeepSeek-R1
    if m.contains("deepseek-r1") || m.contains("deepseek_r1") {
        if p == "ollama" {
            return ReasoningStyle::OllamaThinkTag;
        }
        return ReasoningStyle::DeepSeekReasoning;
    }

    // Gemini 2.5+ 系列（透過 Google OpenAI 相容端點或 OpenRouter）
    // reasoning 在 delta.reasoning_content
    if is_gemini_reasoning_model(&m) {
        return ReasoningStyle::DeepSeekReasoning;
    }

    // Ollama 上支援 "think" 參數的模型（輸出以 <think> tag 包裹）
    // 包含：qwq、gemma4、qwen3、以及名稱含 "thinking" 的模型
    if p == "ollama" && (
        m.contains("qwq")
        || m.contains("thinking")
        || m.contains("gemma4")
        || m.contains("gemma-4")
        || m.contains("qwen3")
    ) {
        return ReasoningStyle::OllamaThinkTag;
    }

    ReasoningStyle::None
}

fn is_gemini_reasoning_model(model: &str) -> bool {
    // Gemini 2.5+、3.x 系列支援 thinking
    // 匹配：gemini-2.5-pro, gemini-2.5-flash, gemini-3-flash-preview, gemini-3.1-pro-preview
    // 也匹配 OpenRouter 格式：google/gemini-2.5-pro-preview
    if !model.contains("gemini") {
        return false;
    }
    // 提取版本號
    for prefix in ["gemini-", "gemini/"] {
        if let Some(rest) = model.split(prefix).nth(1) {
            if let Some(ch) = rest.chars().next() {
                if let Some(major) = ch.to_digit(10) {
                    // 2.5 以上算 reasoning 模型
                    if major >= 3 {
                        return true;
                    }
                    if major == 2 {
                        // 檢查是否 >= 2.5
                        let after = &rest[1..];
                        if after.starts_with(".5") || after.starts_with("5") {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn is_openai_reasoning_model(model: &str) -> bool {
    // 匹配 o1, o1-mini, o1-preview, o3, o3-mini, o4-mini 等
    // 但排除 gpt-4o, gpt-4o-mini（這些不是 reasoning 模型）
    if model.contains("gpt-4o") || model.contains("gpt4o") {
        return false;
    }
    // 檢查是否以 o1/o3/o4 開頭或包含這些模式
    let patterns = ["o1", "o3", "o4-mini"];
    for pat in patterns {
        if model.starts_with(pat) || model.contains(&format!("/{}", pat)) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_reasoning() {
        assert_eq!(detect("o3", "openai"), ReasoningStyle::OpenAiReasoning);
        assert_eq!(detect("o3-mini", "openai"), ReasoningStyle::OpenAiReasoning);
        assert_eq!(detect("o4-mini", "openai"), ReasoningStyle::OpenAiReasoning);
        assert_eq!(detect("o1-preview", "openai"), ReasoningStyle::OpenAiReasoning);
    }

    #[test]
    fn test_not_reasoning() {
        assert_eq!(detect("gpt-4o", "openai"), ReasoningStyle::None);
        assert_eq!(detect("gpt-4o-mini", "openai"), ReasoningStyle::None);
        assert_eq!(detect("qwen2.5:7b", "ollama"), ReasoningStyle::None);
        assert_eq!(detect("claude-3.5-sonnet", "anthropic"), ReasoningStyle::None);
        assert_eq!(detect("gemini-2.0-flash", "google"), ReasoningStyle::None);
    }

    #[test]
    fn test_gemini_reasoning() {
        assert_eq!(detect("gemini-2.5-pro", "google"), ReasoningStyle::DeepSeekReasoning);
        assert_eq!(detect("gemini-2.5-flash", "google"), ReasoningStyle::DeepSeekReasoning);
        assert_eq!(detect("gemini-3-flash-preview", "google"), ReasoningStyle::DeepSeekReasoning);
        assert_eq!(detect("gemini-3.1-pro-preview", "google"), ReasoningStyle::DeepSeekReasoning);
        assert_eq!(detect("google/gemini-2.5-pro-preview", "openrouter"), ReasoningStyle::DeepSeekReasoning);
    }

    #[test]
    fn test_deepseek_r1() {
        assert_eq!(detect("deepseek-r1", "ollama"), ReasoningStyle::OllamaThinkTag);
        assert_eq!(detect("deepseek-r1:latest", "ollama"), ReasoningStyle::OllamaThinkTag);
        assert_eq!(detect("deepseek/deepseek-r1", "openrouter"), ReasoningStyle::DeepSeekReasoning);
    }

    #[test]
    fn test_ollama_think_tag() {
        assert_eq!(detect("qwq:32b", "ollama"), ReasoningStyle::OllamaThinkTag);
    }
}
