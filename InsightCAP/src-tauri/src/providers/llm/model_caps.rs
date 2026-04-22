#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningStyle {
    None,
    OpenAiReasoning,
    DeepSeekReasoning,
    OllamaThinkTag,
    Gemma4Think,
}

pub fn detect(model: &str, provider: &str) -> ReasoningStyle {
    let m = model.to_lowercase();
    let p = provider.to_lowercase();

    if is_openai_reasoning_model(&m) && matches!(p.as_str(), "openai" | "xai") {
        return ReasoningStyle::OpenAiReasoning;
    }

    if m.contains("deepseek-r1") || m.contains("deepseek_r1") {
        if p == "ollama" {
            return ReasoningStyle::OllamaThinkTag;
        }
        return ReasoningStyle::DeepSeekReasoning;
    }

    if is_gemini_reasoning_model(&m) {
        return ReasoningStyle::DeepSeekReasoning;
    }

    if p == "ollama" && (m.contains("gemma4") || m.contains("gemma-4")) {
        return ReasoningStyle::Gemma4Think;
    }

    if p == "ollama" && (m.contains("qwq") || m.contains("thinking") || m.contains("qwen3")) {
        return ReasoningStyle::OllamaThinkTag;
    }

    ReasoningStyle::None
}

fn is_gemini_reasoning_model(model: &str) -> bool {
    if !model.contains("gemini") {
        return false;
    }

    for prefix in ["gemini-", "gemini/"] {
        if let Some(rest) = model.split(prefix).nth(1) {
            if let Some(ch) = rest.chars().next() {
                if let Some(major) = ch.to_digit(10) {
                    if major >= 3 {
                        return true;
                    }
                    if major == 2 {
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
    if model.contains("gpt-4o") || model.contains("gpt4o") {
        return false;
    }

    for pat in ["o1", "o3", "o4-mini"] {
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
        assert_eq!(
            detect("o1-preview", "openai"),
            ReasoningStyle::OpenAiReasoning
        );
    }

    #[test]
    fn test_not_reasoning() {
        assert_eq!(detect("gpt-4o", "openai"), ReasoningStyle::None);
        assert_eq!(detect("gpt-4o-mini", "openai"), ReasoningStyle::None);
        assert_eq!(detect("qwen2.5:7b", "ollama"), ReasoningStyle::None);
        assert_eq!(
            detect("claude-3.5-sonnet", "anthropic"),
            ReasoningStyle::None
        );
        assert_eq!(detect("gemini-2.0-flash", "google"), ReasoningStyle::None);
    }

    #[test]
    fn test_gemini_reasoning() {
        assert_eq!(
            detect("gemini-2.5-pro", "google"),
            ReasoningStyle::DeepSeekReasoning
        );
        assert_eq!(
            detect("gemini-2.5-flash", "google"),
            ReasoningStyle::DeepSeekReasoning
        );
        assert_eq!(
            detect("gemini-3-flash-preview", "google"),
            ReasoningStyle::DeepSeekReasoning
        );
        assert_eq!(
            detect("gemini-3.1-pro-preview", "google"),
            ReasoningStyle::DeepSeekReasoning
        );
        assert_eq!(
            detect("google/gemini-2.5-pro-preview", "openrouter"),
            ReasoningStyle::DeepSeekReasoning
        );
    }

    #[test]
    fn test_deepseek_r1() {
        assert_eq!(
            detect("deepseek-r1", "ollama"),
            ReasoningStyle::OllamaThinkTag
        );
        assert_eq!(
            detect("deepseek-r1:latest", "ollama"),
            ReasoningStyle::OllamaThinkTag
        );
        assert_eq!(
            detect("deepseek/deepseek-r1", "openrouter"),
            ReasoningStyle::DeepSeekReasoning
        );
    }

    #[test]
    fn test_ollama_think_tag() {
        assert_eq!(detect("qwq:32b", "ollama"), ReasoningStyle::OllamaThinkTag);
    }
}
