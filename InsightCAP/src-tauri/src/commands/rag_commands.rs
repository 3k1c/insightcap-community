use crate::db::AppState;
use crate::providers::llm::model_caps;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider, StreamToken};
use crate::services::rag_engine::RagEngine;
use crate::services::web_search::tavily_search;
use tauri::{Emitter, State};

fn build_editor_ai_rewrite_system_prompt(instruction_override: Option<&str>) -> String {
    let mut parts = vec![
        "You are an editor rewrite assistant inside InsightCAP.".to_string(),
        "Rewrite only the provided selected text according to the requested action.".to_string(),
        "Preserve the original meaning unless the action explicitly asks for a change.".to_string(),
        "Do not explain the rewrite, do not include markdown fences, and return only the final replacement text.".to_string(),
    ];

    if let Some(instruction) = instruction_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!(
            "Global editor rewrite preference:\n{}",
            instruction
        ));
    }

    parts.join("\n\n")
}

fn build_editor_ai_rewrite_user_prompt(action_prompt: &str, selected_text: &str) -> String {
    format!(
        "Action prompt:\n{}\n\nSelected text:\n{}",
        action_prompt.trim(),
        selected_text
    )
}

fn should_use_fast_chat_path(
    _rag_enabled: bool,
    _web_enabled: bool,
    _source_ids: Option<&Vec<String>>,
    _tag_filter: Option<&Vec<String>>,
    _temp_chunk_ids: Option<&Vec<String>>,
) -> bool {
    // 為了確保 Pattern/Log/Reminder 始終生效，我們不再使用快速路徑
    false
}

fn build_fast_chat_system_prompt(
    conversation_summary: Option<String>,
    user_instruction: &str,
) -> String {
    let summary = conversation_summary.unwrap_or_default();
    let summary = summary.trim();
    let instruction = user_instruction.trim();

    if summary.is_empty() && instruction.is_empty() {
        return crate::prompts::RAG_SYSTEM_BASE.to_string();
    }

    let mut parts = vec![crate::prompts::RAG_SYSTEM_BASE.to_string()];
    if !summary.is_empty() {
        parts.push(format!("## Conversation Summary\n{}", summary));
    }
    parts.push(crate::prompts::RAG_SYSTEM_PRIORITY.to_string());
    if !instruction.is_empty() {
        parts.push(format!("## User Instruction\n{}", instruction));
    }
    parts.join("\n\n")
}

#[tauri::command]
pub async fn rag_query(
    state: State<'_, AppState>,
    query: String,
    history: Option<Vec<(String, String)>>,
    conversation_summary: Option<String>,
    project_id: Option<String>,
    source_ids: Option<Vec<String>>,
    tag_filter: Option<Vec<String>>,
    rag_enabled: Option<bool>,
    web_enabled: Option<bool>,
    temp_chunk_ids: Option<Vec<String>>,
    thinking_mode: Option<String>,
) -> Result<serde_json::Value, String> {
    let web_context = if web_enabled.unwrap_or(false) {
        let settings = crate::settings::store::get_settings(&state.db)
            .await
            .map_err(|e| e.to_string())?;
        let ws = &settings.web_search;
        if ws.enabled && !ws.api_key.is_empty() {
            match tavily_search(&ws.api_key, &query).await {
                Ok((ctx, _)) => Some(ctx),
                Err(e) => {
                    eprintln!("[WebSearch] Search failed: {}", e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let engine = RagEngine::new(
        state.db.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );
    engine
        .generate_answer(
            &query,
            history.unwrap_or_default(),
            conversation_summary,
            project_id,
            source_ids,
            tag_filter,
            rag_enabled.unwrap_or(true),
            temp_chunk_ids,
            thinking_mode.as_deref().unwrap_or("normal") == "think",
            web_context,
            None,
        )
        .await
}

#[tauri::command]
pub async fn editor_ai_rewrite_stream(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    action_prompt: String,
    selected_text: String,
    conversation_id: String,
) -> Result<(), String> {
    if action_prompt.trim().is_empty() {
        return Err("Action prompt is empty".to_string());
    }
    if selected_text.trim().is_empty() {
        return Err("Selected text is empty".to_string());
    }

    let settings = crate::settings::store::get_settings(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    let cfg = settings.ai_models.chat_llm;
    let is_ollama = cfg.provider == "ollama";
    let api_key = cfg.api_key.clone().unwrap_or_default();

    if api_key.is_empty() && !is_ollama {
        return Err("LLM API key not configured".to_string());
    }

    let system_prompt = build_editor_ai_rewrite_system_prompt(
        settings.editor.prompt_instruction_override.as_deref(),
    );
    let user_prompt = build_editor_ai_rewrite_user_prompt(&action_prompt, &selected_text);

    let llm = OpenAiProvider::new(
        api_key,
        cfg.base_url.clone(),
        cfg.model.clone(),
        cfg.provider.clone(),
    );
    let app_clone = app.clone();
    let conv_id = conversation_id.clone();
    let llm_opts = LLMOptions {
        temperature: 0.2,
        max_tokens: 4096,
        stream: true,
        think_mode: Some(false),
    };

    let stream_result = llm
        .complete_stream(
            &system_prompt,
            &[],
            &user_prompt,
            llm_opts,
            move |token| match &token {
                StreamToken::Reasoning(r) => {
                    let _ = app_clone.emit(
                        "rag-stream-reasoning",
                        serde_json::json!({
                            "conversationId": conv_id,
                            "token": r,
                        }),
                    );
                }
                StreamToken::Content(c) => {
                    let _ = app_clone.emit(
                        "rag-stream-token",
                        serde_json::json!({
                            "conversationId": conv_id,
                            "token": c,
                        }),
                    );
                }
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("rag-stream-done", serde_json::json!({
        "conversationId": conversation_id,
        "fullAnswer": stream_result.content,
        "reasoning": if stream_result.reasoning.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(stream_result.reasoning) },
        "citationSources": [],
        "contextHints": serde_json::json!({ "mode": "editor_ai_rewrite" }),
    }));

    Ok(())
}

#[tauri::command]
pub async fn rag_query_stream(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    query: String,
    conversation_id: String,
    history: Option<Vec<(String, String)>>,
    conversation_summary: Option<String>,
    project_id: Option<String>,
    source_ids: Option<Vec<String>>,
    tag_filter: Option<Vec<String>>,
    rag_enabled: Option<bool>,
    web_enabled: Option<bool>,
    temp_chunk_ids: Option<Vec<String>>,
    thinking_mode: Option<String>,
) -> Result<(), String> {
    let is_think = thinking_mode.as_deref().unwrap_or("normal") == "think";
    let rag_enabled_value = rag_enabled.unwrap_or(true);
    let web_enabled_value = web_enabled.unwrap_or(false);

    let settings = crate::settings::store::get_settings(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    let (web_ctx_text, web_sources) = if web_enabled_value {
        let ws = &settings.web_search;
        if ws.enabled && !ws.api_key.is_empty() {
            match tavily_search(&ws.api_key, &query).await {
                Ok((ctx, srcs)) => (Some(ctx), srcs),
                Err(e) => {
                    eprintln!("[WebSearch] Search failed: {}", e);
                    (None, vec![])
                }
            }
        } else {
            (None, vec![])
        }
    } else {
        (None, vec![])
    };

    let use_fast_chat = should_use_fast_chat_path(
        rag_enabled_value,
        web_enabled_value,
        source_ids.as_ref(),
        tag_filter.as_ref(),
        temp_chunk_ids.as_ref(),
    );

    let (base_prompt, history_vec, citation_sources, context_hints) = if use_fast_chat {
        (
            build_fast_chat_system_prompt(
                conversation_summary,
                settings.chat_prompt_instruction.trim(),
            ),
            history.unwrap_or_default(),
            Vec::new(),
            serde_json::json!({ "mode": "fast_chat" }),
        )
    } else {
        let engine = RagEngine::new(
            state.db.clone(),
            state.vector_store.clone(),
            state.embedder.clone(),
        );

        let (prompt, history_vec, mut citations, context_hints) = engine
            .build_prompt(
                &query,
                history.unwrap_or_default(),
                conversation_summary,
                project_id,
                source_ids,
                tag_filter,
                rag_enabled_value,
                temp_chunk_ids,
                None,
            )
            .await?;

        for src in &web_sources {
            if !citations.iter().any(|s| s == src) {
                citations.push(src.clone());
            }
        }

        (prompt, history_vec, citations, context_hints)
    };

    let cfg = settings.ai_models.chat_llm;
    let is_ollama = cfg.provider == "ollama";
    let api_key = cfg.api_key.clone().unwrap_or_default();

    if api_key.is_empty() && !is_ollama {
        return Err("LLM API key not configured".to_string());
    }

    let reasoning_style = model_caps::detect(&cfg.model, &cfg.provider);
    let thinking_control = model_caps::thinking_control(&cfg.model, &cfg.provider);
    let has_native_reasoning = reasoning_style != model_caps::ReasoningStyle::None;

    let base_prompt_with_web = if let Some(web_ctx) = &web_ctx_text {
        format!(
            "{}\n\n{}\n{}",
            base_prompt,
            crate::prompts::RAG_CONTEXT_WEB_SEARCH,
            web_ctx
        )
    } else {
        base_prompt
    };

    let system_prompt = if is_think
        && thinking_control == model_caps::ThinkingControl::None
        && !has_native_reasoning
    {
        format!(
            "{}{}",
            crate::prompts::THINK_MODE_PREFIX,
            base_prompt_with_web
        )
    } else {
        base_prompt_with_web
    };

    let llm = OpenAiProvider::new(
        api_key,
        cfg.base_url.clone(),
        cfg.model.clone(),
        cfg.provider.clone(),
    );
    let app_clone = app.clone();
    let conv_id = conversation_id.clone();

    let llm_opts = if is_think {
        LLMOptions {
            temperature: 0.6,
            max_tokens: 8192,
            stream: true,
            think_mode: Some(thinking_control != model_caps::ThinkingControl::None),
        }
    } else {
        LLMOptions {
            stream: true,
            think_mode: Some(false),
            ..LLMOptions::default()
        }
    };

    let stream_result = llm
        .complete_stream(
            &system_prompt,
            &history_vec,
            &query,
            llm_opts,
            move |token| match &token {
                StreamToken::Reasoning(r) => {
                    let _ = app_clone.emit(
                        "rag-stream-reasoning",
                        serde_json::json!({
                            "conversationId": conv_id,
                            "token": r,
                        }),
                    );
                }
                StreamToken::Content(c) => {
                    let _ = app_clone.emit(
                        "rag-stream-token",
                        serde_json::json!({
                            "conversationId": conv_id,
                            "token": c,
                        }),
                    );
                }
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("rag-stream-done", serde_json::json!({
        "conversationId": conversation_id,
        "fullAnswer": stream_result.content,
        "reasoning": if stream_result.reasoning.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(stream_result.reasoning) },
        "citationSources": citation_sources,
        "contextHints": context_hints,
    }));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_editor_ai_rewrite_system_prompt, build_editor_ai_rewrite_user_prompt,
        build_fast_chat_system_prompt, should_use_fast_chat_path,
    };

    #[test]
    fn editor_rewrite_system_prompt_includes_global_preference() {
        let prompt =
            build_editor_ai_rewrite_system_prompt(Some("Preserve technical terms in English."));

        assert!(prompt.contains("editor rewrite assistant"));
        assert!(prompt.contains("return only the final replacement text"));
        assert!(prompt.contains("Preserve technical terms in English."));
    }

    #[test]
    fn editor_rewrite_user_prompt_keeps_action_and_selected_text_separate() {
        let prompt =
            build_editor_ai_rewrite_user_prompt("Make it concise", "This is the selected text.");

        assert!(prompt.contains("Action prompt:\nMake it concise"));
        assert!(prompt.contains("Selected text:\nThis is the selected text."));
    }

    #[test]
    fn fast_chat_path_is_disabled_so_memory_context_always_applies() {
        assert!(!should_use_fast_chat_path(false, false, None, None, None));
        assert!(!should_use_fast_chat_path(true, false, None, None, None));
        assert!(!should_use_fast_chat_path(false, true, None, None, None));
        assert!(!should_use_fast_chat_path(
            false,
            false,
            Some(&vec!["source-1".to_string()]),
            None,
            None,
        ));
        assert!(!should_use_fast_chat_path(
            false,
            false,
            None,
            Some(&vec!["PDF".to_string()]),
            None,
        ));
        assert!(!should_use_fast_chat_path(
            false,
            false,
            None,
            None,
            Some(&vec!["temp-1".to_string()]),
        ));
    }

    #[test]
    fn fast_chat_prompt_keeps_summary_and_user_preference() {
        let prompt = build_fast_chat_system_prompt(
            Some("Earlier summary".to_string()),
            "Answer in Traditional Chinese.",
        );

        assert!(prompt.contains(crate::prompts::RAG_SYSTEM_BASE));
        assert!(prompt.contains("Earlier summary"));
        assert!(prompt.contains("Answer in Traditional Chinese."));
    }
}
