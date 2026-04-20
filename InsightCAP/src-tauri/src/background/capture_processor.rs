use chrono::Utc;
use sqlx::{Row, SqlitePool};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::sleep;
use uuid::Uuid;

use crate::db::AppState;
use crate::tray_status::{set_tray_status, TrayStatus};

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
                println!(
                    "[CaptureProcessor] 清除 {} 筆過期 temp_attachment",
                    r.rows_affected()
                );
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
                match process_next_inbox(&pool, &app).await {
                    Ok(true) => {
                        // 有任務完成 → Done，3 秒後自動回 Idle
                        set_tray_status(&app, TrayStatus::Done);
                    }
                    Ok(false) => {
                        // 無任務，保持 Idle
                    }
                    Err(e) => {
                        eprintln!("[CaptureProcessor] 處理失敗: {}", e);
                        set_tray_status(&app, TrayStatus::Error);
                    }
                }
            }
        }
    }
}

/// 回傳 Ok(true) 表示處理了一筆，Ok(false) 表示 inbox 為空
async fn process_next_inbox(pool: &SqlitePool, app: &AppHandle) -> Result<bool, String> {
    // 1. 取得下一筆 pending 的擷取任務
    let row = match sqlx::query(
        "SELECT id, content, content_type, source_exe, source_url, window_title, image_data, captured_at FROM inbox WHERE status = 'pending' ORDER BY captured_at ASC LIMIT 1"
    )
    .fetch_optional(pool)
    .await {
        Ok(Some(r)) => r,
        Ok(None) => return Ok(false), // 無任務
        Err(e) => return Err(format!("查詢 inbox 失敗: {}", e)),
    };

    // 開始處理 → 托盤改為黃色
    set_tray_status(app, TrayStatus::Processing);

    let id: String = row.get("id");
    let content: String = row.get("content");
    let content_type: String = row.get("content_type");
    let source_exe: String = row.try_get("source_exe").unwrap_or_default();
    let source_url: String = row.try_get("source_url").unwrap_or_default();
    let window_title: String = row.try_get("window_title").unwrap_or_default();
    let image_data: Option<Vec<u8>> = row.try_get("image_data").unwrap_or(None);
    let captured_at: String = row.get("captured_at");

    println!(
        "[CaptureProcessor] 處理 inbox: {} (type: {})",
        id, content_type
    );

    // 2. 標記處理中
    sqlx::query("UPDATE inbox SET status = 'processing' WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // 3. 萃取與轉換流程
    let now = Utc::now().to_rfc3339();
    let settings = crate::settings::store::get_settings(pool)
        .await
        .unwrap_or_default();

    // 建構 VisionConfig
    let vision_config = crate::providers::llm::vision::VisionConfig::from_settings(
        &settings.ai_models.vision_model,
    );

    // URL 類型：爬取網頁或影片字幕
    let (display_title, processed_content) = if content_type == "url" && !source_url.is_empty() {
        let sessdata = settings.bilibili_sessdata.clone();
        match crate::capture::file_parser::parse_content(
            &settings.knowledge.kb_path,
            None,
            Some(source_url.clone()),
            sessdata,
            vision_config.as_ref(),
        )
        .await
        {
            Ok(doc) => {
                let combined = doc
                    .chunks
                    .iter()
                    .map(|c| c.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let title = if doc.title.is_empty() {
                    source_url.clone()
                } else {
                    doc.title
                };
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
        (
            crate::utils::title_cleaner::clean_window_title(&title),
            content.clone(),
        )
    };

    // 語言標準化（繁簡轉換）
    let normalized_content = crate::services::language_normalizer::NORMALIZER
        .normalize(&processed_content, &settings.general);
    let source_identity = if content_type == "url" && !source_url.is_empty() {
        crate::capture::source_group::identity_for_url(&source_url, &normalized_content)
    } else {
        crate::capture::source_group::identity_for_clipboard(&display_title, &normalized_content)
    };
    let source_group_id = crate::capture::source_group::get_or_create_source_group(
        pool,
        &source_identity,
        &display_title,
    )
    .await?;

    // 嘗試找到現有同來源的 source，避免重複建立
    let source_type = if content_type == "url" {
        "url"
    } else {
        "clipboard"
    };
    let existing_source_id: Option<String> = if content_type == "url" && !source_url.is_empty() {
        // URL 類型：完全相同的 URL 就視為同一 source
        sqlx::query_scalar("SELECT id FROM sources WHERE source_group_id = ? AND type = 'url' ORDER BY updated_at DESC LIMIT 1")
            .bind(&source_group_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None)
    } else {
        // 剪貼簿/熱鍵：同標題 + 30 分鐘內視為同一 source
        sqlx::query_scalar(
            "SELECT id FROM sources WHERE title = ? AND type = 'clipboard' \
             AND datetime(captured_at) > datetime('now', '-30 minutes') LIMIT 1",
        )
        .bind(&display_title)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
    };

    let source_id = if let Some(existing_id) = existing_source_id {
        // 重用現有 source，更新 updated_at
        sqlx::query("UPDATE sources SET updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&existing_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        println!("[CaptureProcessor] 重用既有 source: {}", existing_id);
        existing_id
    } else {
        // 建立新 source
        let new_source_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO sources (id, source_group_id, type, title, url, clean_content, content_hash, captured_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&new_source_id)
        .bind(&source_group_id)
        .bind(source_type)
        .bind(&display_title)
        .bind(&source_url)
        .bind(&normalized_content)
        .bind(&source_identity.content_hash)
        .bind(&captured_at)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        new_source_id
    };

    // 寫入 captures (按段落切分，與 create_temp_chunk 一致)
    // 若為 image，儲存 bytes，之後 OCR worker 會處理
    let final_status = if content_type == "image" {
        "pending_ocr"
    } else {
        "processed"
    };

    let mut chunk_count: i64 = 0;

    if content_type == "image" {
        // image 不做段落切分，直接寫一筆
        let capture_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO captures (id, source_id, type, raw_content, clean_content, image_data, capture_method, chunk_index, status, content_type, knowledge_type, chunk_strategy, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'image_ocr', 'data', 'semantic', ?, ?)"
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
        let routed_chunks =
            crate::capture::chunking::chunks_for_text(&normalized_content, &content_type);

        let app_state = app.state::<AppState>();

        for (idx, routed) in routed_chunks.iter().enumerate() {
            let capture_id = Uuid::now_v7().to_string();
            let para = &routed.content;

            sqlx::query(
                "INSERT INTO captures (id, source_id, type, raw_content, clean_content, capture_method, chunk_index, status, content_type, knowledge_type, chunk_strategy, chunk_metadata, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&capture_id)
            .bind(&source_id)
            .bind(&content_type)
            .bind(para)
            .bind(para)
            .bind("hotkey")
            .bind(idx as i64)
            .bind(final_status)
            .bind(&routed.content_type)
            .bind(&routed.knowledge_type)
            .bind(&routed.chunk_strategy)
            .bind(&routed.metadata_json)
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

            // 標籤改為 Source 層級統一生成，此處不再逐 chunk 呼叫 TagEngine

            // Space 聚類
            let space_engine = crate::services::space_engine::SpaceEngine::new(
                pool.clone(),
                app_state.embedder.clone(),
                app_state.vector_store.clone(),
            );
            if let Ok(Some((_space_id, is_new))) =
                space_engine.assign_to_space(&capture_id, para).await
            {
                if is_new {
                    let _ = app.emit("space-created", ());
                }
            }

            // 反向鏈接分析（非同步，不阻塞）
            let rel_pool = pool.clone();
            let rel_embedder = app_state.embedder.clone();
            let rel_vs = app_state.vector_store.clone();
            let rel_cid = capture_id.clone();
            let rel_content = para.to_string();
            tokio::spawn(async move {
                let rel_engine = crate::services::chunk_relation_engine::ChunkRelationEngine::new(
                    rel_pool,
                    rel_embedder,
                    rel_vs,
                );
                if let Err(e) = rel_engine
                    .analyze_and_link(&rel_cid, "capture", &rel_content)
                    .await
                {
                    eprintln!("[ChunkRelation] capture 分析失敗: {}", e);
                }
            });

            chunk_count += 1;
        }

        // Source 層級標籤生成（非同步，用整份文件內容，只呼叫一次）
        let tag_pool = pool.clone();
        let tag_source_id = source_id.clone();
        let tag_full_content = normalized_content.clone();
        tokio::spawn(async move {
            let tag_engine = crate::services::tag_engine::TagEngine::new(tag_pool);
            if let Err(e) = tag_engine
                .process_source(&tag_source_id, &tag_full_content)
                .await
            {
                eprintln!(
                    "[CaptureProcessor] Source tag generation failed for {}: {}",
                    &tag_source_id[..8.min(tag_source_id.len())],
                    e
                );
            }
        });

        // 非同步儲存向量索引到磁碟
        let vs = app_state.vector_store.clone();
        tokio::spawn(async move {
            let _ = vs.save().await;
        });
    }

    // 4. 更新 inbox 狀態為完成
    sqlx::query("UPDATE inbox SET status = 'processed' WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // 更新 sources.capture_count（累加而非覆寫，避免重用 source 時丟失舊計數）
    sqlx::query(
        "UPDATE sources SET capture_count = capture_count + ?, updated_at = ? WHERE id = ?",
    )
    .bind(chunk_count)
    .bind(&now)
    .bind(&source_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    println!(
        "[CaptureProcessor] inbox {} 處理完成，source {} 產生 {} 個 chunk",
        id, source_id, chunk_count
    );
    Ok(true)
}

/// 將 capture UUID 字串轉為 u64 作為向量索引 ID
fn capture_id_to_u64(id: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}
