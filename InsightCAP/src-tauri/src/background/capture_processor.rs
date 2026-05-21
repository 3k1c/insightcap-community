use chrono::Utc;
use sqlx::{Row, SqlitePool};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::sleep;
use uuid::Uuid;

use crate::db::AppState;
use crate::tray_status::{set_tray_status, TrayStatus};

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessingTaskProgressPayload {
    task_id: Option<String>,
    file_path: String,
    file_name: String,
    stage: &'static str,
    status: &'static str,
    current: usize,
    total: usize,
    message: String,
}

fn emit_processing_task_progress(
    app: &AppHandle,
    task_id: &str,
    file_path: &str,
    file_name: &str,
    stage: &'static str,
    status: &'static str,
    current: usize,
    total: usize,
    message: impl Into<String>,
) {
    let _ = app.emit(
        "processing-task-progress",
        ProcessingTaskProgressPayload {
            task_id: Some(task_id.to_string()),
            file_path: file_path.to_string(),
            file_name: file_name.to_string(),
            stage,
            status,
            current,
            total,
            message: message.into(),
        },
    );
}

fn extract_bvid(value: &str) -> Option<String> {
    let start = value.find("BV")?;
    let bvid: String = value[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .take(12)
        .collect();
    if bvid.len() == 12 {
        Some(bvid)
    } else {
        None
    }
}

fn youtube_thumbnail_url(url: &str) -> Option<String> {
    let id = if let Some(pos) = url.find("youtu.be/") {
        url[pos + 9..]
            .split(['?', '#', '&', '/', ' '])
            .next()
            .unwrap_or("")
    } else if let Some(pos) = url.find("watch?v=") {
        url[pos + 8..]
            .split(['?', '#', '&', '/', ' '])
            .next()
            .unwrap_or("")
    } else if let Some(pos) = url.find("/shorts/") {
        url[pos + 8..]
            .split(['?', '#', '&', '/', ' '])
            .next()
            .unwrap_or("")
    } else {
        ""
    };

    if id.len() >= 6 {
        Some(format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", id))
    } else {
        None
    }
}

async fn bilibili_thumbnail_url(url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .ok()?;

    let bvid = if let Some(bvid) = extract_bvid(url) {
        bvid
    } else if url.contains("b23.tv/") {
        let resolved = client.get(url).send().await.ok()?.url().to_string();
        extract_bvid(&resolved)?
    } else {
        return None;
    };

    let api_url = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={}",
        bvid
    );
    let json = client
        .get(api_url)
        .header(
            "Referer",
            format!("https://www.bilibili.com/video/{}", bvid),
        )
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;

    let pic = json["data"]["pic"].as_str()?.trim();
    if pic.is_empty() {
        None
    } else if pic.starts_with("//") {
        Some(format!("https:{}", pic))
    } else {
        Some(pic.to_string())
    }
}

async fn fetch_source_thumbnail(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    if lower.contains("youtube.com/") || lower.contains("youtu.be/") {
        return youtube_thumbnail_url(url);
    }
    if lower.contains("bilibili.com/video/") || lower.contains("b23.tv/") {
        return bilibili_thumbnail_url(url).await;
    }
    None
}

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
                    "[CaptureProcessor] Removed {} stale temp_attachment captures",
                    r.rows_affected()
                );
            }
        }
        Err(e) => eprintln!(
            "[CaptureProcessor] Failed to cleanup temp_attachment: {}",
            e
        ),
    }
}

pub async fn start_capture_processor(
    pool: SqlitePool,
    app: AppHandle,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    println!("[CaptureProcessor] Worker started...");
    cleanup_temp_attachments(&pool).await;
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    println!("[CaptureProcessor] Stop signal received, exiting.");
                    break;
                }
            }
            _ = sleep(Duration::from_secs(5)) => {
                match process_next_inbox(&pool, &app).await {
                    Ok(true) => {
                        set_tray_status(&app, TrayStatus::Done);
                    }
                    Ok(false) => {
                    }
                    Err(e) => {
                        eprintln!("[CaptureProcessor] Processing failed: {}", e);
                        set_tray_status(&app, TrayStatus::Error);
                    }
                }
            }
        }
    }
}

async fn process_next_inbox(pool: &SqlitePool, app: &AppHandle) -> Result<bool, String> {
    let row = match sqlx::query(
        "SELECT id, content, content_type, source_exe, source_url, window_title, image_data, captured_at FROM inbox WHERE status = 'pending' ORDER BY captured_at ASC LIMIT 1"
    )
    .fetch_optional(pool)
    .await {
        Ok(Some(r)) => r,
        Ok(None) => return Ok(false),
        Err(e) => return Err(format!("Failed to query inbox: {}", e)),
    };

    set_tray_status(app, TrayStatus::Processing);

    let id: String = row.get("id");
    let content: String = row.get("content");
    let content_type: String = row.get("content_type");
    let source_exe: String = row.try_get("source_exe").unwrap_or_default();
    let source_url: String = row.try_get("source_url").unwrap_or_default();
    let window_title: String = row.try_get("window_title").unwrap_or_default();
    let image_data: Option<Vec<u8>> = row.try_get("image_data").unwrap_or(None);
    let captured_at: String = row.get("captured_at");
    let task_file_path = if !source_url.is_empty() {
        source_url.clone()
    } else if !window_title.is_empty() {
        window_title.clone()
    } else if !source_exe.is_empty() {
        source_exe.clone()
    } else {
        id.clone()
    };
    let task_file_name = if !window_title.is_empty() {
        crate::utils::title_cleaner::clean_window_title(&window_title)
    } else if !source_url.is_empty() {
        source_url.clone()
    } else if !source_exe.is_empty() {
        source_exe.clone()
    } else {
        "Captured content".to_string()
    };

    println!(
        "[CaptureProcessor] Processing inbox: {} (type: {})",
        id, content_type
    );
    emit_processing_task_progress(
        app,
        &id,
        &task_file_path,
        &task_file_name,
        "parsing",
        "processing",
        0,
        0,
        "Parsing captured content",
    );

    sqlx::query("UPDATE inbox SET status = 'processing' WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    let settings = crate::settings::store::get_settings(pool)
        .await
        .unwrap_or_default();

    let vision_config = crate::providers::llm::vision::VisionConfig::from_settings(
        &settings.ai_models.vision_model,
    );

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
                eprintln!("[CaptureProcessor] URL parse failed {}: {}", source_url, e);
                (source_url.clone(), content.clone())
            }
        }
    } else {
        let title = if !window_title.is_empty() {
            window_title.clone()
        } else if !source_exe.is_empty() {
            source_exe.clone()
        } else {
            "Untitled Capture".to_string()
        };
        (
            crate::utils::title_cleaner::clean_window_title(&title),
            content.clone(),
        )
    };

    emit_processing_task_progress(
        app,
        &id,
        &task_file_path,
        &display_title,
        "cleaning",
        "processing",
        0,
        0,
        "Cleaning captured content",
    );

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

    let source_type = if content_type == "url" {
        "url"
    } else {
        "clipboard"
    };
    let thumbnail_url = if content_type == "url" && !source_url.is_empty() {
        fetch_source_thumbnail(&source_url).await
    } else {
        None
    };
    let existing_source_id: Option<String> = if content_type == "url" && !source_url.is_empty() {
        sqlx::query_scalar("SELECT id FROM sources WHERE source_group_id = ? AND type = 'url' ORDER BY updated_at DESC LIMIT 1")
            .bind(&source_group_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None)
    } else {
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
        if let Some(thumbnail) = thumbnail_url.as_deref() {
            sqlx::query(
                "UPDATE sources SET updated_at = ?, thumbnail = COALESCE(NULLIF(thumbnail, ''), ?) WHERE id = ?",
            )
            .bind(&now)
            .bind(thumbnail)
            .bind(&existing_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        } else {
            sqlx::query("UPDATE sources SET updated_at = ? WHERE id = ?")
                .bind(&now)
                .bind(&existing_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        println!("[CaptureProcessor] Reusing source: {}", existing_id);
        existing_id
    } else {
        let new_source_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO sources (id, source_group_id, type, title, url, thumbnail, clean_content, content_hash, captured_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&new_source_id)
        .bind(&source_group_id)
        .bind(source_type)
        .bind(&display_title)
        .bind(&source_url)
        .bind(thumbnail_url.as_deref())
        .bind(&normalized_content)
        .bind(&source_identity.content_hash)
        .bind(&captured_at)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        new_source_id
    };

    let final_status = if content_type == "image" {
        "pending_ocr"
    } else {
        "processed"
    };

    let mut chunk_count: i64 = 0;

    if content_type == "image" {
        emit_processing_task_progress(
            app,
            &id,
            &task_file_path,
            &display_title,
            "indexing",
            "processing",
            0,
            1,
            "Creating knowledge points and index",
        );
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
        emit_processing_task_progress(
            app,
            &id,
            &task_file_path,
            &display_title,
            "indexing",
            "processing",
            1,
            1,
            "Creating knowledge points and index",
        );
    } else {
        let routed_chunks =
            crate::capture::chunking::chunks_for_text(&normalized_content, &content_type);
        let routed_chunks: Vec<_> = if content_type == "url" {
            routed_chunks
                .into_iter()
                .filter(|c| !is_low_signal_url_chunk(&c.content))
                .collect()
        } else {
            routed_chunks
        };

        let app_state = app.state::<AppState>();
        let total_chunks = routed_chunks.len();

        for (idx, routed) in routed_chunks.iter().enumerate() {
            emit_processing_task_progress(
                app,
                &id,
                &task_file_path,
                &display_title,
                "indexing",
                "processing",
                chunk_count as usize,
                total_chunks,
                "Creating knowledge points and index",
            );
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

            match app_state.embedder.embed(para).await {
                Ok(vector) => {
                    let vector_id = capture_id_to_u64(&capture_id);
                    if let Err(e) = app_state.vector_store.add_vector(vector_id, &vector).await {
                        eprintln!("[CaptureProcessor] Failed to add vector: {}", e);
                    } else {
                        let _ = sqlx::query("UPDATE captures SET vector_id = ? WHERE id = ?")
                            .bind(vector_id as i64)
                            .bind(&capture_id)
                            .execute(pool)
                            .await;
                    }
                }
                Err(e) => eprintln!("[CaptureProcessor] Embedding failed (chunk {}): {}", idx, e),
            }

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

            let chunk_tag_pool = pool.clone();
            let chunk_tag_capture_id = capture_id.clone();
            let chunk_tag_content = para.to_string();
            tokio::spawn(async move {
                let tag_engine = crate::services::tag_engine::TagEngine::new(chunk_tag_pool);
                if let Err(e) = tag_engine
                    .process_new_capture(&chunk_tag_capture_id, &chunk_tag_content)
                    .await
                {
                    eprintln!(
                        "[CaptureProcessor] Chunk tag generation failed for {}: {}",
                        &chunk_tag_capture_id[..8.min(chunk_tag_capture_id.len())],
                        e
                    );
                }
            });

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
                    eprintln!("[ChunkRelation] capture relation analysis failed: {}", e);
                }
            });

            chunk_count += 1;
            emit_processing_task_progress(
                app,
                &id,
                &task_file_path,
                &display_title,
                "indexing",
                "processing",
                chunk_count as usize,
                total_chunks,
                "Creating knowledge points and index",
            );
        }

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

        let vs = app_state.vector_store.clone();
        tokio::spawn(async move {
            let _ = vs.save().await;
        });
    }

    sqlx::query("UPDATE inbox SET status = 'processed' WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

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
        "[CaptureProcessor] inbox {} processed, source {} got {} chunks",
        id, source_id, chunk_count
    );
    emit_processing_task_progress(
        app,
        &id,
        &task_file_path,
        &display_title,
        "completed",
        "done",
        chunk_count as usize,
        chunk_count.max(1) as usize,
        "Processing completed",
    );
    Ok(true)
}

fn capture_id_to_u64(id: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

fn is_low_signal_url_chunk(content: &str) -> bool {
    let text = content.trim();
    if text.is_empty() {
        return true;
    }

    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return true;
    }

    let char_count = text.chars().count();
    let alpha_num_count = text.chars().filter(|c| c.is_alphanumeric()).count();
    let url_line_count = lines
        .iter()
        .filter(|line| line.starts_with("http://") || line.starts_with("https://"))
        .count();
    let short_plain_lines = lines
        .iter()
        .filter(|line| {
            line.chars().count() <= 60
                && !line.contains('.')
                && !line.contains('!')
                && !line.contains('?')
                && !line.contains('。')
                && !line.contains('，')
                && !line.contains('！')
                && !line.contains('？')
        })
        .count();

    if char_count < 120 && lines.len() <= 3 {
        return true;
    }
    if alpha_num_count < 40 {
        return true;
    }

    lines.len() <= 4 && short_plain_lines + url_line_count >= lines.len() && char_count < 220
}
