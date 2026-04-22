use crate::db::AppState;
use crate::services::memory_engine::MemoryEngine;
use chrono;
use sqlx::Row;
use tauri::State;

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

#[tauri::command]
pub async fn get_pending_memory_chunks(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query(
        "SELECT id, knowledge_type, content, tags, confidence, created_at \
         FROM memory_chunks \
         WHERE pending_confirm = 1 \
           AND content IS NOT NULL \
           AND TRIM(content) != '' \
         ORDER BY created_at DESC LIMIT 20",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let chunks = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.try_get::<String, _>("id").unwrap_or_default(),
                "knowledgeType": r.try_get::<String, _>("knowledge_type").unwrap_or_default(),
                "content": r.try_get::<String, _>("content").unwrap_or_default(),
                "tags": r.try_get::<String, _>("tags").unwrap_or_else(|_| "[]".to_string()),
                "confidence": r.try_get::<f64, _>("confidence").unwrap_or(0.0),
                "createdAt": r.try_get::<String, _>("created_at").unwrap_or_default(),
            })
        })
        .collect();

    Ok(chunks)
}

#[tauri::command]
pub async fn get_pending_patterns(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query(
        "SELECT id, knowledge_type, content, tags, confidence, created_at \
         FROM memory_chunks \
         WHERE pending_confirm = 1 \
           AND knowledge_type = 'pattern' \
           AND content IS NOT NULL \
           AND TRIM(content) != '' \
         ORDER BY created_at DESC LIMIT 20",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let chunks = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.try_get::<String, _>("id").unwrap_or_default(),
                "knowledgeType": r.try_get::<String, _>("knowledge_type").unwrap_or_default(),
                "content": r.try_get::<String, _>("content").unwrap_or_default(),
                "tags": r.try_get::<String, _>("tags").unwrap_or_else(|_| "[]".to_string()),
                "confidence": r.try_get::<f64, _>("confidence").unwrap_or(0.0),
                "createdAt": r.try_get::<String, _>("created_at").unwrap_or_default(),
            })
        })
        .collect();

    Ok(chunks)
}

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

#[tauri::command]
pub async fn cleanup_expired_pending_chunks(
    state: State<'_, AppState>,
    days: i64,
) -> Result<u64, String> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    let result =
        sqlx::query("DELETE FROM memory_chunks WHERE pending_confirm = 1 AND created_at < ?")
            .bind(&cutoff)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    Ok(result.rows_affected())
}

#[tauri::command]
pub async fn confirm_pattern(
    state: State<'_, AppState>,
    capture_id: String,
    accept: bool,
) -> Result<(), String> {
    let engine = MemoryEngine::new(
        state.db.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );
    engine.confirm_memory_chunk(&capture_id, accept).await
}
