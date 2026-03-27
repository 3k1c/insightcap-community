pub mod clipboard;
pub mod file_parser;
pub mod keyboard;
pub mod metadata;
pub mod readability;
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

    let main_was_visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);

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
            println!("[CAPTURE] No selection. Showing Quick Capture UI...");
            if let Some(qc) = app.get_webview_window("quick-capture") {
                let _ = qc.show();
                let _ = qc.set_focus();
                let _ = qc.emit("show-quick-capture", ());
            }
            if !main_was_visible {
                if let Some(w) = app.get_webview_window("main") { let _ = w.hide(); }
            }
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

    sqlx::query(
        "INSERT INTO sources (id, type, title, file_path, clean_content, captured_at, updated_at) VALUES (?, 'file', ?, ?, ?, ?, ?)"
    )
    .bind(&source_id)
    .bind(&parsed.title)
    .bind(&path_str)
    .bind(&full_content)
    .bind(&now_iso)
    .bind(&now_iso)
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to create source: {}", e))?;

    println!("[CAPTURE] 📄 File '{}' saved as source.", parsed.title);
    // 向量化和完整的 captures 寫入由 background/capture_processor.rs 負責
    let _ = app.emit("knowledge-updated", ());
    Ok(())
}
