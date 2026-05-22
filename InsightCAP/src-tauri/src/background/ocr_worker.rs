use crate::db::AppState;
use crate::ocr::postprocess::{detect_language, postprocess_ocr_text};
use crate::ocr::preprocess::preprocess_for_ocr;
use crate::processing_tasks::ProcessingTaskState;
use sqlx::{Row, SqlitePool};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrTaskProgressPayload {
    task_id: Option<String>,
    file_path: String,
    file_name: String,
    stage: &'static str,
    status: &'static str,
    current: usize,
    total: usize,
    message: String,
}

fn emit_ocr_task_progress(
    app: &AppHandle,
    capture_id: &str,
    stage: &'static str,
    status: &'static str,
    current: usize,
    total: usize,
    message: impl Into<String>,
) {
    let short_id = &capture_id[..8.min(capture_id.len())];
    let _ = app.emit(
        "processing-task-progress",
        OcrTaskProgressPayload {
            task_id: Some(format!("ocr-{capture_id}")),
            file_path: capture_id.to_string(),
            file_name: format!("OCR image {short_id}"),
            stage,
            status,
            current,
            total,
            message: message.into(),
        },
    );
}

async fn cancel_ocr_if_requested(
    pool: &SqlitePool,
    app: &AppHandle,
    tasks: &ProcessingTaskState,
    capture_id: &str,
    current: usize,
    total: usize,
) -> bool {
    let task_id = format!("ocr-{capture_id}");
    if !tasks.is_cancelled(&task_id) {
        return false;
    }

    emit_ocr_task_progress(
        app,
        capture_id,
        "cancelled",
        "cancelled",
        current,
        total,
        "Cancelled",
    );
    tasks.clear(&task_id);
    let _ =
        sqlx::query("UPDATE captures SET status = 'processed', clean_content = '' WHERE id = ?")
            .bind(capture_id)
            .execute(pool)
            .await;
    true
}

pub fn start_ocr_worker(app: AppHandle) {
    let mut shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[OCR-WORKER] Stop signal received, exiting.");
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
                let processing_tasks = app.state::<ProcessingTaskState>();
                emit_ocr_task_progress(
                    &app,
                    &capture_id,
                    "parsing",
                    "processing",
                    0,
                    1,
                    "Running OCR",
                );
                if cancel_ocr_if_requested(&pool, &app, &processing_tasks, &capture_id, 0, 1).await
                {
                    continue;
                }

                if let Some(bytes) = img_bytes {
                    let raw_bytes = bytes.clone();

                    let ocr_input = match preprocess_for_ocr(&bytes) {
                        Ok(processed) => processed,
                        Err(e) => {
                            eprintln!("[OCR-WORKER] Preprocess failed, fallback to raw image: {e}");
                            bytes
                        }
                    };
                    if cancel_ocr_if_requested(&pool, &app, &processing_tasks, &capture_id, 0, 1)
                        .await
                    {
                        continue;
                    }

                    match crate::ocr::perform_ocr(&ocr_input).await {
                        Ok(raw_text) => {
                            if cancel_ocr_if_requested(
                                &pool,
                                &app,
                                &processing_tasks,
                                &capture_id,
                                0,
                                1,
                            )
                            .await
                            {
                                continue;
                            }
                            emit_ocr_task_progress(
                                &app,
                                &capture_id,
                                "cleaning",
                                "processing",
                                0,
                                1,
                                "Cleaning OCR text",
                            );
                            let language = detect_language(&raw_text);
                            let mut clean_text = postprocess_ocr_text(&raw_text, language);

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
                                        println!(
                                            "[OCR-WORKER] Vision enhancement applied: {capture_id}"
                                        );
                                        clean_text = vision_text;
                                    }
                                }
                            }
                            if cancel_ocr_if_requested(
                                &pool,
                                &app,
                                &processing_tasks,
                                &capture_id,
                                0,
                                1,
                            )
                            .await
                            {
                                continue;
                            }

                            let app_state = app.state::<AppState>();
                            emit_ocr_task_progress(
                                &app,
                                &capture_id,
                                "indexing",
                                "processing",
                                0,
                                1,
                                "Creating knowledge points and index",
                            );
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
                            if cancel_ocr_if_requested(
                                &pool,
                                &app,
                                &processing_tasks,
                                &capture_id,
                                0,
                                1,
                            )
                            .await
                            {
                                continue;
                            }

                            let _ = sqlx::query(
                                "UPDATE captures SET raw_content = ?, clean_content = ?, status = 'processed', vector_id = ? WHERE id = ?"
                            )
                            .bind(&raw_text)
                            .bind(&clean_text)
                            .bind(vector_id_opt)
                            .bind(&capture_id)
                            .execute(&pool)
                            .await;

                            let vs = app_state.vector_store.clone();
                            tokio::spawn(async move {
                                let _ = vs.save().await;
                            });

                            let tag_pool = pool.clone();
                            let tag_cid = capture_id.clone();
                            let tag_content = clean_text.clone();
                            tokio::spawn(async move {
                                let tag_engine =
                                    crate::services::tag_engine::TagEngine::new(tag_pool);
                                let _ =
                                    tag_engine.process_new_capture(&tag_cid, &tag_content).await;
                            });

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

                            println!("[OCR-WORKER] OCR completed: {capture_id}");
                            emit_ocr_task_progress(
                                &app,
                                &capture_id,
                                "completed",
                                "done",
                                1,
                                1,
                                "OCR completed",
                            );
                        }
                        Err(e) => {
                            eprintln!("[OCR-WORKER] OCR failed {capture_id}: {e}");
                            emit_ocr_task_progress(
                                &app,
                                &capture_id,
                                "failed",
                                "failed",
                                0,
                                1,
                                e.to_string(),
                            );
                            let _ = sqlx::query(
                                "UPDATE captures SET status = 'processed', clean_content = '' WHERE id = ?"
                            )
                            .bind(&capture_id)
                            .execute(&pool)
                            .await;
                        }
                    }
                } else {
                    emit_ocr_task_progress(
                        &app,
                        &capture_id,
                        "failed",
                        "failed",
                        0,
                        1,
                        "No image data",
                    );
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
