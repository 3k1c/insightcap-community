pub mod clipboard;
pub mod encoding;
pub mod file_parser;
pub mod keyboard;
pub mod metadata;
pub mod readability;
pub mod video_parser;
pub mod extractors;
pub mod attachment_manager;

use sqlx::SqlitePool;
use tauri::{Emitter, Manager};

/// 正規化影片 URL
fn normalize_video_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.contains("bilibili.com/video/") {
        if let Some(start) = trimmed.find("/video/") {
            let bvid = trimmed[start + 7..]
                .split('/').next().unwrap_or("")
                .split('?').next().unwrap_or("");
            if !bvid.is_empty() {
                return format!("https://www.bilibili.com/video/{}", bvid);
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

/// 全域熱鍵觸發擷取
pub async fn trigger_capture(app: tauri::AppHandle) -> Result<(), String> {
    let pool = app.state::<SqlitePool>();
    println!("\n[CAPTURE] 🚀 Hotkey triggered. Starting capture...");

    // 1. 清空剪貼簿
    let _ = clipboard::clear_clipboard();

    // 2. 模擬 Ctrl+C
    keyboard::simulate_copy().map_err(|e| format!("Keyboard error: {}", e))?;

    // 3. 等待 OS 回寫
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // 4. 讀取剪貼簿（重試 3 次）
    let mut clipboard_result = clipboard::read_clipboard();
    for attempt in 0..3 {
        if clipboard_result.is_ok() { break; }
        println!("[CAPTURE]   Retrying clipboard {}/3...", attempt + 1);
        tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
        clipboard_result = clipboard::read_clipboard();
    }

    let clipboard_data = match clipboard_result {
        Ok(data) => data,
        Err(_) => {
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
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                if ["pdf","docx","xlsx","csv","txt","md","png","jpg","jpeg","webp"].contains(&ext.as_str()) {
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
        clipboard::ClipboardContent::Data { text, has_image, image_bytes } => (has_image, text, image_bytes),
    };

    let content_text = text_content.clone().unwrap_or_default();
    let mut content_type = if has_image { "image" } else { "text" };
    let mut source_url = window_meta.url.clone();

    // URL 偵測
    if content_type == "text" && !content_text.is_empty() {
        let trimmed = content_text.trim();
        if (trimmed.starts_with("http://") || trimmed.starts_with("https://")) && !trimmed.contains(' ') {
            content_type = "url";
            source_url = normalize_video_url(trimmed);
        }
    }

    // 寫入 inbox 表（v2 Schema）
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

    println!("[CAPTURE] 🎉 Saved to inbox. Background processor will handle it.");
    let _ = app.emit("capture-success", ());
    Ok(())
}

/// 需要複製到知識庫的文件副檔名（排除影片）
fn should_copy_to_kb(ext: &str) -> bool {
    matches!(ext, "txt" | "log" | "md" | "pdf" | "doc" | "docx" | "ppt" | "pptx"
                | "xls" | "xlsx" | "csv" | "html" | "htm" | "rtf" | "epub" | "code")
}

/// 直接處理剪貼簿中的文件（跳過 inbox，直接寫入 sources + captures）
async fn process_clipboard_file(
    app: tauri::AppHandle,
    file_path: std::path::PathBuf,
) -> Result<(), String> {
    let pool = app.state::<SqlitePool>().inner().clone();

    let mut kb_path = String::new();
    if let Ok(settings) = crate::settings::store::get_settings(&pool).await {
        kb_path = settings.knowledge.kb_path;
    }

    let path_str = file_path.to_string_lossy().to_string();
    let parsed = crate::capture::file_parser::parse_file(&kb_path, &path_str).await?;
    let now_iso = chrono::Utc::now().to_rfc3339();
    let source_id = uuid::Uuid::now_v7().to_string();

    let full_content = parsed.chunks.iter()
        .map(|c| c.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    // 複製檔案到知識庫 files 目錄（文件類型，排除影片）
    let local_doc_path: Option<String> = {
        let ext = file_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if should_copy_to_kb(&ext) && !kb_path.is_empty() {
            let files_dir = std::path::Path::new(&kb_path).join("files");
            if let Err(e) = std::fs::create_dir_all(&files_dir) {
                eprintln!("[CAPTURE] 無法建立 files 目錄: {}", e);
                None
            } else {
                let file_name = file_path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("{}.{}", source_id, ext));
                let dest = files_dir.join(format!("{}_{}", &source_id[..8], file_name));
                match std::fs::copy(&file_path, &dest) {
                    Ok(_) => {
                        println!("[CAPTURE] 📋 已複製至知識庫: {}", dest.display());
                        Some(dest.to_string_lossy().to_string())
                    }
                    Err(e) => {
                        eprintln!("[CAPTURE] 複製檔案失敗: {}", e);
                        None
                    }
                }
            }
        } else {
            None
        }
    };

    sqlx::query(
        "INSERT INTO sources (id, type, title, file_path, local_doc_path, clean_content, captured_at, updated_at) VALUES (?, 'file', ?, ?, ?, ?, ?, ?)"
    )
    .bind(&source_id)
    .bind(&parsed.title)
    .bind(&path_str)
    .bind(&local_doc_path)
    .bind(&full_content)
    .bind(&now_iso)
    .bind(&now_iso)
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to create source: {}", e))?;

    println!("[CAPTURE] 📄 File '{}' saved as source.", parsed.title);

    // 切分段落 → 寫入 captures → embedding + tag
    let app_state = app.state::<crate::db::AppState>();
    let mut chunk_count: i64 = 0;

    for f_chunk in &parsed.chunks {
        let paragraphs: Vec<String> = f_chunk.content
            .split("\n\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let paragraphs = if paragraphs.is_empty() {
            vec![f_chunk.content.clone()]
        } else {
            paragraphs
        };

        for (idx, para) in paragraphs.into_iter().enumerate() {
            let chunk_id = uuid::Uuid::now_v7().to_string();

            // Embedding
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
                        Err(e) => { eprintln!("[CAPTURE] vector store error: {}", e); None }
                    }
                }
                Err(e) => { eprintln!("[CAPTURE] embed error: {}", e); None }
            };

            sqlx::query(
                "INSERT INTO captures (id, source_id, type, raw_content, clean_content, \
                 capture_method, chunk_index, status, vector_id, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, 'hotkey', ?, 'processed', ?, ?, ?)"
            )
            .bind(&chunk_id)
            .bind(&source_id)
            .bind(&f_chunk.chunk_type)
            .bind(&para)
            .bind(&para)
            .bind((chunk_count + idx as i64) as i64)
            .bind(vector_id_opt)
            .bind(&now_iso)
            .bind(&now_iso)
            .execute(&pool)
            .await
            .map_err(|e| format!("insert capture failed: {}", e))?;

            // Tagger：非同步提取標籤
            let tag_pool = pool.clone();
            let tag_cid = chunk_id.clone();
            let tag_content = para.clone();
            tokio::spawn(async move {
                let tag_engine = crate::services::tag_engine::TagEngine::new(tag_pool);
                if let Err(e) = tag_engine.process_new_capture(&tag_cid, &tag_content).await {
                    eprintln!("[CAPTURE] Tag failed for {}: {}", &tag_cid[..8.min(tag_cid.len())], e);
                }
            });

            chunk_count += 1;
        }
    }

    // 更新 capture_count
    sqlx::query("UPDATE sources SET capture_count = ?, updated_at = ? WHERE id = ?")
        .bind(chunk_count)
        .bind(&now_iso)
        .bind(&source_id)
        .execute(&pool)
        .await
        .map_err(|e| format!("update source count failed: {}", e))?;

    // 非同步儲存向量索引
    let vs = app_state.vector_store.clone();
    tokio::spawn(async move { let _ = vs.save().await; });

    println!("[CAPTURE] ✅ File '{}' processed: {} chunks with tags", parsed.title, chunk_count);
    let _ = app.emit("knowledge-updated", ());
    Ok(())
}
