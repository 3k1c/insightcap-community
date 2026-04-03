use sqlx::{Row, SqlitePool};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;
use chrono::Utc;
use tauri::{AppHandle, Manager};

use crate::db::AppState;

/// 清理超過 7 天的 temp_attachment captures（對話結束後不再需要）
async fn cleanup_temp_attachments(pool: &SqlitePool) {
    let cutoff = (Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    match sqlx::query(
        "DELETE FROM captures WHERE capture_method = 'temp_attachment' AND created_at < ?",
    )
    .bind(&cutoff)
    .execute(pool)
    .await
    {
        Ok(r) => {
            if r.rows_affected() > 0 {
                println!("[CaptureProcessor] 清除 {} 筆過期 temp_attachment", r.rows_affected());
            }
        }
        Err(e) => eprintln!("[CaptureProcessor] 清除 temp_attachment 失敗: {}", e),
    }
}

pub async fn start_capture_processor(
    pool: SqlitePool,
    app: AppHandle,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    println!("[CaptureProcessor] 啟動背景處理迴圈...");
    cleanup_temp_attachments(&pool).await;
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    println!("[CaptureProcessor] 收到停止訊號，退出。");
                    break;
                }
            }
            _ = sleep(Duration::from_secs(5)) => {
                if let Err(e) = process_next_inbox(&pool, &app).await {
                    eprintln!("[CaptureProcessor] 處理失敗: {}", e);
                }
            }
        }
    }
}

async fn process_next_inbox(pool: &SqlitePool, app: &AppHandle) -> Result<(), String> {
    // 1. 取得下一筆 pending 的擷取任務
    let row = match sqlx::query(
        "SELECT id, content, content_type, source_exe, source_url, window_title, image_data, captured_at FROM inbox WHERE status = 'pending' ORDER BY captured_at ASC LIMIT 1"
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
    let source_url: String = row.try_get("source_url").unwrap_or_default();
    let window_title: String = row.try_get("window_title").unwrap_or_default();
    let image_data: Option<Vec<u8>> = row.try_get("image_data").unwrap_or(None);
    let captured_at: String = row.get("captured_at");

    println!("[CaptureProcessor] 處理 inbox: {} (type: {})", id, content_type);

    // 2. 標記處理中
    sqlx::query("UPDATE inbox SET status = 'processing' WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // 3. 萃取與轉換流程
    let source_id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let settings = crate::settings::store::get_settings(pool).await.unwrap_or_default();

    // URL 類型：爬取網頁或影片字幕
    let (display_title, processed_content) = if content_type == "url" && !source_url.is_empty() {
        let sessdata = settings.bilibili_sessdata.clone();
        match crate::capture::file_parser::parse_content(
            &settings.knowledge.kb_path,
            None,
            Some(source_url.clone()),
            sessdata,
        ).await {
            Ok(doc) => {
                let combined = doc.chunks.iter()
                    .map(|c| c.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let title = if doc.title.is_empty() { source_url.clone() } else { doc.title };
                (title, combined)
            }
            Err(e) => {
                eprintln!("[CaptureProcessor] URL 擷取失敗 {}: {}", source_url, e);
                (source_url.clone(), content.clone())
            }
        }
    } else {
        let title = if !window_title.is_empty() {
            window_title.clone()
        } else if !source_exe.is_empty() {
            source_exe.clone()
        } else {
            "無標題擷取".to_string()
        };
        (title, content.clone())
    };

    // 語言標準化（繁簡轉換）
    let normalized_content = crate::services::language_normalizer::NORMALIZER
        .normalize(&processed_content, &settings.general);

    // 寫入 sources
    sqlx::query(
        "INSERT INTO sources (id, type, title, url, clean_content, captured_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&source_id)
    .bind(if content_type == "url" { "url" } else { "clipboard" })
    .bind(&display_title)
    .bind(&source_url)
    .bind(&normalized_content)
    .bind(&captured_at)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 寫入 captures (按段落切分，與 create_temp_chunk 一致)
    // 若為 image，儲存 bytes，之後 OCR worker 會處理
    let final_status = if content_type == "image" { "pending_ocr" } else { "processed" };
    
    let mut chunk_count: i64 = 0;
    
    if content_type == "image" {
        // image 不做段落切分，直接寫一筆
        let capture_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO captures (id, source_id, type, raw_content, clean_content, image_data, capture_method, chunk_index, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&capture_id)
        .bind(&source_id)
        .bind(&content_type)
        .bind(&processed_content)
        .bind(&normalized_content)
        .bind(&image_data)
        .bind("hotkey")
        .bind(0i64)
        .bind(final_status)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        chunk_count = 1;
    } else {
        // 文字類型：按段落切分（與 create_temp_chunk 一致）
        let paragraphs: Vec<String> = normalized_content
            .split("\n\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let paragraphs = if paragraphs.is_empty() {
            if normalized_content.trim().is_empty() {
                vec![]
            } else {
                vec![normalized_content.clone()]
            }
        } else {
            paragraphs
        };

        let app_state = app.state::<AppState>();
        
        for (idx, para) in paragraphs.iter().enumerate() {
            let capture_id = Uuid::now_v7().to_string();
            
            sqlx::query(
                "INSERT INTO captures (id, source_id, type, raw_content, clean_content, capture_method, chunk_index, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&capture_id)
            .bind(&source_id)
            .bind(&content_type)
            .bind(para)
            .bind(para)
            .bind("hotkey")
            .bind(idx as i64)
            .bind(final_status)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            
            // Embedding 向量化並寫入 VectorStore
            match app_state.embedder.embed(para).await {
                Ok(vector) => {
                    let vector_id = capture_id_to_u64(&capture_id);
                    if let Err(e) = app_state.vector_store.add_vector(vector_id, &vector).await {
                        eprintln!("[CaptureProcessor] 向量寫入失敗: {}", e);
                    } else {
                        let _ = sqlx::query("UPDATE captures SET vector_id = ? WHERE id = ?")
                            .bind(vector_id as i64)
                            .bind(&capture_id)
                            .execute(pool)
                            .await;
                    }
                }
                Err(e) => eprintln!("[CaptureProcessor] Embedding 失敗 (chunk {}): {}", idx, e),
            }
            
            // Tagger：提取標籤並寫入 captures.tags + tags 表
            let tag_pool = pool.clone();
            let tag_cid = capture_id.clone();
            let tag_content = para.to_string();
            tokio::spawn(async move {
                let tag_engine = crate::services::tag_engine::TagEngine::new(tag_pool);
                if let Err(e) = tag_engine.process_new_capture(&tag_cid, &tag_content).await {
                    eprintln!("[CaptureProcessor] Tag generation failed for {}: {}", &tag_cid[..8.min(tag_cid.len())], e);
                }
            });

            // Space 聚類
            let space_engine = crate::services::space_engine::SpaceEngine::new(pool.clone());
            let _ = space_engine.assign_to_space(&capture_id, para).await;
            
            chunk_count += 1;
        }
        
        // 非同步儲存向量索引到磁碟
        let vs = app_state.vector_store.clone();
        tokio::spawn(async move { let _ = vs.save().await; });
    }

    // 4. 更新 inbox 狀態為完成
    sqlx::query("UPDATE inbox SET status = 'processed' WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // 更新 sources.capture_count
    sqlx::query("UPDATE sources SET capture_count = ?, updated_at = ? WHERE id = ?")
        .bind(chunk_count)
        .bind(&now)
        .bind(&source_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    println!("[CaptureProcessor] inbox {} 處理完成，source {} 產生 {} 個 chunk", id, source_id, chunk_count);
    Ok(())
}

/// 將 capture UUID 字串轉為 u64 作為向量索引 ID
fn capture_id_to_u64(id: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}
