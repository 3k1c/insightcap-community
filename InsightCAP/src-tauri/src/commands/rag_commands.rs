use tauri::{Emitter, State};
use crate::db::AppState;
use crate::providers::llm::{LLMOptions, LLMProvider, StreamToken};
use crate::providers::llm::model_caps;
use crate::providers::llm::openai::OpenAiProvider;
use crate::services::rag_engine::RagEngine;
use crate::services::web_search::tavily_search;

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
        let settings = crate::settings::store::get_settings(&state.db).await.map_err(|e| e.to_string())?;
        let ws = &settings.web_search;
        if ws.enabled && !ws.api_key.is_empty() {
            match tavily_search(&ws.api_key, &query).await {
                Ok((ctx, _)) => Some(ctx),
                Err(e) => { eprintln!("[WebSearch] 搜尋失敗: {}", e); None }
            }
        } else { None }
    } else { None };

    let engine = RagEngine::new(
        state.db.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );
    engine.generate_answer(
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
    ).await
}

/// Streaming 版本：每個 token 透過 Tauri event 推送到前端
/// event name: "rag-stream-token"      payload: { conversationId, token }
/// event name: "rag-stream-reasoning"  payload: { conversationId, token }
/// 完成後發 "rag-stream-done"          payload: { conversationId, fullAnswer, reasoning, ... }
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

    // 聯網搜尋：若啟用則先呼叫 Tavily，取得 web context 及來源清單
    let (web_ctx_text, web_sources) = if web_enabled.unwrap_or(false) {
        let settings = crate::settings::store::get_settings(&state.db).await.map_err(|e| e.to_string())?;
        let ws = &settings.web_search;
        if ws.enabled && !ws.api_key.is_empty() {
            match tavily_search(&ws.api_key, &query).await {
                Ok((ctx, srcs)) => (Some(ctx), srcs),
                Err(e) => { eprintln!("[WebSearch] 搜尋失敗: {}", e); (None, vec![]) }
            }
        } else { (None, vec![]) }
    } else { (None, vec![]) };

    let engine = RagEngine::new(
        state.db.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );

    let (base_prompt, history_vec, mut citation_sources, context_hints) = engine.build_prompt(
        &query,
        history.unwrap_or_default(),
        conversation_summary,
        project_id,
        source_ids,
        tag_filter,
        rag_enabled.unwrap_or(true),
        temp_chunk_ids,
        None,
    ).await?;

    // 將網路搜尋來源加入 citation_sources
    for src in &web_sources {
        if !citation_sources.iter().any(|s| s == src) {
            citation_sources.push(src.clone());
        }
    }

    let settings = crate::settings::store::get_settings(&state.db).await.map_err(|e| e.to_string())?;
    let cfg = settings.ai_models.chat_llm;
    let is_ollama = cfg.provider == "ollama";
    let api_key = cfg.api_key.clone().unwrap_or_default();

    if api_key.is_empty() && !is_ollama {
        return Err("LLM 未設定".to_string());
    }

    // 偵測模型原生推理能力，決定是否需要 prompt 注入
    let reasoning_style = model_caps::detect(&cfg.model, &cfg.provider);
    let has_native_reasoning = reasoning_style != model_caps::ReasoningStyle::None;

    // 若有聯網搜尋結果，附加到 base_prompt 後
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

    // 只有非原生推理模型才注入 THINK_MODE_PREFIX
    let system_prompt = if is_think && !has_native_reasoning {
        format!("{}{}", crate::prompts::THINK_MODE_PREFIX, base_prompt_with_web)
    } else {
        base_prompt_with_web
    };

    let llm = OpenAiProvider::new(api_key, cfg.base_url.clone(), cfg.model.clone(), cfg.provider.clone());
    let app_clone = app.clone();
    let conv_id = conversation_id.clone();

    let llm_opts = if is_think {
        LLMOptions { temperature: 0.6, max_tokens: 8192, stream: false, think_mode: Some(true) }
    } else {
        LLMOptions { think_mode: Some(false), ..LLMOptions::default() }
    };

    let stream_result = llm.complete_stream(
        &system_prompt,
        &history_vec,
        &query,
        llm_opts,
        move |token| {
            match &token {
                StreamToken::Reasoning(r) => {
                    let _ = app_clone.emit("rag-stream-reasoning", serde_json::json!({
                        "conversationId": conv_id,
                        "token": r,
                    }));
                }
                StreamToken::Content(c) => {
                    let _ = app_clone.emit("rag-stream-token", serde_json::json!({
                        "conversationId": conv_id,
                        "token": c,
                    }));
                }
            }
        },
    ).await.map_err(|e| e.to_string())?;

    let _ = app.emit("rag-stream-done", serde_json::json!({
        "conversationId": conversation_id,
        "fullAnswer": stream_result.content,
        "reasoning": if stream_result.reasoning.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(stream_result.reasoning) },
        "citationSources": citation_sources,
        "contextHints": context_hints,
    }));

    Ok(())
}
