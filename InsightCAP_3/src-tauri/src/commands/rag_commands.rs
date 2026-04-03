use tauri::{Emitter, State};
use crate::db::AppState;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::providers::llm::openai::OpenAiProvider;
use crate::services::rag_engine::RagEngine;

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
    _web_enabled: Option<bool>,
    temp_chunk_ids: Option<Vec<String>>,
    thinking_mode: Option<String>,
) -> Result<serde_json::Value, String> {
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
    ).await
}

/// Streaming 版本：每個 token 透過 Tauri event 推送到前端
/// event name: "rag-stream-token"  payload: { conversation_id, token }
/// 完成後發 "rag-stream-done"      payload: { conversation_id, full_answer }
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
    _web_enabled: Option<bool>,
    temp_chunk_ids: Option<Vec<String>>,
    thinking_mode: Option<String>,
) -> Result<(), String> {
    let is_think = thinking_mode.as_deref().unwrap_or("normal") == "think";

    let engine = RagEngine::new(
        state.db.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );

    let (base_prompt, history_vec, citation_sources) = engine.build_prompt(
        &query,
        history.unwrap_or_default(),
        conversation_summary,
        project_id,
        source_ids,
        tag_filter,
        rag_enabled.unwrap_or(true),
        temp_chunk_ids,
    ).await?;

    // Think 模式：在 system prompt 最前面注入深度推理指令
    let system_prompt = if is_think {
        format!("{}{}", crate::prompts::THINK_MODE_PREFIX, base_prompt)
    } else {
        base_prompt
    };

    let settings = crate::settings::store::get_settings(&state.db).await.map_err(|e| e.to_string())?;
    let cfg = settings.ai_models.chat_llm;
    let is_ollama = cfg.provider == "ollama";
    let api_key = cfg.api_key.clone().unwrap_or_default();

    if api_key.is_empty() && !is_ollama {
        return Err("LLM 未設定".to_string());
    }

    let llm = OpenAiProvider::new(api_key, cfg.base_url.clone(), cfg.model.clone());
    let app_clone = app.clone();
    let conv_id = conversation_id.clone();

    let llm_opts = if is_think {
        LLMOptions { temperature: 0.6, max_tokens: 8192, stream: false }
    } else {
        LLMOptions::default()
    };

    let full_answer = llm.complete_stream(
        &system_prompt,
        &history_vec,
        &query,
        llm_opts,
        move |token| {
            let _ = app_clone.emit("rag-stream-token", serde_json::json!({
                "conversationId": conv_id,
                "token": token,
            }));
        },
    ).await.map_err(|e| e.to_string())?;

    let _ = app.emit("rag-stream-done", serde_json::json!({
        "conversationId": conversation_id,
        "fullAnswer": full_answer,
        "citationSources": citation_sources,
    }));

    Ok(())
}
