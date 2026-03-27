use std::fs;
use std::path::PathBuf;
use tauri::State;
use sqlx::SqlitePool;
use uuid::Uuid;
use chrono::Utc;
use base64::{Engine as _, engine::general_purpose};

#[tauri::command]
pub async fn open_document(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_document(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = PathBuf::from(&path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_document(path: String, format: String) -> Result<(), String> {
    if let Some(parent) = PathBuf::from(&path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = if format == "md" { "" } else { "" };
    fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn write_binary_file(absolute_path: String, base64_content: String) -> Result<(), String> {
    if let Some(parent) = PathBuf::from(&absolute_path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let decoded = general_purpose::STANDARD.decode(base64_content).map_err(|e| e.to_string())?;
    fs::write(&absolute_path, decoded).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_document(absolute_path: String, content: String) -> Result<(), String> {
    save_document(absolute_path, content).await
}

#[tauri::command]
pub async fn read_image_base64(absolute_path: String) -> Result<String, String> {
    let bytes = fs::read(&absolute_path).map_err(|e| e.to_string())?;
    let b64 = general_purpose::STANDARD.encode(bytes);
    let ext = PathBuf::from(&absolute_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "image/png",
    };
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
pub async fn copy_image_to_assets(doc_path: String, image_abs_path: String) -> Result<String, String> {
    let doc_buf = PathBuf::from(doc_path);
    let parent = doc_buf.parent().unwrap_or_else(|| std::path::Path::new(""));
    let assets_dir = parent.join("assets");
    fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;
    
    let img_buf = PathBuf::from(&image_abs_path);
    let file_name = img_buf.file_name().ok_or("Invalid image path")?;
    let unique_name = format!("{}_{}", Uuid::now_v7().to_string().chars().take(8).collect::<String>(), file_name.to_string_lossy());
    
    let dest_path = assets_dir.join(&unique_name);
    fs::copy(&image_abs_path, &dest_path).map_err(|e| e.to_string())?;
    
    // Return relative path like ./assets/filename.png
    Ok(format!("./assets/{}", unique_name))
}

#[tauri::command]
pub async fn save_editor_to_knowledge(
    pool: State<'_, SqlitePool>,
    title: String,
    content: String,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    
    let clean_content = content.clone(); // Tiptap content is markdown
    
    let mut hasher = Sha256::new();
    hasher.update(clean_content.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());
    let now = Utc::now().to_rfc3339();

    // Check if source with same title and type editor already exists
    let existing_source_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM sources WHERE title = ? AND type = 'editor'"
    )
    .bind(&title)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let source_id = if let Some(id) = existing_source_id {
        // Source exists, clear old captures
        sqlx::query("DELETE FROM captures WHERE source_id = ?")
            .bind(&id)
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
        
        // Update source hash and time
        sqlx::query("UPDATE sources SET content_hash = ?, updated_at = ?, clean_content = ? WHERE id = ?")
            .bind(&content_hash)
            .bind(&now)
            .bind(&clean_content)
            .bind(&id)
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
        id
    } else {
        // Create new source
        let new_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO sources (id, type, title, clean_content, content_hash, captured_at, updated_at) VALUES (?, 'editor', ?, ?, ?, ?, ?)"
        )
        .bind(&new_id)
        .bind(&title)
        .bind(&clean_content)
        .bind(&content_hash)
        .bind(&now)
        .bind(&now)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
        new_id
    };

    // Split content by Markdown headers (H1, H2, H3)
    let chunks = split_markdown_by_headings(&clean_content);
    
    for (idx, chunk_text) in chunks.into_iter().enumerate() {
        if chunk_text.trim().is_empty() {
            continue;
        }

        let capture_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO captures (id, source_id, type, raw_content, clean_content, capture_method, chunk_index, status, created_at, updated_at) VALUES (?, ?, 'text', ?, ?, 'editor_export', ?, 'processed', ?, ?)"
        )
        .bind(&capture_id)
        .bind(&source_id)
        .bind(&chunk_text)
        .bind(&chunk_text)
        .bind(idx as i32)
        .bind(&now)
        .bind(&now)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

        // 觸發背景 tag 與 space (非同步)
        let pool_clone = pool.inner().clone();
        let capture_id_clone = capture_id.clone();
        let chunk_text_clone = chunk_text.clone();
        tauri::async_runtime::spawn(async move {
            let tag_engine = crate::services::tag_engine::TagEngine::new(pool_clone.clone());
            let _ = tag_engine.process_new_capture(&capture_id_clone, &chunk_text_clone).await;
            
            let space_engine = crate::services::space_engine::SpaceEngine::new(pool_clone);
            let _ = space_engine.assign_to_space(&capture_id_clone, &chunk_text_clone).await;
        });
    }

    Ok(())
}

fn split_markdown_by_headings(markdown: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for line in markdown.lines() {
        if line.starts_with("# ") || line.starts_with("## ") || line.starts_with("### ") {
            if !current_chunk.trim().is_empty() {
                chunks.push(current_chunk.clone());
                current_chunk.clear();
            }
        }
        current_chunk.push_str(line);
        current_chunk.push('\n');
    }
    
    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk);
    }

    if chunks.is_empty() {
        chunks.push(markdown.to_string());
    }

    chunks
}
