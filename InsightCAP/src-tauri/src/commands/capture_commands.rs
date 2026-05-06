use crate::db::AppState;
use chrono::Utc;
use sqlx::SqlitePool;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn quick_capture(pool: State<'_, SqlitePool>, content: String) -> Result<(), String> {
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    let trimmed = content.trim();
    let content_type = if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
        && !trimmed.contains(' ')
    {
        "url"
    } else {
        "text"
    };

    let source_url = if content_type == "url" { trimmed } else { "" };

    sqlx::query(
        "INSERT INTO inbox (id, content, content_type, source_exe, source_url, window_title, session_id, status, captured_at) \
         VALUES (?, ?, ?, 'QuickCapture', ?, 'Quick Capture', '', 'pending', ?)"
    )
    .bind(&id)
    .bind(trimmed)
    .bind(content_type)
    .bind(source_url)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("quick_capture DB error: {}", e))?;

    println!(
        "[QuickCapture] Saved to inbox: {} ({})",
        &id[..8],
        content_type
    );
    Ok(())
}

#[tauri::command]
pub async fn create_temp_chunk(
    state: State<'_, AppState>,
    file_path: Option<String>,
    url: Option<String>,
    _conversation_id: String,
) -> Result<Vec<String>, String> {
    let db = &state.db;

    let settings = crate::settings::store::get_settings(db)
        .await
        .map_err(|e| e.to_string())?;
    let kb_path = settings.knowledge.kb_path.clone();
    let vision_config = crate::providers::llm::vision::VisionConfig::from_settings(
        &settings.ai_models.vision_model,
    );

    let parsed = crate::capture::file_parser::parse_content(
        &kb_path,
        file_path.clone(),
        url.clone(),
        None, // sessdata not provided in this command
        vision_config.as_ref(),
    )
    .await?;

    let mut chunk_ids: Vec<String> = Vec::new();
    let file_path_str = file_path.unwrap_or_default();
    let url_str = url.unwrap_or_default();

    let mut chunk_index = 0i64;
    for f_chunk in parsed.chunks {
        let routed_chunks = crate::capture::chunking::chunks_for_file_chunk(&f_chunk);

        for routed in routed_chunks {
            let chunk_id = Uuid::now_v7().to_string();
            let now = Utc::now().to_rfc3339();
            let para = routed.content;

            let vec = state
                .embedder
                .embed(&para)
                .await
                .map_err(|e| e.to_string())?;
            let vector_id = {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                chunk_id.hash(&mut hasher);
                hasher.finish()
            };
            state
                .vector_store
                .add_vector(vector_id, &vec)
                .await
                .map_err(|e| format!("vector store error: {}", e))?;

            sqlx::query(
                "INSERT INTO captures (id, source_id, type, raw_content, clean_content, \
                 capture_method, chunk_index, status, vector_id, content_type, knowledge_type, chunk_strategy, chunk_metadata, created_at, updated_at) \
                 VALUES (?, NULL, ?, ?, ?, 'temp_attachment', ?, 'processed', ?, ?, ?, ?, ?, ?)"
            )
            .bind(&chunk_id)
            .bind(&f_chunk.chunk_type)
            .bind(&para)
            .bind(&para)
            .bind(chunk_index)
            .bind(vector_id as i64)
            .bind(&routed.content_type)
            .bind(&routed.knowledge_type)
            .bind(&routed.chunk_strategy)
            .bind(&routed.metadata_json)
            .bind(&now)
            .bind(&now)
            .execute(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

            println!(
                "[TempChunk] {} {} para={}",
                &chunk_id[..8],
                if url_str.is_empty() {
                    &file_path_str
                } else {
                    &url_str
                },
                chunk_index
            );
            chunk_ids.push(chunk_id);
            chunk_index += 1;
        }
    }

    Ok(chunk_ids)
}

#[tauri::command]
pub async fn ingest_file(
    state: State<'_, AppState>,
    file_path: String,
    _conversation_id: Option<String>,
) -> Result<(), String> {
    let db = &state.db;
    println!("[IngestFile] Starting import: {}", file_path);

    let settings = crate::settings::store::get_settings(db)
        .await
        .map_err(|e| e.to_string())?;
    let kb_path = settings.knowledge.kb_path.clone();
    let vision_config = crate::providers::llm::vision::VisionConfig::from_settings(
        &settings.ai_models.vision_model,
    );

    let parsed =
        match crate::capture::file_parser::parse_file(&kb_path, &file_path, vision_config.as_ref())
            .await
        {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("[IngestFile] Parse failed for {}: {}", file_path, e);
                return Err(e);
            }
        };
    let now = Utc::now().to_rfc3339();
    let source_id = Uuid::now_v7().to_string();

    let raw_title = if parsed.title.trim().is_empty() {
        Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string()
    } else {
        parsed.title.clone()
    };
    let title = crate::utils::title_cleaner::clean_window_title(&raw_title);

    let source_clean_content = parsed
        .chunks
        .iter()
        .map(|c| c.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let source_identity =
        crate::capture::source_group::identity_for_file(&file_path, &source_clean_content);
    let source_group_id =
        crate::capture::source_group::get_or_create_source_group(db, &source_identity, &title)
            .await?;

    let local_doc_path: Option<String> = {
        let src_path = Path::new(&file_path);
        let ext = src_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let should_copy = matches!(
            ext.as_str(),
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
                | "wav"
                | "mp3"
                | "m4a"
                | "aac"
                | "flac"
                | "ogg"
                | "opus"
                | "webm"
        );
        if should_copy && !kb_path.is_empty() && src_path.exists() {
            let files_dir = Path::new(&kb_path).join("files");
            match std::fs::create_dir_all(&files_dir) {
                Err(e) => {
                    eprintln!("[IngestFile] Failed to create files directory: {}", e);
                    None
                }
                Ok(_) => {
                    let file_name = src_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| format!("{}.{}", source_id, ext));
                    let dest = files_dir.join(format!("{}_{}", &source_id[..8], file_name));
                    match std::fs::copy(src_path, &dest) {
                        Ok(_) => {
                            println!("[IngestFile] Copied file into KB: {}", dest.display());
                            Some(dest.to_string_lossy().to_string())
                        }
                        Err(e) => {
                            eprintln!("[IngestFile] Failed to copy file: {}", e);
                            None
                        }
                    }
                }
            }
        } else {
            None
        }
    };

    sqlx::query(
        "INSERT INTO sources (id, source_group_id, type, title, file_path, local_doc_path, clean_content, content_hash, capture_count, captured_at, updated_at) \
         VALUES (?, ?, 'file', ?, ?, ?, ?, ?, 0, ?, ?)"
    )
    .bind(&source_id)
    .bind(&source_group_id)
    .bind(&title)
    .bind(&file_path)
    .bind(&local_doc_path)
    .bind(&source_clean_content)
    .bind(&source_identity.content_hash)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await
    .map_err(|e| format!("insert source failed: {}", e))?;

    let mut chunk_count: usize = 0;

    for f_chunk in parsed.chunks {
        let routed_chunks = crate::capture::chunking::chunks_for_file_chunk(&f_chunk);

        for routed in routed_chunks {
            let chunk_id = Uuid::now_v7().to_string();
            let para = routed.content;
            let vec = state
                .embedder
                .embed(&para)
                .await
                .map_err(|e| e.to_string())?;
            let vector_id = {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                chunk_id.hash(&mut hasher);
                hasher.finish()
            };

            state
                .vector_store
                .add_vector(vector_id, &vec)
                .await
                .map_err(|e| format!("vector store error: {}", e))?;

            let capture_status = if f_chunk.status == "pending_ocr" {
                "pending_ocr"
            } else {
                "processed"
            };

            sqlx::query(
                "INSERT INTO captures (id, source_id, type, raw_content, clean_content, \
                 capture_method, chunk_index, status, vector_id, content_type, knowledge_type, chunk_strategy, chunk_metadata, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, 'source_import', ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&chunk_id)
            .bind(&source_id)
            .bind(&f_chunk.chunk_type)
            .bind(&para)
            .bind(&para)
            .bind(chunk_count as i64)
            .bind(capture_status)
            .bind(vector_id as i64)
            .bind(&routed.content_type)
            .bind(&routed.knowledge_type)
            .bind(&routed.chunk_strategy)
            .bind(&routed.metadata_json)
            .bind(&now)
            .bind(&now)
            .execute(db)
            .await
            .map_err(|e| format!("insert capture failed: {}", e))?;

            let space_engine = crate::services::space_engine::SpaceEngine::new(
                db.clone(),
                state.embedder.clone(),
                state.vector_store.clone(),
            );
            if let Err(e) = space_engine.assign_to_space(&chunk_id, &para).await {
                eprintln!(
                    "[IngestFile] Space assignment failed for {}: {}",
                    &chunk_id[..8],
                    e
                );
            }

            let rel_pool = db.clone();
            let rel_embedder = state.embedder.clone();
            let rel_vs = state.vector_store.clone();
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
                        "[ChunkRelation] ingest_file relation analysis failed: {}",
                        e
                    );
                }
            });

            chunk_count += 1;
        }
    }

    let tag_pool = db.clone();
    let tag_source_id = source_id.clone();
    let tag_full_content = source_clean_content.clone();
    tokio::spawn(async move {
        let tag_engine = crate::services::tag_engine::TagEngine::new(tag_pool);
        if let Err(e) = tag_engine
            .process_source(&tag_source_id, &tag_full_content)
            .await
        {
            eprintln!("[IngestFile] Source tag generation failed: {}", e);
        }
    });

    sqlx::query("UPDATE sources SET capture_count = ?, updated_at = ? WHERE id = ?")
        .bind(chunk_count as i64)
        .bind(&now)
        .bind(&source_id)
        .execute(db)
        .await
        .map_err(|e| format!("update source count failed: {}", e))?;

    let vs = state.vector_store.clone();
    tokio::spawn(async move {
        let _ = vs.save().await;
    });

    println!("[IngestFile] source={} chunks={}", source_id, chunk_count);
    Ok(())
}
