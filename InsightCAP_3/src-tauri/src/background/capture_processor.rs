use sqlx::{Row, SqlitePool};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;
use chrono::Utc;

pub async fn start_capture_processor(pool: SqlitePool) {
    println!("[CaptureProcessor] 啟動背景處理迴圈...");
    loop {
        if let Err(e) = process_next_inbox(&pool).await {
            eprintln!("[CaptureProcessor] 處理失敗: {}", e);
        }
        sleep(Duration::from_secs(5)).await;
    }
}

async fn process_next_inbox(pool: &SqlitePool) -> Result<(), String> {
    // 1. 取得下一筆 pending 的擷取任務
    let row = match sqlx::query(
        "SELECT id, content, content_type, source_exe, window_title, session_id, captured_at FROM inbox WHERE status = 'pending' ORDER BY captured_at ASC LIMIT 1"
    )
    .fetch_optional(pool)
    .await {
        Ok(Some(r)) => r,
        Ok(None) => return Ok(()), // 無任務
        Err(e) => return Err(format!("查詢 inbox 失敗: {}", e)),
    };

    let id: String = row.get("id");
    let content: String = row.get("content");
    let content_type: String = row.get("content_type");
    let source_exe: String = row.try_get("source_exe").unwrap_or_default();
    let source_url: String = if content_type == "url" { content.clone() } else { String::new() };
    let window_title: String = row.try_get("window_title").unwrap_or_default();
    let _session_id: String = row.try_get("session_id").unwrap_or_default();
    let captured_at: String = row.get("captured_at");

    println!("[CaptureProcessor] 處理 inbox: {} (type: {})", id, content_type);

    // 2. 標記處理中
    sqlx::query("UPDATE inbox SET status = 'processing' WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // 3. 虛擬的萃取與轉換流程（後續應呼叫 LLM 產生 title/tags、呼叫 Embedder 產生向量）
    let source_id = Uuid::now_v7().to_string();
    let capture_id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    // 在 v2 中，剪貼簿文字或網頁需要成為一個 Source
    let display_title = if !window_title.is_empty() {
        window_title.clone()
    } else if !source_exe.is_empty() {
        source_exe.clone()
    } else {
        "無標題擷取".to_string()
    };

    // 寫入 sources
    sqlx::query(
        "INSERT INTO sources (id, type, title, url, clean_content, captured_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&source_id)
    .bind(if content_type == "url" { "url" } else { "clipboard" })
    .bind(&display_title)
    .bind(&source_url)
    .bind(&content)
    .bind(&captured_at)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 寫入 captures (單一 chunk)
    sqlx::query(
        "INSERT INTO captures (id, source_id, type, raw_content, clean_content, capture_method, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&capture_id)
    .bind(&source_id)
    .bind(&content_type)
    .bind(&content)
    .bind(&content)
    .bind("hotkey")
    .bind("processed") // 因為我們跳過了 vectorize 暫時標記為 processed
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 4. 更新 inbox 狀態為完成
    sqlx::query("UPDATE inbox SET status = 'processed' WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // 5. 非同步背景執行標籤與聚類
    let tag_engine = crate::services::tag_engine::TagEngine::new(pool.clone());
    let _ = tag_engine.process_new_capture(&capture_id, &content).await;

    let space_engine = crate::services::space_engine::SpaceEngine::new(pool.clone());
    let _ = space_engine.assign_to_space(&capture_id, &content).await;

    println!("[CaptureProcessor] inbox {} 處理完成，已轉換為 source {} 和 capture {}", id, source_id, capture_id);
    Ok(())
}
