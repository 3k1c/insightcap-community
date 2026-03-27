use tauri::{AppHandle, Manager};
use std::time::Duration;
use sqlx::{SqlitePool, Row};
use crate::providers::llm::vision::{describe_image, extract_text_from_vision_output};

/// OCR 背景工作程式 
/// 負責抓取狀態為 pending_ocr 的截圖，送到 Vision 模型解析後推回 inbox
pub fn start_ocr_worker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;

            let pool = match app.try_state::<SqlitePool>() {
                Some(p) => p.inner().clone(),
                None => continue,
            };

            // 1. 取得需要 OCR 的 chunk
            let rows = match sqlx::query(
                "SELECT id, source_id, image_data FROM captures WHERE status = 'pending_ocr' LIMIT 5"
            ).fetch_all(&pool).await {
                Ok(r) => r,
                Err(_) => continue,
            };

            if rows.is_empty() {
                continue;
            }

            let settings = match crate::settings::store::get_settings(&pool).await {
                Ok(s) => s,
                Err(_) => continue,
            };
            let cfg = settings.ai_models.vision_model;
            let provider = cfg.provider;
            let model = cfg.model;
            let api_key = cfg.api_key.unwrap_or_default();
            let base_url = cfg.base_url;

            if api_key.is_empty() && (provider != "ollama") {
                continue;
            }

            // 3. 處理每一筆
            for row in rows {
                let capture_id: String = row.get("id");
                let img_bytes: Option<Vec<u8>> = row.try_get("image_data").unwrap_or(None);

                if let Some(bytes) = img_bytes {
                    // 呼叫 vision 模型
                    match describe_image(
                        &bytes,
                        &provider,
                        &model,
                        &api_key,
                        base_url.as_deref()
                    ).await {
                        Ok(raw_res) => {
                            let clean_text = extract_text_from_vision_output(&raw_res);
                            
                            // 更新資料庫並標記為 processed
                            let _ = sqlx::query(
                                "UPDATE captures SET raw_content = ?, clean_content = ?, status = 'processed' WHERE id = ?"
                            )
                            .bind(&raw_res)
                            .bind(&clean_text)
                            .bind(&capture_id)
                            .execute(&pool)
                            .await;
                            
                            // 觸發後續的 Engine
                            let tag_engine = crate::services::tag_engine::TagEngine::new(pool.clone());
                            let _ = tag_engine.process_new_capture(&capture_id, &clean_text).await;

                            let space_engine = crate::services::space_engine::SpaceEngine::new(pool.clone());
                            let _ = space_engine.assign_to_space(&capture_id, &clean_text).await;

                            println!("[OCR-WORKER] Processed {capture_id}");
                        }
                        Err(e) => {
                            eprintln!("[OCR-WORKER] Vision API error for {capture_id}: {e}");
                        }
                    }
                } else {
                    // 圖片不見了，直接把這個標記回去 inbox，內容設成空，讓他被 embedding 但是沒有文字
                    let _ = sqlx::query("UPDATE captures SET status = 'inbox' WHERE id = ?")
                        .bind(&capture_id)
                        .execute(&pool)
                        .await;
                }
            }
        }
    });
}
