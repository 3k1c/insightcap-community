use sqlx::Row;
use tauri::State;
use crate::db::AppState;
use crate::services::memory_engine::MemoryEngine;

/// 用戶確認或拒絕 pending_confirm 的 memory_chunk
#[tauri::command]
pub async fn confirm_memory_chunk(
    state: State<'_, AppState>,
    chunk_id: String,
    accept: bool,
) -> Result<(), String> {
    let engine = MemoryEngine::new(
        state.db.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );
    engine.confirm_memory_chunk(&chunk_id, accept).await
}

/// 取得所有 pending_confirm = 1 的 memory_chunks（供前端顯示確認 toast）
#[tauri::command]
pub async fn get_pending_memory_chunks(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query(
        "SELECT id, knowledge_type, content, tags, confidence, created_at \
         FROM memory_chunks WHERE pending_confirm = 1 ORDER BY created_at DESC LIMIT 20"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let chunks = rows.into_iter().map(|r| {
        serde_json::json!({
            "id": r.try_get::<String, _>("id").unwrap_or_default(),
            "knowledgeType": r.try_get::<String, _>("knowledge_type").unwrap_or_default(),
            "content": r.try_get::<String, _>("content").unwrap_or_default(),
            "tags": r.try_get::<String, _>("tags").unwrap_or_else(|_| "[]".to_string()),
            "confidence": r.try_get::<f64, _>("confidence").unwrap_or(0.0),
            "createdAt": r.try_get::<String, _>("created_at").unwrap_or_default(),
        })
    }).collect();

    Ok(chunks)
}

/// 取得升格候選的 memory_chunks（pattern_promotion 產出，status = pending_confirm）
/// 對應舊版 get_pending_patterns，現在正確操作 memory_chunks
#[tauri::command]
pub async fn get_pending_patterns(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query(
        "SELECT id, knowledge_type, content, tags, confidence, created_at \
         FROM memory_chunks \
         WHERE pending_confirm = 1 AND knowledge_type = 'pattern' \
         ORDER BY created_at DESC LIMIT 20"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let chunks = rows.into_iter().map(|r| {
        serde_json::json!({
            "id": r.try_get::<String, _>("id").unwrap_or_default(),
            "knowledgeType": r.try_get::<String, _>("knowledge_type").unwrap_or_default(),
            "content": r.try_get::<String, _>("content").unwrap_or_default(),
            "tags": r.try_get::<String, _>("tags").unwrap_or_else(|_| "[]".to_string()),
            "confidence": r.try_get::<f64, _>("confidence").unwrap_or(0.0),
            "createdAt": r.try_get::<String, _>("created_at").unwrap_or_default(),
        })
    }).collect();

    Ok(chunks)
}

/// 用戶確認或拒絕 pattern 升格
/// 對應舊版 confirm_pattern，現在正確操作 memory_chunks
#[tauri::command]
pub async fn confirm_pattern(
    state: State<'_, AppState>,
    capture_id: String, // 參數名保持相容，實際為 memory_chunk id
    accept: bool,
) -> Result<(), String> {
    let engine = MemoryEngine::new(
        state.db.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );
    engine.confirm_memory_chunk(&capture_id, accept).await
}
