use sqlx::SqlitePool;
use tauri::State;
use chrono::Utc;
use uuid::Uuid;
use crate::db::AppState;
use std::path::Path;

/// Quick Capture：前端手動輸入框送出後寫入 inbox
#[tauri::command]
pub async fn quick_capture(
    pool: State<'_, SqlitePool>,
    content: String,
) -> Result<(), String> {
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
         VALUES (?, ?, ?, 'QuickCapture', ?, '快速擷取', '', 'pending', ?)"
    )
    .bind(&id)
    .bind(trimmed)
    .bind(content_type)
    .bind(source_url)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("quick_capture DB error: {}", e))?;

    println!("[QuickCapture] Saved to inbox: {} ({})", &id[..8], content_type);
    Ok(())
}

/// 對話臨時附件：解析檔案/URL 內容，產生 embedding，存入 captures 表供當次對話使用
#[tauri::command]
pub async fn create_temp_chunk(
    state: State<'_, AppState>,
    file_path: Option<String>,
    url: Option<String>,
    _conversation_id: String,
) -> Result<Vec<String>, String> {
    let db = &state.db;

    let kb_path = {
        let settings = crate::settings::store::get_settings(db).await.map_err(|e| e.to_string())?;
        settings.knowledge.kb_path
    };

    let parsed = crate::capture::file_parser::parse_content(
        &kb_path,
        file_path.clone(),
        url.clone(),
        None, // sessdata — Bilibili 登入暫不支援
    ).await?;

    let mut chunk_ids: Vec<String> = Vec::new();
    let file_path_str = file_path.unwrap_or_default();
    let url_str = url.unwrap_or_default();

    for f_chunk in parsed.chunks {
        // 段落切分（與 ref 版一致）
        let paragraphs: Vec<String> = f_chunk.content
            .split("\n\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let paragraphs = if paragraphs.is_empty() {
            if f_chunk.content.trim().is_empty() { continue; }
            vec![f_chunk.content.clone()]
        } else {
            paragraphs
        };

        for (idx, para) in paragraphs.into_iter().enumerate() {
            let chunk_id = Uuid::now_v7().to_string();
            let now = Utc::now().to_rfc3339();

            // 產生 embedding 並寫入 vector store（使用確定性 hash 作為 vector_id）
            let vec = state.embedder.embed(&para).await.map_err(|e| e.to_string())?;
            let vector_id = {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                chunk_id.hash(&mut hasher);
                hasher.finish()
            };
            state.vector_store.add_vector(vector_id, &vec).await
                .map_err(|e| format!("vector store error: {}", e))?;

            sqlx::query(
                "INSERT INTO captures (id, source_id, type, raw_content, clean_content, \
                 capture_method, chunk_index, status, vector_id, created_at, updated_at) \
                 VALUES (?, NULL, ?, ?, ?, 'temp_attachment', ?, 'processed', ?, ?, ?)"
            )
            .bind(&chunk_id)
            .bind(&f_chunk.chunk_type)
            .bind(&para)
            .bind(&para)
            .bind(idx as i64)
            .bind(vector_id as i64)
            .bind(&now)
            .bind(&now)
            .execute(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

            println!("[TempChunk] {} {} para={}", &chunk_id[..8], if url_str.is_empty() { &file_path_str } else { &url_str }, idx);
            chunk_ids.push(chunk_id);
        }
    }

    Ok(chunk_ids)
}

/// 匯入本地檔案到 sources + captures（儲存庫）
#[tauri::command]
pub async fn ingest_file(
    state: State<'_, AppState>,
    file_path: String,
    _conversation_id: Option<String>,
) -> Result<(), String> {
    let db = &state.db;

    let kb_path = {
        let settings = crate::settings::store::get_settings(db).await.map_err(|e| e.to_string())?;
        settings.knowledge.kb_path
    };

    let parsed = crate::capture::file_parser::parse_file(&kb_path, &file_path).await?;
    let now = Utc::now().to_rfc3339();
    let source_id = Uuid::now_v7().to_string();

    let raw_title = if parsed.title.trim().is_empty() {
        Path::new(&file_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("未命名文件")
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

    // 將來源檔案複製到 kb_path/files/ 目錄，確保匯出時完整
    let local_doc_path: Option<String> = {
        let src_path = Path::new(&file_path);
        let ext = src_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let should_copy = matches!(ext.as_str(),
            "txt" | "log" | "md" | "pdf" | "doc" | "docx" | "ppt" | "pptx"
            | "xls" | "xlsx" | "csv" | "html" | "htm" | "rtf" | "epub"
        );
        if should_copy && !kb_path.is_empty() && src_path.exists() {
            let files_dir = Path::new(&kb_path).join("files");
            match std::fs::create_dir_all(&files_dir) {
                Err(e) => {
                    eprintln!("[IngestFile] 無法建立 files 目錄: {}", e);
                    None
                }
                Ok(_) => {
                    let file_name = src_path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| format!("{}.{}", source_id, ext));
                    let dest = files_dir.join(format!("{}_{}", &source_id[..8], file_name));
                    match std::fs::copy(src_path, &dest) {
                        Ok(_) => {
                            println!("[IngestFile] 📋 已複製至知識庫: {}", dest.display());
                            Some(dest.to_string_lossy().to_string())
                        }
                        Err(e) => {
                            eprintln!("[IngestFile] 複製檔案失敗: {}", e);
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
        "INSERT INTO sources (id, type, title, file_path, local_doc_path, clean_content, capture_count, captured_at, updated_at) \
         VALUES (?, 'file', ?, ?, ?, ?, 0, ?, ?)"
    )
    .bind(&source_id)
    .bind(&title)
    .bind(&file_path)
    .bind(&local_doc_path)
    .bind(&source_clean_content)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await
    .map_err(|e| format!("insert source failed: {}", e))?;

    let mut chunk_count: usize = 0;

    for f_chunk in parsed.chunks {
        let paragraphs: Vec<String> = f_chunk
            .content
            .split("\n\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let paragraphs = if paragraphs.is_empty() {
            if f_chunk.content.trim().is_empty() {
                continue;
            }
            vec![f_chunk.content.clone()]
        } else {
            paragraphs
        };

        for (idx, para) in paragraphs.into_iter().enumerate() {
            let chunk_id = Uuid::now_v7().to_string();
            let vec = state.embedder.embed(&para).await.map_err(|e| e.to_string())?;
            let vector_id = {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                chunk_id.hash(&mut hasher);
                hasher.finish()
            };

            state.vector_store.add_vector(vector_id, &vec).await
                .map_err(|e| format!("vector store error: {}", e))?;

            sqlx::query(
                "INSERT INTO captures (id, source_id, type, raw_content, clean_content, \
                 capture_method, chunk_index, status, vector_id, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, 'source_import', ?, 'processed', ?, ?, ?)"
            )
            .bind(&chunk_id)
            .bind(&source_id)
            .bind(&f_chunk.chunk_type)
            .bind(&para)
            .bind(&para)
            .bind(idx as i64)
            .bind(vector_id as i64)
            .bind(&now)
            .bind(&now)
            .execute(db)
            .await
            .map_err(|e| format!("insert capture failed: {}", e))?;

            // Tagger：非同步提取標籤（不阻塞主流程）
            let tag_pool = db.clone();
            let tag_cid = chunk_id.clone();
            let tag_content = para.clone();
            tokio::spawn(async move {
                let tag_engine = crate::services::tag_engine::TagEngine::new(tag_pool);
                if let Err(e) = tag_engine.process_new_capture(&tag_cid, &tag_content).await {
                    eprintln!("[IngestFile] Tag generation failed for {}: {}", &tag_cid[..8.min(tag_cid.len())], e);
                }
            });

            chunk_count += 1;
        }
    }

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
