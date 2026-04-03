use tauri::{AppHandle, Manager};
use std::time::Duration;
use sqlx::{SqlitePool, Row};
use crate::db::AppState;
use crate::ocr::postprocess::{detect_language, postprocess_ocr_text};
use crate::ocr::preprocess::preprocess_for_ocr;

pub fn start_ocr_worker(app: AppHandle) {
    let mut shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[OCR-WORKER] 收到停止訊號，退出。");
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {}
            }
            if *shutdown_rx.borrow() { break; }

            let pool = match app.try_state::<SqlitePool>() {
                Some(p) => p.inner().clone(),
                None => continue,
            };

            let rows = match sqlx::query(
                "SELECT id, image_data FROM captures WHERE status = 'pending_ocr' ORDER BY created_at ASC LIMIT 5"
            ).fetch_all(&pool).await {
                Ok(r) => r,
                Err(_) => continue,
            };

            if rows.is_empty() { continue; }

            for row in rows {
                let capture_id: String = row.get("id");
                let img_bytes: Option<Vec<u8>> = row.try_get("image_data").unwrap_or(None);

                if let Some(bytes) = img_bytes {
                    // 第二層：圖像前處理，失敗時靜默降級
                    let ocr_input = match preprocess_for_ocr(&bytes) {
                        Ok(preprocessed) => preprocessed,
                        Err(e) => {
                            eprintln!("[OCR-WORKER] 前處理失敗，使用原始圖像: {e}");
                            bytes
                        }
                    };

                    // 第一層：原生 OCR
                    match crate::ocr::perform_ocr(&ocr_input).await {
                        Ok(raw_text) => {
                            // 第三層：文字後處理
                            let language = detect_language(&raw_text);
                            let clean_text = postprocess_ocr_text(&raw_text, language);

                            let _ = sqlx::query(
                                "UPDATE captures SET raw_content = ?, clean_content = ?, status = 'processed' WHERE id = ?"
                            )
                            .bind(&raw_text)
                            .bind(&clean_text)
                            .bind(&capture_id)
                            .execute(&pool)
                            .await;

                            let tag_engine = crate::services::tag_engine::TagEngine::new(pool.clone());
                            let _ = tag_engine.process_new_capture(&capture_id, &clean_text).await;

                            println!("[OCR-WORKER] ✅ {capture_id}");
                        }
                        Err(e) => {
                            eprintln!("[OCR-WORKER] ❌ OCR 失敗 {capture_id}: {e}");
                            let _ = sqlx::query(
                                "UPDATE captures SET status = 'processed', clean_content = '' WHERE id = ?"
                            )
                            .bind(&capture_id)
                            .execute(&pool)
                            .await;
                        }
                    }
                } else {
                    let _ = sqlx::query(
                        "UPDATE captures SET status = 'processed', clean_content = '' WHERE id = ?"
                    )
                    .bind(&capture_id)
                    .execute(&pool)
                    .await;
                }
            }
        }
    });
}
