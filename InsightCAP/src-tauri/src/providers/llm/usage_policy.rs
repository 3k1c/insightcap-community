use crate::providers::llm::LLMOptions;
use crate::settings::store::{AIUsageMode, AIUsageSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LLMTaskKind {
    InteractiveChat,
    InteractiveThink,
    EditorRewrite,
    BackgroundSummary,
    BackgroundSynthesis,
    TinyClassification,
}

pub fn apply_llm_usage_policy(
    mut options: LLMOptions,
    usage: &AIUsageSettings,
    task: LLMTaskKind,
) -> LLMOptions {
    let cap = max_tokens_for(usage.mode, task);
    options.max_tokens = options.max_tokens.min(cap);
    options
}

fn max_tokens_for(mode: AIUsageMode, task: LLMTaskKind) -> usize {
    match (mode, task) {
        (_, LLMTaskKind::TinyClassification) => 200,
        (AIUsageMode::Economy, LLMTaskKind::InteractiveThink) => 4096,
        (AIUsageMode::Economy, LLMTaskKind::InteractiveChat) => 1024,
        (AIUsageMode::Economy, LLMTaskKind::EditorRewrite) => 2048,
        (AIUsageMode::Economy, LLMTaskKind::BackgroundSummary) => 600,
        (AIUsageMode::Economy, LLMTaskKind::BackgroundSynthesis) => 600,
        (AIUsageMode::Balanced, LLMTaskKind::InteractiveThink) => 8192,
        (AIUsageMode::Balanced, LLMTaskKind::InteractiveChat) => 2048,
        (AIUsageMode::Balanced, LLMTaskKind::EditorRewrite) => 4096,
        (AIUsageMode::Balanced, LLMTaskKind::BackgroundSummary) => 1024,
        (AIUsageMode::Balanced, LLMTaskKind::BackgroundSynthesis) => 2048,
        (AIUsageMode::Quality, LLMTaskKind::InteractiveThink) => 8192,
        (AIUsageMode::Quality, LLMTaskKind::InteractiveChat) => 4096,
        (AIUsageMode::Quality, LLMTaskKind::EditorRewrite) => 4096,
        (AIUsageMode::Quality, LLMTaskKind::BackgroundSummary) => 1024,
        (AIUsageMode::Quality, LLMTaskKind::BackgroundSynthesis) => 2048,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_policy_keeps_interactive_defaults_and_think_cap() {
        let usage = AIUsageSettings::default();

        let chat = apply_llm_usage_policy(
            LLMOptions {
                max_tokens: 4096,
                ..LLMOptions::default()
            },
            &usage,
            LLMTaskKind::InteractiveChat,
        );
        let think = apply_llm_usage_policy(
            LLMOptions {
                max_tokens: 8192,
                think_mode: Some(true),
                ..LLMOptions::default()
            },
            &usage,
            LLMTaskKind::InteractiveThink,
        );

        assert_eq!(chat.max_tokens, 2048);
        assert_eq!(think.max_tokens, 8192);
    }

    #[test]
    fn economy_policy_caps_background_below_interactive_chat() {
        let usage = AIUsageSettings {
            mode: AIUsageMode::Economy,
        };

        let chat = apply_llm_usage_policy(
            LLMOptions {
                max_tokens: 8192,
                ..LLMOptions::default()
            },
            &usage,
            LLMTaskKind::InteractiveChat,
        );
        let background = apply_llm_usage_policy(
            LLMOptions {
                max_tokens: 8192,
                ..LLMOptions::default()
            },
            &usage,
            LLMTaskKind::BackgroundSynthesis,
        );

        assert_eq!(chat.max_tokens, 1024);
        assert!(background.max_tokens < chat.max_tokens);
    }

    #[test]
    fn quality_policy_does_not_expand_background_to_interactive_limits() {
        let usage = AIUsageSettings {
            mode: AIUsageMode::Quality,
        };

        let background = apply_llm_usage_policy(
            LLMOptions {
                max_tokens: 8192,
                ..LLMOptions::default()
            },
            &usage,
            LLMTaskKind::BackgroundSynthesis,
        );

        assert_eq!(background.max_tokens, 2048);
    }

    #[test]
    fn quality_policy_allows_larger_interactive_chat_outputs() {
        let usage = AIUsageSettings {
            mode: AIUsageMode::Quality,
        };

        let chat = apply_llm_usage_policy(
            LLMOptions {
                max_tokens: 8192,
                ..LLMOptions::default()
            },
            &usage,
            LLMTaskKind::InteractiveChat,
        );

        assert_eq!(chat.max_tokens, 4096);
    }
}
