use crate::db::AppState;
use crate::providers::llm::model_caps;
use crate::providers::llm::openai::{LlmTimingTrace, OpenAiProvider};
use crate::providers::llm::{LLMOptions, LLMProvider, StreamToken};
use crate::services::rag_engine::RagEngine;
use crate::services::web_search::tavily_search;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{Emitter, State};

fn should_use_fast_chat_path(
    rag_enabled: bool,
    web_enabled: bool,
    source_ids: Option<&Vec<String>>,
    tag_filter: Option<&Vec<String>>,
    temp_chunk_ids: Option<&Vec<String>>,
) -> bool {
    // 純聊天不需要 context tool 時走快速路徑，避免第一個 token 被檢索前處理阻塞。
    rag_enabled == false
        && web_enabled == false
        && source_ids.map_or(true, Vec::is_empty)
        && tag_filter.map_or(true, Vec::is_empty)
        && temp_chunk_ids.map_or(true, Vec::is_empty)
}

fn build_fast_chat_system_prompt(
    conversation_summary: Option<String>,
    user_instruction: &str,
) -> String {
    crate::prompts::build_chat_system_prompt(crate::prompts::ChatPromptInput {
        kind: crate::prompts::ChatPromptKind::Plain,
        conversation_summary: conversation_summary.as_deref(),
        context_sections: &[],
        user_instruction,
    })
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

    let editor_prompts =
        crate::prompts::build_editor_rewrite_prompts(crate::prompts::EditorRewritePromptInput {
            action_prompt: &action_prompt,
            selected_text: &selected_text,
            instruction_override: settings.editor.prompt_instruction_override.as_deref(),
        });

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
            &editor_prompts.system_prompt,
            &[],
            &editor_prompts.user_prompt,
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
    let timing_trace = LlmTimingTrace::new(conversation_id.clone());
    timing_trace.log("backend_received", None);

    let is_think = thinking_mode.as_deref().unwrap_or("normal") == "think";
    let rag_enabled_value = rag_enabled.unwrap_or(true);
    let web_enabled_value = web_enabled.unwrap_or(false);
    let use_fast_chat = should_use_fast_chat_path(
        rag_enabled_value,
        web_enabled_value,
        source_ids.as_ref(),
        tag_filter.as_ref(),
        temp_chunk_ids.as_ref(),
    );
    let fast_path_detail = format!(
        "fastPath={} ragEnabled={} webEnabled={} sourceIds={} tagFilter={} tempChunks={}",
        use_fast_chat,
        rag_enabled_value,
        web_enabled_value,
        source_ids.as_ref().map_or(0, Vec::len),
        tag_filter.as_ref().map_or(0, Vec::len),
        temp_chunk_ids.as_ref().map_or(0, Vec::len)
    );
    timing_trace.log("fast_path_decided", Some(&fast_path_detail));

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
    let first_token_logged = Arc::new(AtomicBool::new(false));
    let first_token_trace = timing_trace.clone();
    let first_token_flag = first_token_logged.clone();

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

    timing_trace.log("llm_request_start", None);

    let stream_result = llm
        .complete_stream_traced(
            &system_prompt,
            &history_vec,
            &query,
            llm_opts,
            Some(timing_trace.clone()),
            move |token| match &token {
                StreamToken::Reasoning(r) => {
                    if !first_token_flag.swap(true, Ordering::Relaxed) {
                        first_token_trace.log("backend_first_stream_token", None);
                    }
                    let _ = app_clone.emit(
                        "rag-stream-reasoning",
                        serde_json::json!({
                            "conversationId": conv_id,
                            "token": r,
                        }),
                    );
                }
                StreamToken::Content(c) => {
                    if !first_token_flag.swap(true, Ordering::Relaxed) {
                        first_token_trace.log("backend_first_stream_token", None);
                    }
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
    use super::{build_fast_chat_system_prompt, should_use_fast_chat_path};

    #[test]
    fn fast_chat_path_is_used_only_for_plain_chat_without_context_tools() {
        assert!(should_use_fast_chat_path(false, false, None, None, None));
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

        assert!(prompt.contains(crate::prompts::CHAT_SYSTEM_BASE));
        assert!(!prompt.contains(crate::prompts::RAG_SYSTEM_BASE));
        assert!(prompt.contains("Earlier summary"));
        assert!(prompt.contains("Answer in Traditional Chinese."));
    }
}
