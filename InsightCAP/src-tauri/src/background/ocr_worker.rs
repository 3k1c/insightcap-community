use crate::db::AppState;
use crate::ocr::postprocess::{detect_language, postprocess_ocr_text};
use crate::ocr::preprocess::preprocess_for_ocr;
use sqlx::{Row, SqlitePool};
use std::time::Duration;
use tauri::{AppHandle, Manager};

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
            if *shutdown_rx.borrow() {
                break;
            }

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

            if rows.is_empty() {
                continue;
            }

            for row in rows {
                let capture_id: String = row.get("id");
                let img_bytes: Option<Vec<u8>> = row.try_get("image_data").unwrap_or(None);

                if let Some(bytes) = img_bytes {
                    // 保留原始圖像供 vision model（前處理 binarize 會破壞顏色資訊）
                    let raw_bytes = bytes.clone();

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
                            let mut clean_text = postprocess_ocr_text(&raw_text, language);

                            // Vision model 增強：用原始圖像嘗試 vision，成功則取代 OCR 結果
                            if let Ok(settings) = crate::settings::store::get_settings(&pool).await
                            {
                                if let Some(vc) =
                                    crate::providers::llm::vision::VisionConfig::from_settings(
                                        &settings.ai_models.vision_model,
                                    )
                                {
                                    if let Some(vision_text) =
                                        crate::providers::llm::vision::try_vision_enhance(
                                            &vc,
                                            &raw_bytes,
                                            crate::providers::llm::vision::general_vision_prompt(),
                                        )
                                        .await
                                    {
                                        println!("[OCR-WORKER] Vision 增強成功 {capture_id}");
                                        clean_text = vision_text;
                                    }
                                }
                            }

                            // Embedding 向量化並寫入 VectorStore
                            let app_state = app.state::<AppState>();
                            let vector_id_opt: Option<i64> =
                                match app_state.embedder.embed(&clean_text).await {
                                    Ok(vec) => {
                                        let vid = {
                                            use std::collections::hash_map::DefaultHasher;
                                            use std::hash::{Hash, Hasher};
                                            let mut h = DefaultHasher::new();
                                            capture_id.hash(&mut h);
                                            h.finish()
                                        };
                                        match app_state.vector_store.add_vector(vid, &vec).await {
                                            Ok(_) => Some(vid as i64),
                                            Err(e) => {
                                                eprintln!("[OCR-WORKER] vector store error: {}", e);
                                                None
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[OCR-WORKER] embed error: {}", e);
                                        None
                                    }
                                };

                            let _ = sqlx::query(
                                "UPDATE captures SET raw_content = ?, clean_content = ?, status = 'processed', vector_id = ? WHERE id = ?"
                            )
                            .bind(&raw_text)
                            .bind(&clean_text)
                            .bind(vector_id_opt)
                            .bind(&capture_id)
                            .execute(&pool)
                            .await;

                            // 非同步儲存向量索引
                            let vs = app_state.vector_store.clone();
                            tokio::spawn(async move {
                                let _ = vs.save().await;
                            });

                            // 標籤生成
                            let tag_pool = pool.clone();
                            let tag_cid = capture_id.clone();
                            let tag_content = clean_text.clone();
                            tokio::spawn(async move {
                                let tag_engine =
                                    crate::services::tag_engine::TagEngine::new(tag_pool);
                                let _ =
                                    tag_engine.process_new_capture(&tag_cid, &tag_content).await;
                            });

                            // Space 聚類
                            let sp_pool = pool.clone();
                            let sp_embedder = app_state.embedder.clone();
                            let sp_vs = app_state.vector_store.clone();
                            let sp_cid = capture_id.clone();
                            let sp_content = clean_text.clone();
                            tokio::spawn(async move {
                                let space_engine = crate::services::space_engine::SpaceEngine::new(
                                    sp_pool,
                                    sp_embedder,
                                    sp_vs,
                                );
                                let _ = space_engine.assign_to_space(&sp_cid, &sp_content).await;
                            });

                            // 反向鏈接分析
                            let rel_pool = pool.clone();
                            let rel_embedder = app_state.embedder.clone();
                            let rel_vs = app_state.vector_store.clone();
                            let rel_cid = capture_id.clone();
                            let rel_content = clean_text.clone();
                            tokio::spawn(async move {
                                let rel_engine = crate::services::chunk_relation_engine::ChunkRelationEngine::new(
                                    rel_pool, rel_embedder, rel_vs,
                                );
                                let _ = rel_engine
                                    .analyze_and_link(&rel_cid, "capture", &rel_content)
                                    .await;
                            });

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
                        "UPDATE captures SET status = 'processed', clean_content = '' WHERE id = ?",
                    )
                    .bind(&capture_id)
                    .execute(&pool)
                    .await;
                }
            }
        }
    });
}
