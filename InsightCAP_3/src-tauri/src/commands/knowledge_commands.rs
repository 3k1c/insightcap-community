use sqlx::{Row, SqlitePool};
use tauri::State;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceItem {
    pub id: String,
    pub title: String,
    pub r#type: String,
    pub url: Option<String>,
    pub file_path: Option<String>,
    pub captured_at: String,
    pub content_preview: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureItem {
    pub id: String,
    pub source_id: Option<String>,
    pub clean_content: String,
    pub r#type: String,
    pub status: String,
    pub created_at: String,
}

#[tauri::command]
pub async fn get_sources(
    pool: State<'_, SqlitePool>,
    space_id: Option<String>,
    limit: Option<i64>
) -> Result<Vec<SourceItem>, String> {
    let max = limit.unwrap_or(50);
    
    let rows = if let Some(sid) = space_id {
        sqlx::query(
            "SELECT DISTINCT s.id, s.title, s.type, s.url, s.file_path, s.captured_at, SUBSTR(s.clean_content, 1, 120) as preview 
             FROM sources s 
             JOIN captures c ON s.id = c.source_id 
             WHERE c.space_id = ? 
             ORDER BY s.captured_at DESC LIMIT ?"
        )
        .bind(sid)
        .bind(max)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query(
            "SELECT id, title, type, url, file_path, captured_at, SUBSTR(clean_content, 1, 120) as preview 
             FROM sources 
             ORDER BY captured_at DESC LIMIT ?"
        )
        .bind(max)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?
    };

    let sources = rows.into_iter().map(|r| SourceItem {
        id: r.try_get("id").unwrap_or_default(),
        title: r.get("title"),
        r#type: r.get("type"),
        url: r.try_get("url").unwrap_or(None),
        file_path: r.try_get("file_path").unwrap_or(None),
        captured_at: r.get("captured_at"),
        content_preview: r.try_get("preview").unwrap_or_default(),
    }).collect();

    Ok(sources)
}

#[tauri::command]
pub async fn get_captures(
    pool: State<'_, SqlitePool>,
    status: Option<String>,
    source_id: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<CaptureItem>, String> {
    let max = limit.unwrap_or(100);
    
    let rows = if let Some(sid) = source_id {
        let s = status.unwrap_or_else(|| "processed".to_string());
        sqlx::query(
            "SELECT id, source_id, type, clean_content, status, created_at FROM captures WHERE status = ? AND source_id = ? ORDER BY created_at DESC LIMIT ?"
        )
        .bind(s)
        .bind(sid)
        .bind(max)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?
    } else {
        let s = status.unwrap_or_else(|| "inbox".to_string());
        sqlx::query(
            "SELECT id, source_id, type, clean_content, status, created_at FROM captures WHERE status = ? ORDER BY created_at DESC LIMIT ?"
        )
        .bind(s)
        .bind(max)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?
    };

    let captures = rows.into_iter().map(|r| CaptureItem {
        id: r.try_get("id").unwrap_or_default(),
        source_id: r.try_get("source_id").unwrap_or(None),
        clean_content: r.get("clean_content"),
        r#type: r.get("type"),
        status: r.try_get("status").unwrap_or_default(),
        created_at: r.get("created_at"),
    }).collect();

    Ok(captures)
}

#[tauri::command]
pub async fn get_pending_patterns(pool: State<'_, SqlitePool>) -> Result<Vec<CaptureItem>, String> {
    let rows = sqlx::query(
        "SELECT id, source_id, type, clean_content, status, created_at FROM captures WHERE status = 'pending_confirm' AND capture_method = 'pattern_promotion' ORDER BY created_at DESC"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let captures = rows.into_iter().map(|r| CaptureItem {
        id: r.try_get("id").unwrap_or_default(),
        source_id: r.try_get("source_id").unwrap_or(None),
        clean_content: r.get("clean_content"),
        r#type: r.get("type"),
        status: r.try_get("status").unwrap_or_default(),
        created_at: r.get("created_at"),
    }).collect();

    Ok(captures)
}

#[tauri::command]
pub async fn confirm_pattern(pool: State<'_, SqlitePool>, capture_id: String, accept: bool) -> Result<(), String> {
    if accept {
        sqlx::query("UPDATE captures SET status = 'processed' WHERE id = ?")
            .bind(&capture_id)
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    } else {
        // 如果使用者拒絕升格，則直接刪除該 capture 記錄
        sqlx::query("DELETE FROM captures WHERE id = ?")
            .bind(&capture_id)
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Quick Capture：前端手動輸入框送出後寫入 inbox
#[tauri::command]
pub async fn quick_capture(
    pool: State<'_, SqlitePool>,
    content: String,
) -> Result<(), String> {
    let id = uuid::Uuid::now_v7().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    
    // 偵測是否為 URL
    let trimmed = content.trim();
    let content_type = if (trimmed.starts_with("http://") || trimmed.starts_with("https://")) 
        && !trimmed.contains(' ') {
        "url"
    } else {
        "text"
    };

    sqlx::query(
        "INSERT INTO inbox (id, content, content_type, source_exe, window_title, session_id, status, captured_at) VALUES (?, ?, ?, 'QuickCapture', '快速擷取', '', 'pending', ?)"
    )
    .bind(&id)
    .bind(trimmed)
    .bind(content_type)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("quick_capture DB error: {}", e))?;

    println!("[QuickCapture] Saved to inbox: {} ({})", &id[..8], content_type);
    Ok(())
}
