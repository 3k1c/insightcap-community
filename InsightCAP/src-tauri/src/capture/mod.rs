pub mod attachment_manager;
pub mod chunking;
pub mod clipboard;
pub mod encoding;
pub mod extractors;
pub mod file_parser;
pub mod keyboard;
pub mod metadata;
pub mod readability;
pub mod source_group;
pub mod video_parser;

use sqlx::SqlitePool;
use tauri::{Emitter, Manager};
use crate::tray_status::{set_tray_status, TrayStatus};

fn normalize_video_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.contains("bilibili.com/video/") {
        if let Some(start) = trimmed.find("/video/") {
            let bvid = trimmed[start + 7..]
                .split('/')
                .next()
                .unwrap_or("")
                .split('?')
                .next()
                .unwrap_or("");
            if !bvid.is_empty() {
                return format!("https://www.bilibili.com/video/{}", bvid);
            }
        }
    }
    if trimmed.contains("youtu.be/") {
        if let Some(id) = trimmed.split("youtu.be/").nth(1) {
            let video_id = id
                .split('?')
                .next()
                .unwrap_or("")
                .split('/')
                .next()
                .unwrap_or("");
            if !video_id.is_empty() {
                return format!("https://www.youtube.com/watch?v={}", video_id);
            }
        }
    }
    if trimmed.contains("youtube.com/watch") {
        if let Some(start) = trimmed.find("v=") {
            let vid = trimmed[start + 2..].split('&').next().unwrap_or("");
            if !vid.is_empty() {
                return format!("https://www.youtube.com/watch?v={}", vid);
            }
        }
    }
    trimmed.to_string()
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CapturePayload {
    pub id: String,
    pub content: String,
    pub content_type: String,
    pub file_path: Option<String>,
    pub source_exe: String,
    pub source_pid: u32,
    pub source_url: String,
    pub source_file_path: String,
    pub window_title: String,
    pub session_id: String,
    pub captured_at: String,
}

pub async fn trigger_capture(app: tauri::AppHandle) -> Result<(), String> {
    set_tray_status(&app, TrayStatus::Capturing);
    let pool = app.state::<SqlitePool>();
    println!("\n[CAPTURE] Hotkey triggered. Starting capture...");

    let _ = clipboard::clear_clipboard();

    keyboard::simulate_copy().map_err(|e| format!("Keyboard error: {}", e))?;

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    let mut clipboard_result = clipboard::read_clipboard();
    for attempt in 0..3 {
        if clipboard_result.is_ok() {
            break;
        }
        println!("[CAPTURE]   Retrying clipboard {}/3...", attempt + 1);
        tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
        clipboard_result = clipboard::read_clipboard();
    }

    let clipboard_data = match clipboard_result {
        Ok(data) => data,
        Err(_) => {
            set_tray_status(&app, TrayStatus::Idle);
            println!("[CAPTURE] No selection detected, aborting capture.");
            return Ok(());
        }
    };

    let window_meta = metadata::get_active_window();
    let session_id = uuid::Uuid::now_v7().to_string();
    let now_iso = chrono::Utc::now().to_rfc3339();
    let id_str = uuid::Uuid::now_v7().to_string();

    let (has_image, text_content, image_bytes) = match clipboard_data {
        clipboard::ClipboardContent::Files(paths) => {
            for path in paths {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                if [
                    "pdf", "docx", "xlsx", "csv", "txt", "md", "png", "jpg", "jpeg", "webp",
                ]
                .contains(&ext.as_str())
                {
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = process_clipboard_file(app_clone, path).await {
                            eprintln!("[CAPTURE] File processing error: {}", e);
                        }
                    });
                }
            }
            return Ok(());
        }
        clipboard::ClipboardContent::Data {
            text,
            has_image,
            image_bytes,
        } => (has_image, text, image_bytes),
    };

    let content_text = text_content.clone().unwrap_or_default();
    let mut content_type = if has_image { "image" } else { "text" };
    let mut source_url = window_meta.url.clone();

    if content_type == "text" && !content_text.is_empty() {
        let trimmed = content_text.trim();
        if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
            && !trimmed.contains(' ')
        {
            content_type = "url";
            source_url = normalize_video_url(trimmed);
        }
    }

    sqlx::query(
        "INSERT INTO inbox (id, content, content_type, source_exe, source_pid, source_url, window_title, image_data, session_id, status, captured_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?)"
    )
    .bind(&id_str)
    .bind(&content_text)
    .bind(content_type)
    .bind(&window_meta.app_name)
    .bind(window_meta.pid as i64)
    .bind(&source_url)
    .bind(&window_meta.title)
    .bind(&image_bytes)
    .bind(&session_id)
    .bind(&now_iso)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    println!("[CAPTURE] Saved to inbox. Background processor will handle it.");
    let _ = app.emit("capture-success", ());
    Ok(())
}

fn should_copy_to_kb(ext: &str) -> bool {
    matches!(
        ext,
        "txt"
            | "log"
            | "md"
            | "pdf"
            | "doc"
            | "docx"
            | "ppt"
            | "pptx"
            | "xls"
            | "xlsx"
            | "csv"
            | "html"
            | "htm"
            | "rtf"
            | "epub"
            | "code"
    )
}

async fn process_clipboard_file(
    app: tauri::AppHandle,
    file_path: std::path::PathBuf,
) -> Result<(), String> {
    let pool = app.state::<SqlitePool>().inner().clone();

    let (kb_path, vision_config) = match crate::settings::store::get_settings(&pool).await {
        Ok(settings) => {
            let vc = crate::providers::llm::vision::VisionConfig::from_settings(
                &settings.ai_models.vision_model,
            );
            (settings.knowledge.kb_path, vc)
        }
        Err(_) => (String::new(), None),
    };

    let path_str = file_path.to_string_lossy().to_string();
    let parsed =
        crate::capture::file_parser::parse_file(&kb_path, &path_str, vision_config.as_ref())
            .await?;
    let now_iso = chrono::Utc::now().to_rfc3339();
    let source_id = uuid::Uuid::now_v7().to_string();

    let full_content = parsed
        .chunks
        .iter()
        .map(|c| c.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let source_identity = crate::capture::source_group::identity_for_file(&path_str, &full_content);
    let source_group_id = crate::capture::source_group::get_or_create_source_group(
        &pool,
        &source_identity,
        &parsed.title,
    )
    .await?;

    let local_doc_path: Option<String> = {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if should_copy_to_kb(&ext) && !kb_path.is_empty() {
            let files_dir = std::path::Path::new(&kb_path).join("files");
            if let Err(e) = std::fs::create_dir_all(&files_dir) {
                eprintln!("[CAPTURE] Failed to create kb/files directory: {}", e);
                None
            } else {
                let file_name = file_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("{}.{}", source_id, ext));
                let dest = files_dir.join(format!("{}_{}", &source_id[..8], file_name));
                match std::fs::copy(&file_path, &dest) {
                    Ok(_) => {
                        println!("[CAPTURE] Copied source file to {}", dest.display());
                        Some(dest.to_string_lossy().to_string())
                    }
                    Err(e) => {
                        eprintln!("[CAPTURE] Failed to copy source file: {}", e);
                        None
                    }
                }
            }
        } else {
            None
        }
    };

    sqlx::query(
        "INSERT INTO sources (id, source_group_id, type, title, file_path, local_doc_path, clean_content, content_hash, captured_at, updated_at) VALUES (?, ?, 'file', ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&source_id)
    .bind(&source_group_id)
    .bind(&parsed.title)
    .bind(&path_str)
    .bind(&local_doc_path)
    .bind(&full_content)
    .bind(&source_identity.content_hash)
    .bind(&now_iso)
    .bind(&now_iso)
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to create source: {}", e))?;

    println!("[CAPTURE] File '{}' saved as source.", parsed.title);

    let app_state = app.state::<crate::db::AppState>();
    let mut chunk_count: i64 = 0;

    for f_chunk in &parsed.chunks {
        let routed_chunks = crate::capture::chunking::chunks_for_file_chunk(f_chunk);

        for routed in routed_chunks {
            let chunk_id = uuid::Uuid::now_v7().to_string();
            let para = routed.content;

            let vector_id_opt: Option<i64> = match app_state.embedder.embed(&para).await {
                Ok(vec) => {
                    let vid = {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut h = DefaultHasher::new();
                        chunk_id.hash(&mut h);
                        h.finish()
                    };
                    match app_state.vector_store.add_vector(vid, &vec).await {
                        Ok(_) => Some(vid as i64),
                        Err(e) => {
                            eprintln!("[CAPTURE] vector store error: {}", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[CAPTURE] embed error: {}", e);
                    None
                }
            };

            let capture_status = if f_chunk.status == "pending_ocr" {
                "pending_ocr"
            } else {
                "processed"
            };

            sqlx::query(
                "INSERT INTO captures (id, source_id, type, raw_content, clean_content, \
                 capture_method, chunk_index, status, vector_id, content_type, knowledge_type, chunk_strategy, chunk_metadata, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, 'hotkey', ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&chunk_id)
            .bind(&source_id)
            .bind(&f_chunk.chunk_type)
            .bind(&para)
            .bind(&para)
            .bind(chunk_count)
            .bind(capture_status)
            .bind(vector_id_opt)
            .bind(&routed.content_type)
            .bind(&routed.knowledge_type)
            .bind(&routed.chunk_strategy)
            .bind(&routed.metadata_json)
            .bind(&now_iso)
            .bind(&now_iso)
            .execute(&pool)
            .await
            .map_err(|e| format!("insert capture failed: {}", e))?;

            let space_engine = crate::services::space_engine::SpaceEngine::new(
                pool.clone(),
                app_state.embedder.clone(),
                app_state.vector_store.clone(),
            );
            if let Err(e) = space_engine.assign_to_space(&chunk_id, &para).await {
                eprintln!(
                    "[CAPTURE] Space assignment failed for {}: {}",
                    &chunk_id[..8],
                    e
                );
            }

            let chunk_tag_pool = pool.clone();
            let chunk_tag_id = chunk_id.clone();
            let chunk_tag_content = para.clone();
            tokio::spawn(async move {
                let tag_engine = crate::services::tag_engine::TagEngine::new(chunk_tag_pool);
                if let Err(e) = tag_engine
                    .process_new_capture(&chunk_tag_id, &chunk_tag_content)
                    .await
                {
                    eprintln!(
                        "[CAPTURE] Chunk tag generation failed for {}: {}",
                        &chunk_tag_id[..8.min(chunk_tag_id.len())],
                        e
                    );
                }
            });

            let rel_pool = pool.clone();
            let rel_embedder = app_state.embedder.clone();
            let rel_vs = app_state.vector_store.clone();
            let rel_cid = chunk_id.clone();
            let rel_content = para.clone();
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
                    eprintln!(
                        "[ChunkRelation] Clipboard file relation build failed: {}",
                        e
                    );
                }
            });

            chunk_count += 1;
        }
    }

    let tag_pool = pool.clone();
    let tag_source_id = source_id.clone();
    let tag_full_content = full_content.clone();
    tokio::spawn(async move {
        let tag_engine = crate::services::tag_engine::TagEngine::new(tag_pool);
        if let Err(e) = tag_engine
            .process_source(&tag_source_id, &tag_full_content)
            .await
        {
            eprintln!("[CAPTURE] Source tag generation failed: {}", e);
        }
    });

    sqlx::query("UPDATE sources SET capture_count = ?, updated_at = ? WHERE id = ?")
        .bind(chunk_count)
        .bind(&now_iso)
        .bind(&source_id)
        .execute(&pool)
        .await
        .map_err(|e| format!("update source count failed: {}", e))?;

    let vs = app_state.vector_store.clone();
    tokio::spawn(async move {
        let _ = vs.save().await;
    });

    println!(
        "[CAPTURE] File '{}' processed: {} chunks with tags",
        parsed.title, chunk_count
    );
    let _ = app.emit("knowledge-updated", ());
    Ok(())
}
