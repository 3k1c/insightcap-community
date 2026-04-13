use sqlx::Row;
use tauri::State;
use crate::db::AppState;
use crate::services::memory_engine::MemoryEngine;
use chrono;

/// 手動更新 memory_chunk 的 tags / space_id
#[tauri::command]
pub async fn update_memory_chunk(
    state: State<'_, AppState>,
    chunk_id: String,
    tags: Option<String>,
    space_id: Option<String>,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut sets = vec!["updated_at = ?".to_string()];
    let mut binds: Vec<String> = vec![now];

    if let Some(ref t) = tags {
        sets.push("tags = ?".to_string());
        binds.push(t.clone());
    }
    if let Some(ref s) = space_id {
        sets.push("space_id = ?".to_string());
        binds.push(s.clone());
    }

    binds.push(chunk_id);
    let sql = format!("UPDATE memory_chunks SET {} WHERE id = ?", sets.join(", "));
    let mut query = sqlx::query(&sql);
    for b in &binds {
        query = query.bind(b);
    }
    query.execute(&state.db).await.map_err(|e| e.to_string())?;
    Ok(())
}

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

/// 批量確認或拒絕 pending_confirm 的 memory_chunks
#[tauri::command]
pub async fn batch_confirm_memory_chunks(
    state: State<'_, AppState>,
    chunk_ids: Vec<String>,
    accept: bool,
) -> Result<(), String> {
    if chunk_ids.is_empty() {
        return Ok(());
    }
    let engine = MemoryEngine::new(
        state.db.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );
    for id in &chunk_ids {
        engine.confirm_memory_chunk(id, accept).await?;
    }
    Ok(())
}

/// 清理超過 N 天未處理的 pending_confirm chunks（自動移除）
#[tauri::command]
pub async fn cleanup_expired_pending_chunks(
    state: State<'_, AppState>,
    days: i64,
) -> Result<u64, String> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    let result = sqlx::query(
        "DELETE FROM memory_chunks WHERE pending_confirm = 1 AND created_at < ?"
    )
    .bind(&cutoff)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(result.rows_affected())
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
