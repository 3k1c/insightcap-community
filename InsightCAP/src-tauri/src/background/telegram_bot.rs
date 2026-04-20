//! Telegram Bot Polling Worker
//!
//! 負責與 Telegram API 進行長輪詢 (long-polling)，
//! 接收訊息並處理 RAG 查詢或指令。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Manager};
use tokio::time::sleep;
use uuid::Uuid;

use crate::db::AppState;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider, StreamToken};
use crate::services::rag_engine::RagEngine;

const POLL_TIMEOUT_SECS: u64 = 30;
const RETRY_DELAY_SECS: u64 = 10;
const TELEGRAM_MSG_LIMIT: usize = 4096;
/// sendMessageDraft 節流間隔
const DRAFT_THROTTLE_MS: u64 = 300;

static POLLING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 啟動 Telegram Bot polling worker
pub fn start_telegram_bot(app: AppHandle) {
    if POLLING_ACTIVE.swap(true, Ordering::SeqCst) {
        println!("[TelegramBot] Already running, skipping duplicate start");
        return;
    }
    tauri::async_runtime::spawn(async move {
        println!("[TelegramBot] Worker started, waiting for settings...");
        // 延遲 3 秒啟動以確保 DB 已就緒
        sleep(Duration::from_secs(3)).await;
        telegram_poll_loop(app).await;
        POLLING_ACTIVE.store(false, Ordering::SeqCst);
    });
}

async fn telegram_poll_loop(app: AppHandle) {
    let mut offset: i64 = 0;
    let client = Client::builder()
        .timeout(Duration::from_secs(POLL_TIMEOUT_SECS + 10))
        .build()
        .expect("Failed to create HTTP client");

    loop {
        // 每次循環重新讀取設定，以應對用戶動態變更
        let settings = {
            let state = app.state::<AppState>();
            match crate::settings::store::get_settings(&state.db).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[TelegramBot] 讀取設定失敗: {}", e);
                    sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
                    continue;
                }
            }
        };

        let tg = &settings.telegram;

        // 若未啟用或 Token 為空則等待
        if !tg.enabled || tg.bot_token.trim().is_empty() {
            sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
            continue;
        }

        let bot_token = tg.bot_token.trim().to_string();
        let mut allowed_ids = tg.allowed_user_ids.clone();

        // 呼叫 getUpdates (long-polling)
        let url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout={}",
            bot_token, offset, POLL_TIMEOUT_SECS
        );

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[TelegramBot] getUpdates 失敗: {}", e);
                sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
                continue;
            }
        };

        let body: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[TelegramBot] 解析響應失敗: {}", e);
                sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
                continue;
            }
        };

        if body["ok"].as_bool() != Some(true) {
            let error_code = body["error_code"].as_i64().unwrap_or(0);
            if error_code == 409 {
                // 另一個實作正在輪詢，稍後再試
                eprintln!("[TelegramBot] 409 Conflict: 偵測到多個 Bot 實例，等待 60 秒後重試");
                sleep(Duration::from_secs(60)).await;
            } else {
                eprintln!("[TelegramBot] API 錯誤: {}", body);
                sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
            }
            continue;
        }

        let updates = match body["result"].as_array() {
            Some(arr) => arr.clone(),
            None => {
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        for update in &updates {
            // 更新 offset
            if let Some(uid) = update["update_id"].as_i64() {
                offset = uid + 1;
            }

            // 處理 callback_query
            let callback = &update["callback_query"];
            if !callback.is_null() {
                let cb_chat_id = callback["message"]["chat"]["id"].as_i64().unwrap_or(0);
                let cb_user_id = callback["from"]["id"].as_i64().unwrap_or(cb_chat_id);
                let cb_data = callback["data"].as_str().unwrap_or("").to_string();
                let cb_id = callback["id"].as_str().unwrap_or("").to_string();
                let cb_msg_id = callback["message"]["message_id"].as_i64();

                if !allowed_ids.is_empty() && !allowed_ids.contains(&cb_user_id) {
                    continue;
                }

                // 回應 callback
                let _ = answer_callback_query(&bot_token, &cb_id).await;
                if let Err(e) =
                    handle_callback(&app, &bot_token, cb_chat_id, &cb_data, cb_msg_id).await
                {
                    eprintln!("[TelegramBot] callback 處理失敗: {}", e);
                    let _ =
                        send_message(&bot_token, cb_chat_id, &format!("⚠️ 處理失敗：{}", e)).await;
                }
                continue;
            }

            let message = &update["message"];
            if message.is_null() {
                continue;
            }

            let user_id = match message["from"]["id"].as_i64() {
                Some(id) => id,
                None => continue,
            };

            let chat_id = match message["chat"]["id"].as_i64() {
                Some(id) => id,
                None => continue,
            };

            // 安全檢查：若未設定允許的用戶，則自動將第一個發送訊息的人加入白名單
            if allowed_ids.is_empty() {
                println!(
                    "[TelegramBot] 偵測到第一個權限請求 User ID: {} (Chat ID: {})",
                    user_id, chat_id
                );
                let state = app.state::<AppState>();
                if let Ok(mut settings) = crate::settings::store::get_settings(&state.db).await {
                    if !settings.telegram.allowed_user_ids.contains(&user_id) {
                        settings.telegram.allowed_user_ids.push(user_id);
                        if let Err(e) =
                            crate::settings::store::save_settings(&state.db, settings).await
                        {
                            eprintln!("[TelegramBot] 自動加入 User ID 失敗: {}", e);
                        } else {
                            allowed_ids.push(user_id);
                            let token_clone = bot_token.clone();
                            tokio::spawn(async move {
                                let _ = send_message(&token_clone, chat_id, "👋 您好！這是您第一次使用。系統已自動將您加入白名單，現在您可以開始發送訊息了。").await;
                            });
                        }
                    }
                }
            } else if !allowed_ids.contains(&user_id) {
                println!(
                    "[TelegramBot] 未授權的訪問 User ID: {} (Chat ID: {})",
                    user_id, chat_id
                );
                continue;
            }

            let text = message["text"].as_str().unwrap_or("").to_string();
            let caption = message["caption"].as_str().unwrap_or("").to_string();
            let photo_arr = message["photo"].as_array();
            let has_photo = photo_arr.map(|a| !a.is_empty()).unwrap_or(false);
            let document = message["document"].clone();
            let has_document = !document.is_null();

            if text.is_empty() && !has_photo && !has_document {
                continue;
            }

            let app_clone = app.clone();
            let token = bot_token.clone();

            if has_photo {
                let photos = message["photo"].as_array().unwrap();
                let largest = &photos[photos.len() - 1];
                let file_id = largest["file_id"].as_str().unwrap_or("").to_string();
                let label = if !caption.is_empty() {
                    caption.clone()
                } else {
                    text.clone()
                };

                tokio::spawn(async move {
                    if let Err(e) =
                        handle_photo_capture(&app_clone, &token, chat_id, &file_id, &label).await
                    {
                        eprintln!("[TelegramBot] 圖片處理失敗: {}", e);
                        let _ =
                            send_message(&token, chat_id, &format!("❌ 圖片處理失敗：{}", e)).await;
                    }
                });
            } else if has_document {
                let caption_clone = if !caption.is_empty() {
                    caption.clone()
                } else {
                    text.clone()
                };

                tokio::spawn(async move {
                    if let Err(e) = handle_document_capture(
                        &app_clone,
                        &token,
                        chat_id,
                        &document,
                        &caption_clone,
                    )
                    .await
                    {
                        eprintln!("[TelegramBot] 文件處理失敗: {}", e);
                        let _ =
                            send_message(&token, chat_id, &format!("❌ 文件處理失敗：{}", e)).await;
                    }
                });
            } else {
                let text_clone = text.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_message(&app_clone, &token, chat_id, &text_clone).await {
                        eprintln!("[TelegramBot] 訊息處理失敗: {}", e);
                        let _ = send_message(&token, chat_id, &format!("❌ 處理失敗：{}", e)).await;
                    }
                });
            }
        }

        if updates.is_empty() {
            sleep(Duration::from_millis(500)).await;
        }
    }
}

/// 處理傳入訊息：指令、URL 擷取或 RAG 查詢
async fn handle_message(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    text: &str,
) -> Result<(), String> {
    // 指令處理
    if text.starts_with('/') {
        return handle_command(app, bot_token, chat_id, text).await;
    }

    // URL 擷取範疇：偵測 http(s):// 並進行網頁快照擷取
    if let Some(url) = extract_url(text) {
        return handle_url_capture(app, bot_token, chat_id, &url).await;
    }

    // RAG 查詢
    handle_rag_query(app, bot_token, chat_id, text).await
}

/// 從字串中提取第一個 http/https URL
fn extract_url(text: &str) -> Option<String> {
    for word in text.split_whitespace() {
        if word.starts_with("http://") || word.starts_with("https://") {
            // 移除尾部標點
            let url = word.trim_end_matches(|c| matches!(c, '.' | ',' | ')' | ']' | '>'));
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }
    }
    None
}

/// 根據 URL 判斷類型標籤
fn classify_url_label(url: &str) -> &'static str {
    let lower = url.to_lowercase();
    if lower.contains("youtube.com/watch") || lower.contains("youtu.be/") {
        "YouTube 影片"
    } else if lower.contains("bilibili.com/video") || lower.contains("b23.tv") {
        "B站影片"
    } else {
        "網頁"
    }
}

/// 將 URL 擷取請求存入 inbox
async fn handle_url_capture(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    url: &str,
) -> Result<(), String> {
    let label = classify_url_label(url);
    send_message(
        bot_token,
        chat_id,
        &format!("⏳ 偵測到 {} 連結，排程擷取中...", label),
    )
    .await?;

    let state = app.state::<AppState>();
    let pool = &state.db;

    // 檢查是否已存在
    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM sources WHERE url = ? AND type = 'url' LIMIT 1")
            .bind(url)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return send_message(
            bot_token,
            chat_id,
            &format!("💡 該 {} 連結已在知識庫中，跳過重複擷取。", label),
        )
        .await;
    }

    let inbox_id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO inbox (id, content, content_type, source_url, window_title, session_id, status, captured_at) \
         VALUES (?, ?, 'url', ?, ?, '', 'pending', ?)"
    )
    .bind(&inbox_id)
    .bind(url)
    .bind(url)
    .bind(url)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("寫入 inbox 失敗: {}", e))?;

    send_message(
        bot_token,
        chat_id,
        &format!(
            "✅ {} 擷取任務已建立，完成後可使用 /recent 查看結果。",
            label
        ),
    )
    .await
}

/// 處理 Bot 指令
async fn handle_command(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    text: &str,
) -> Result<(), String> {
    let cmd = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .split('@')
        .next()
        .unwrap_or("");

    match cmd {
        "/start" => {
            send_message(bot_token, chat_id,
                "👋 歡迎使用 InsightCAP 助理！\n\n\
                 您可以直接輸入問題，我會根據您的知識庫回答。\n\
                 或傳送網頁連結、照片與文檔來充實知識庫。\n\n\
                 可用指令：\n\
                 /new - 開啟新對話\n\
                 /list - 檢視對話清單\n\
                 /rename <名稱> - 重命名當前對話\n\
                 /project - 檢視專案清單\n\
                 /newproject <名稱> - 建立新專案\n\
                 /reminders - 檢視活動提醒\n\
                 /status - 伺服器狀態\n\
                 /recent - 最近擷取內容"
            ).await
        }
        "/new" => handle_new_conversation(app, bot_token, chat_id).await,
        "/list" => handle_list_conversations(app, bot_token, chat_id).await,
        "/rename" => {
            let new_name = text.strip_prefix("/rename").unwrap_or("").trim();
            handle_rename_conversation(app, bot_token, chat_id, new_name).await
        }
        "/project" => handle_list_projects(app, bot_token, chat_id).await,
        "/newproject" => {
            let name = text.strip_prefix("/newproject").unwrap_or("").trim();
            handle_new_project(app, bot_token, chat_id, name).await
        }
        "/status" => handle_status(app, bot_token, chat_id).await,
        "/recent" => handle_recent(app, bot_token, chat_id).await,
        "/reminders" => handle_list_reminders(app, bot_token, chat_id).await,
        _ => {
            send_message(bot_token, chat_id,
                "❓ 未知的指令。可用指令：/new /list /rename /project /newproject /reminders /status /recent"
            ).await
        }
    }
}

/// 處理照片擷取並存入 inbox 待 OCR 處理
async fn handle_photo_capture(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    file_id: &str,
    caption: &str,
) -> Result<(), String> {
    send_message(
        bot_token,
        chat_id,
        "📸 已收到照片，正在下載並排程 OCR 解析...",
    )
    .await?;

    let client = Client::new();

    // 1. 獲取 file_path
    let get_file_url = format!(
        "https://api.telegram.org/bot{}/getFile?file_id={}",
        bot_token, file_id
    );
    let resp = client
        .get(&get_file_url)
        .send()
        .await
        .map_err(|e| format!("getFile 失敗: {}", e))?;
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;

    if body["ok"].as_bool() != Some(true) {
        return Err(format!("getFile API 錯誤: {}", body));
    }

    let file_path = body["result"]["file_path"]
        .as_str()
        .ok_or_else(|| "無法獲取 file_path".to_string())?
        .to_string();

    // 2. 下載照片
    let download_url = format!(
        "https://api.telegram.org/file/bot{}/{}",
        bot_token, file_path
    );
    let image_bytes: Vec<u8> = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("下載照片失敗: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("解析照片數據失敗: {}", e))?
        .to_vec();

    // 3. 存入 inbox
    let state = app.state::<AppState>();
    let pool = &state.db;

    let inbox_id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let title: String = if caption.is_empty() {
        "Telegram 擷取圖片".to_string()
    } else {
        caption.chars().take(80).collect()
    };

    sqlx::query(
        "INSERT INTO inbox (id, content, content_type, source_exe, source_url, window_title, image_data, session_id, status, captured_at) \
         VALUES (?, '', 'image', 'telegram', '', ?, ?, '', 'pending', ?)"
    )
    .bind(&inbox_id)
    .bind(&title)
    .bind(image_bytes)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("寫入 inbox 失敗: {}", e))?;

    send_message(
        bot_token,
        chat_id,
        "✅ 照片已存入，OCR 解析完成後可使用 /recent 查看。",
    )
    .await
}

/// 處理文檔擷取（PDF, DOCX, etc.）
async fn handle_document_capture(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    document: &Value,
    caption: &str,
) -> Result<(), String> {
    let file_name = document["file_name"]
        .as_str()
        .unwrap_or("document")
        .to_string();
    let mime_type = document["mime_type"].as_str().unwrap_or("").to_string();
    let file_id = document["file_id"]
        .as_str()
        .ok_or_else(|| "遺失 file_id".to_string())?
        .to_string();
    let file_size = document["file_size"].as_i64().unwrap_or(0);

    // Telegram Bot API 限制下載 20MB
    if file_size > 20 * 1024 * 1024 {
        return send_message(
            bot_token,
            chat_id,
            "⚠️ 文件過大 (超過 20MB)，Bot 模式無法下載，請手動上傳。",
        )
        .await;
    }

    let ext = file_name.rsplit('.').next().unwrap_or("").to_lowercase();
    let is_image = mime_type.starts_with("image/")
        || matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif");

    let is_supported_doc = matches!(
        ext.as_str(),
        "txt"
            | "log"
            | "md"
            | "pdf"
            | "docx"
            | "xlsx"
            | "pptx"
            | "csv"
            | "rtf"
            | "epub"
            | "html"
            | "htm"
            | "py"
            | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "rs"
            | "go"
            | "java"
            | "cpp"
            | "c"
            | "h"
            | "rb"
            | "php"
    );

    if !is_image && !is_supported_doc {
        return send_message(
            bot_token,
            chat_id,
            &format!(
                "❌ 不支援的檔案格式: {}\n\
                 目前僅支援 PDF, DOCX, Office 文檔、純文字與常見程式碼檔案。",
                ext
            ),
        )
        .await;
    }

    let label = if is_image {
        "🖼️ 圖片文檔"
    } else {
        "📄 文檔"
    };
    send_message(
        bot_token,
        chat_id,
        &format!("⏳ 偵測到{}：{}，下載中...", label, file_name),
    )
    .await?;

    let client = Client::new();
    let get_file_url = format!(
        "https://api.telegram.org/bot{}/getFile?file_id={}",
        bot_token, file_id
    );
    let resp = client
        .get(&get_file_url)
        .send()
        .await
        .map_err(|e| format!("getFile 失敗: {}", e))?;
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;

    if body["ok"].as_bool() != Some(true) {
        return Err(format!("getFile API 錯誤: {}", body));
    }

    let remote_path = body["result"]["file_path"]
        .as_str()
        .ok_or_else(|| "無法獲取檔案路徑".to_string())?
        .to_string();

    let download_url = format!(
        "https://api.telegram.org/file/bot{}/{}",
        bot_token, remote_path
    );
    let file_bytes: Vec<u8> = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("下載失敗: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("數據解析失敗: {}", e))?
        .to_vec();

    let state = app.state::<AppState>();
    let pool = &state.db;
    let inbox_id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let title = if !caption.is_empty() {
        format!(
            "{} ({})",
            caption.chars().take(60).collect::<String>(),
            file_name
        )
    } else {
        file_name.clone()
    };

    if is_image {
        sqlx::query(
            "INSERT INTO inbox (id, content, content_type, source_exe, source_url, window_title, image_data, session_id, status, captured_at) \
             VALUES (?, '', 'image', 'telegram', '', ?, ?, '', 'pending', ?)"
        )
        .bind(&inbox_id)
        .bind(&title)
        .bind(file_bytes)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("寫入 inbox 失敗: {}", e))?;

        send_message(bot_token, chat_id, "✅ 圖片已存入並排程解析。").await
    } else {
        let temp_path = std::env::temp_dir().join(format!("tg_{}_{}", inbox_id, file_name));
        std::fs::write(&temp_path, &file_bytes).map_err(|e| format!("寫入臨時檔案失敗: {}", e))?;

        let settings = crate::settings::store::get_settings(pool)
            .await
            .map_err(|e| e.to_string())?;
        let vision_config = crate::providers::llm::vision::VisionConfig::from_settings(
            &settings.ai_models.vision_model,
        );

        let parse_result = crate::capture::file_parser::parse_file(
            &settings.knowledge.kb_path,
            temp_path.to_str().unwrap_or(""),
            vision_config.as_ref(),
        )
        .await;

        let _ = std::fs::remove_file(&temp_path);

        let extracted_text = parse_result
            .map(|doc| {
                doc.chunks
                    .iter()
                    .map(|c| c.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n")
            })
            .map_err(|e| format!("解析文檔 {} 失敗: {}", file_name, e))?;

        if extracted_text.trim().is_empty() {
            return send_message(
                bot_token,
                chat_id,
                &format!("⚠️ 文件 {} 解析後內容為空，跳過擷取。", file_name),
            )
            .await;
        }

        sqlx::query(
            "INSERT INTO inbox (id, content, content_type, source_exe, source_url, window_title, image_data, session_id, status, captured_at) \
             VALUES (?, ?, 'clipboard', 'telegram', '', ?, NULL, '', 'pending', ?)"
        )
        .bind(&inbox_id)
        .bind(&extracted_text)
        .bind(&title)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| format!("寫入 inbox 失敗: {}", e))?;

        send_message(
            bot_token,
            chat_id,
            &format!("✅ 文檔 {} 內容已提取並存入知識庫。", file_name),
        )
        .await
    }
}

/// /new 開啟新對話並自動設置上下文
async fn handle_new_conversation(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?, '未命名對話', ?, ?)"
    )
    .bind(&id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 紀錄此 chat 的當前對話
    set_telegram_context(pool, chat_id, &id).await?;

    send_message(
        bot_token,
        chat_id,
        "🆕 已為您開啟新對話，現在可以直接輸入問題囉！",
    )
    .await
}

/// /list 顯示最近對話列表（Inline Keyboard）
async fn handle_list_conversations(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    let rows = sqlx::query(
        "SELECT id, title, updated_at FROM conversations \
         WHERE is_archived = 0 ORDER BY updated_at DESC LIMIT 10",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return send_message(
            bot_token,
            chat_id,
            "📭 目前沒有活躍對話。輸入 /new 開啟新對話。",
        )
        .await;
    }

    let current_conv = get_telegram_context(pool, chat_id)
        .await
        .unwrap_or_default();

    // 構造 Inline Keyboard：左側選擇，右側封存
    let mut keyboard: Vec<Value> = Vec::new();
    for row in &rows {
        let id: String = row.get("id");
        let title: String = row
            .try_get("title")
            .unwrap_or_else(|_| "未命名".to_string());
        let marker = if id == current_conv { " ✅" } else { "" };
        keyboard.push(json!([
            {
                "text": format!("{}{}", title, marker),
                "callback_data": format!("conv_{}", id),
            },
            {
                "text": "📁 封存",
                "callback_data": format!("arch_{}", id),
            },
        ]));
    }

    let client = Client::new();
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let _ = client
        .post(&url)
        .json(&json!({
            "chat_id": chat_id,
            "text": "💬 請選擇對話或將其封存：",
            "reply_markup": {
                "inline_keyboard": keyboard,
            },
        }))
        .send()
        .await
        .map_err(|e| format!("sendMessage 失敗: {}", e))?;

    Ok(())
}

/// 處理 Inline Keyboard 回傳
async fn handle_callback(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    data: &str,
    keyboard_msg_id: Option<i64>,
) -> Result<(), String> {
    if let Some(conv_id) = data.strip_prefix("conv_") {
        let state = app.state::<AppState>();
        let pool = &state.db;

        let title: Option<String> =
            sqlx::query_scalar("SELECT title FROM conversations WHERE id = ?")
                .bind(conv_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

        match title {
            Some(t) => {
                set_telegram_context(pool, chat_id, conv_id).await?;
                let reply = format!("📌 已切換至對話：{}", t);
                if let Some(msg_id) = keyboard_msg_id {
                    edit_message_text(bot_token, chat_id, msg_id, &reply).await
                } else {
                    send_message(bot_token, chat_id, &reply).await
                }
            }
            None => send_message(bot_token, chat_id, "❌ 該對話已不存在。").await,
        }
    } else if let Some(conv_id) = data.strip_prefix("arch_") {
        let state = app.state::<AppState>();
        let pool = &state.db;

        let title: Option<String> =
            sqlx::query_scalar("SELECT title FROM conversations WHERE id = ? AND is_archived = 0")
                .bind(conv_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

        let Some(t) = title else {
            return send_message(bot_token, chat_id, "⚠️ 該對話已封存或不存在。").await;
        };

        // 封存對話
        sqlx::query("UPDATE conversations SET is_archived = 1 WHERE id = ?")
            .bind(conv_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        // 若封存的是當前使用的對話，清除或切換上下文
        let current = get_telegram_context(pool, chat_id)
            .await
            .unwrap_or_default();
        if current == conv_id {
            let next: Option<String> = sqlx::query_scalar(
                "SELECT id FROM conversations WHERE is_archived = 0 ORDER BY updated_at DESC LIMIT 1"
            )
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

            if let Some(next_id) = next {
                set_telegram_context(pool, chat_id, &next_id).await?;
            } else {
                set_telegram_context(pool, chat_id, "").await?;
            }
        }

        let reply = format!("📁 已封存對話：{}", t);
        if let Some(msg_id) = keyboard_msg_id {
            edit_message_text(bot_token, chat_id, msg_id, &reply).await
        } else {
            send_message(bot_token, chat_id, &reply).await
        }
    } else if let Some(proj_id) = data.strip_prefix("proj_") {
        let state = app.state::<AppState>();
        let pool = &state.db;

        let name: Option<String> =
            sqlx::query_scalar("SELECT name FROM projects WHERE id = ? AND is_archived = 0")
                .bind(proj_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

        let Some(n) = name else {
            return send_message(bot_token, chat_id, "⚠️ 該專案不存在。").await;
        };

        set_telegram_project_context(pool, chat_id, proj_id).await?;

        let reply = format!("📂 已切換至專案：{}", n);
        if let Some(msg_id) = keyboard_msg_id {
            edit_message_text(bot_token, chat_id, msg_id, &reply).await
        } else {
            send_message(bot_token, chat_id, &reply).await
        }
    } else if data == "new_project" {
        let reply = "請輸入 /newproject <名稱> 來建立專案，例如：\n/newproject 學習筆記";
        if let Some(msg_id) = keyboard_msg_id {
            edit_message_text(bot_token, chat_id, msg_id, reply).await
        } else {
            send_message(bot_token, chat_id, reply).await
        }
    } else if let Some(rmd_id) = data.strip_prefix("rmd_done_") {
        let state = app.state::<AppState>();
        let engine = crate::services::reminder_engine::ReminderEngine::new(state.db.clone());
        engine.update_reminder_status(rmd_id, "completed").await?;
        let reply = "✅ 提醒事項已標記為完成。";
        if let Some(msg_id) = keyboard_msg_id {
            edit_message_text(bot_token, chat_id, msg_id, reply).await
        } else {
            send_message(bot_token, chat_id, reply).await
        }
    } else if let Some(rmd_id) = data.strip_prefix("rmd_cancel_") {
        let state = app.state::<AppState>();
        let engine = crate::services::reminder_engine::ReminderEngine::new(state.db.clone());
        engine.update_reminder_status(rmd_id, "dismissed").await?;
        let reply = "📁 提醒事項已忽略。";
        if let Some(msg_id) = keyboard_msg_id {
            edit_message_text(bot_token, chat_id, msg_id, reply).await
        } else {
            send_message(bot_token, chat_id, reply).await
        }
    } else {
        Ok(())
    }
}

/// /reminders 顯示活動中的提醒事項
async fn handle_list_reminders(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let engine = crate::services::reminder_engine::ReminderEngine::new(state.db.clone());
    let reminders: Vec<Value> = engine.get_active_reminders().await?;

    if reminders.is_empty() {
        return send_message(bot_token, chat_id, "📭 目前沒有活動中的提醒。").await;
    }

    send_message(bot_token, chat_id, "🔔 以下是您的活躍提醒清單：").await?;

    for item in reminders {
        let id = item["id"].as_str().unwrap_or("");
        let title = item["title"].as_str().unwrap_or("未命名內容");
        let date = item["eventDate"].as_str().unwrap_or("未定日期");
        let time = item["eventTime"].as_str().unwrap_or("");
        let time_display = if time.is_empty() {
            date.to_string()
        } else {
            format!("{} {}", date, time)
        };
        let event_type = match item["eventType"].as_str().unwrap_or("") {
            "meeting" => "📅 會議",
            "deliverable" => "📦 交付物",
            "event" => "🕒 活動",
            "appointment" => "🏥 預約",
            _ => "ℹ️ 提醒",
        };

        let msg = format!("{}\n📌 {}\n⏰ {}", event_type, title, time_display);

        let keyboard = json!([
            [
                { "text": "✅ 已完成", "callback_data": format!("rmd_done_{}", id) },
                { "text": "📁 忽略", "callback_data": format!("rmd_cancel_{}", id) }
            ]
        ]);

        let client = Client::new();
        let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
        let _ = client
            .post(&url)
            .json(&json!({
                "chat_id": chat_id,
                "text": msg,
                "reply_markup": {
                    "inline_keyboard": keyboard,
                },
            }))
            .send()
            .await
            .map_err(|e| format!("sendMessage 失敗: {}", e))?;
    }

    Ok(())
}

/// 編輯訊息文本（用於動態更新或 Inline Keyboard 回應）
async fn edit_message_text(
    bot_token: &str,
    chat_id: i64,
    message_id: i64,
    text: &str,
) -> Result<(), String> {
    let client = Client::new();
    let url = format!("https://api.telegram.org/bot{}/editMessageText", bot_token);
    let resp = client
        .post(&url)
        .json(&json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": text,
            "parse_mode": "Markdown",
        }))
        .send()
        .await
        .map_err(|e| format!("editMessageText 失敗: {}", e))?;

    // 若 Markdown 解析失敗則嘗試純文本
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    if body["ok"].as_bool() != Some(true) {
        let desc = body["description"].as_str().unwrap_or("");
        if desc.contains("parse") || desc.contains("markdown") {
            let _ = client
                .post(&url)
                .json(&json!({
                    "chat_id": chat_id,
                    "message_id": message_id,
                    "text": text,
                }))
                .send()
                .await;
        }
    }
    Ok(())
}

/// 發送「輸入中...」狀態
async fn send_chat_action(bot_token: &str, chat_id: i64) {
    let client = Client::new();
    let url = format!("https://api.telegram.org/bot{}/sendChatAction", bot_token);
    let _ = client
        .post(&url)
        .json(&json!({
            "chat_id": chat_id,
            "action": "typing",
        }))
        .send()
        .await;
}

/// 回應 callback_query
async fn answer_callback_query(bot_token: &str, callback_query_id: &str) -> Result<(), String> {
    let client = Client::new();
    let url = format!(
        "https://api.telegram.org/bot{}/answerCallbackQuery",
        bot_token
    );
    let _ = client
        .post(&url)
        .json(&json!({ "callback_query_id": callback_query_id }))
        .send()
        .await;
    Ok(())
}

/// /status 顯示伺服器狀態
async fn handle_status(app: &AppHandle, bot_token: &str, chat_id: i64) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    // 數據統計
    let capture_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM captures")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let source_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sources")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let conv_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE is_archived = 0")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let chunk_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_chunks")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    // 資料庫大小
    let settings = crate::settings::store::get_settings(pool)
        .await
        .map_err(|e| e.to_string())?;
    let kb_size = dir_size_mb(&settings.knowledge.kb_path);

    let msg = format!(
        "🖥️ InsightCAP 伺服器狀態\n\n\
         運行狀態：運作中\n\
         知識庫大小：{:.1} MB\n\
         資料來源 (Sources)：{}\n\
         擷取記錄 (Captures)：{}\n\
         記憶區塊 (Memory Chunks)：{}\n\
         活躍對話數：{}",
        kb_size, source_count, capture_count, chunk_count, conv_count
    );

    send_message(bot_token, chat_id, &msg).await
}

/// /rename 重命名當前對話
async fn handle_rename_conversation(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    new_name: &str,
) -> Result<(), String> {
    if new_name.is_empty() {
        return send_message(bot_token, chat_id, "⚠️ 請輸入新的名稱：/rename <名稱>").await;
    }

    let state = app.state::<AppState>();
    let pool = &state.db;

    let conv_id = get_telegram_context(pool, chat_id)
        .await
        .unwrap_or_default();
    if conv_id.is_empty() {
        return send_message(
            bot_token,
            chat_id,
            "⚠️ 當前沒有選定的對話，請使用 /new 建立新對話。",
        )
        .await;
    }

    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE conversations SET title = ?, updated_at = ? WHERE id = ?")
        .bind(new_name)
        .bind(&now)
        .bind(&conv_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    send_message(
        bot_token,
        chat_id,
        &format!("✅ 對話已重命名為：{}", new_name),
    )
    .await
}

/// /recent 查看最近擷取的內容
async fn handle_recent(app: &AppHandle, bot_token: &str, chat_id: i64) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    let rows = sqlx::query(
        "SELECT title, type, captured_at FROM sources \
         ORDER BY captured_at DESC LIMIT 10",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return send_message(bot_token, chat_id, "📭 目前沒有擷取記錄。").await;
    }

    let mut msg = String::from("⏳ 最近擷取的內容：\n\n");
    for row in &rows {
        let title: String = row
            .try_get("title")
            .unwrap_or_else(|_| "未命名".to_string());
        let source_type: String = row.try_get("type").unwrap_or_else(|_| "text".to_string());
        let captured_at: String = row.get("captured_at");
        let type_emoji = match source_type.as_str() {
            "url" => "🌐",
            "file" | "pdf" => "📄",
            "image" | "screenshot" => "🖼️",
            "editor" => "📝",
            _ => "ℹ️",
        };
        msg.push_str(&format!(
            "{} {} @ {}\n",
            type_emoji,
            title,
            &captured_at[..10]
        ));
    }

    send_message(bot_token, chat_id, &msg).await
}

/// /project 顯示專案列表（Inline Keyboard）
async fn handle_list_projects(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    let rows = sqlx::query(
        "SELECT id, name FROM projects WHERE is_archived = 0 ORDER BY updated_at DESC LIMIT 15",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let current_proj = get_telegram_project_context(pool, chat_id)
        .await
        .unwrap_or_default();

    let mut keyboard: Vec<Value> = Vec::new();
    for row in &rows {
        let id: String = row.get("id");
        let name: String = row.try_get("name").unwrap_or_else(|_| "未命名".to_string());
        let marker = if id == current_proj { " ✅" } else { "" };
        keyboard.push(json!([
            {
                "text": format!("{}{}", name, marker),
                "callback_data": format!("proj_{}", id),
            },
        ]));
    }

    // 底部：建立新專案按鈕
    keyboard.push(json!([{
        "text": "➕ 建立新專案",
        "callback_data": "new_project",
    }]));

    let text = if rows.is_empty() {
        "📭 目前沒有活躍專案。輸入 /newproject <名稱> 建立第一個專案。"
    } else {
        "📁 請選擇當前工作專案："
    };

    let client = Client::new();
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let _ = client
        .post(&url)
        .json(&json!({
            "chat_id": chat_id,
            "text": text,
            "reply_markup": { "inline_keyboard": keyboard },
        }))
        .send()
        .await
        .map_err(|e| format!("sendMessage 失敗: {}", e))?;

    Ok(())
}

/// /newproject <名稱> 建立新專案
async fn handle_new_project(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    name: &str,
) -> Result<(), String> {
    if name.is_empty() {
        return send_message(
            bot_token,
            chat_id,
            "⚠️ 請輸入專案名稱，例如：/newproject 學習筆記",
        )
        .await;
    }

    let state = app.state::<AppState>();
    let pool = &state.db;

    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let default_tags = "[]";

    // 確保 color 欄位存在
    let _ = sqlx::query("ALTER TABLE projects ADD COLUMN color TEXT")
        .execute(pool)
        .await;

    sqlx::query(
        "INSERT INTO projects (id, name, default_tags, color, created_at, updated_at) VALUES (?, ?, ?, NULL, ?, ?)"
    )
    .bind(&id)
    .bind(name)
    .bind(default_tags)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 自動切換至新建立的專案
    set_telegram_project_context(pool, chat_id, &id).await?;

    send_message(
        bot_token,
        chat_id,
        &format!("✅ 專案「{}」已建立，並已自動切換為當前工作專案。", name),
    )
    .await
}

/// RAG 查詢處理
async fn handle_rag_query(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    query: &str,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    // 獲取當前對話上下文
    let conv_id_raw = get_telegram_context(pool, chat_id)
        .await
        .unwrap_or_default();
    let mut conv_id = conv_id_raw.clone();

    // 檢查對話是否存在
    if !conv_id.is_empty() {
        let exists: Option<String> =
            sqlx::query_scalar("SELECT id FROM conversations WHERE id = ?")
                .bind(&conv_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

        if exists.is_none() {
            conv_id = "".to_string();
        }
    }

    // 若無對話上下文，自動建立一個
    let conv_id = if conv_id.is_empty() {
        let new_id = Uuid::now_v7().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?, '未命名對話', ?, ?)"
        )
        .bind(&new_id)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        set_telegram_context(pool, chat_id, &new_id).await?;
        new_id
    } else {
        conv_id
    };

    // 顯示「正在輸入中...」
    send_chat_action(bot_token, chat_id).await;

    // 獲取對話標題
    let conv_title: String = sqlx::query_scalar("SELECT title FROM conversations WHERE id = ?")
        .bind(&conv_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "對話".to_string());

    // 獲取最近對話歷史
    let history = get_conversation_history(pool, &conv_id, 10).await?;

    // 獲取對話摘要
    let summary: Option<String> =
        sqlx::query_scalar("SELECT summary FROM conversations WHERE id = ?")
            .bind(&conv_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .and_then(|s: String| if s.is_empty() { None } else { Some(s) });

    // 獲取當前專案設定
    let project_id = get_telegram_project_context(pool, chat_id)
        .await
        .unwrap_or_default();
    let project_id_opt = if project_id.is_empty() {
        None
    } else {
        Some(project_id.clone())
    };

    // 獲取設定
    let settings = crate::settings::store::get_settings(pool)
        .await
        .map_err(|e| e.to_string())?;

    // 構造 RAG prompt
    let engine = RagEngine::new(
        pool.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );

    let telegram_override = settings
        .telegram
        .prompt_instruction_override
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned();

    let (base_prompt, history_vec, _citation_sources, _context_hints) = engine
        .build_prompt(
            query,
            history,
            summary.clone(),
            project_id_opt.clone(),
            None,
            None,
            true,
            None,
            telegram_override.clone(),
        )
        .await?;

    let cfg = settings.ai_models.chat_llm;
    let api_key = cfg.api_key.clone().unwrap_or_default();
    let is_ollama = cfg.provider == "ollama";

    if api_key.is_empty() && !is_ollama {
        return send_message(
            bot_token,
            chat_id,
            "⚠️ 未設定 AI 模型，請於主程式設定 AI 連結。",
        )
        .await;
    }

    let llm = OpenAiProvider::new(api_key, cfg.base_url.clone(), cfg.model, cfg.provider);
    let use_streaming = settings.telegram.streaming == "partial";

    let prefix = format!("[{}] 🤖\n", conv_title);
    let answer = if use_streaming {
        let token_str = bot_token.to_string();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        const DRAFT_ID: i64 = 1;
        let initial_draft = format!("{}{}", prefix, "⏳ 生成中...");
        let draft_message_id = send_message_draft(&token_str, chat_id, DRAFT_ID, &initial_draft)
            .await
            .ok();

        let bot_token_draft = token_str.clone();
        let prefix_draft = prefix.clone();
        let has_draft = draft_message_id.is_some();
        let draft_task = tokio::spawn(async move {
            let mut accumulated = prefix_draft;
            let mut last_sent = std::time::Instant::now();
            let mut overflow = false;

            while let Some(chunk) = rx.recv().await {
                if overflow {
                    continue;
                }
                accumulated.push_str(&chunk);

                if accumulated.len() > TELEGRAM_MSG_LIMIT {
                    overflow = true;
                    if has_draft {
                        if let Some(mid) = draft_message_id {
                            let _ = edit_message_text(
                                &bot_token_draft,
                                chat_id,
                                mid,
                                "⚠️ 訊息過長，改以分段訊息發送。",
                            )
                            .await;
                        }
                    }
                    continue;
                }

                if has_draft && last_sent.elapsed().as_millis() as u64 >= DRAFT_THROTTLE_MS {
                    if let Some(mid) = draft_message_id {
                        let _ =
                            edit_message_text(&bot_token_draft, chat_id, mid, &accumulated).await;
                    }
                    last_sent = std::time::Instant::now();
                }
            }

            if has_draft && !overflow {
                if let Some(mid) = draft_message_id {
                    let _ = edit_message_text(&bot_token_draft, chat_id, mid, &accumulated).await;
                }
            }
            overflow
        });

        let stream_result = llm
            .complete_stream(
                &base_prompt,
                &history_vec,
                query,
                LLMOptions {
                    think_mode: Some(false),
                    ..LLMOptions::default()
                },
                move |token| {
                    if let StreamToken::Content(c) = token {
                        let _ = tx.send(c);
                    }
                },
            )
            .await
            .map_err(|e| e.to_string())?;

        let overflow = draft_task.await.unwrap_or(true);
        let final_reply = format!("{}{}", prefix, stream_result.content);
        if overflow || draft_message_id.is_none() {
            send_long_message(bot_token, chat_id, &final_reply).await?;
        } else if let Some(mid) = draft_message_id {
            let _ = edit_message_text(bot_token, chat_id, mid, &final_reply).await;
        }
        stream_result.content
    } else {
        // 非串流模式
        let engine2 = RagEngine::new(
            pool.clone(),
            state.vector_store.clone(),
            state.embedder.clone(),
        );
        let result = engine2
            .generate_answer(
                query,
                history_vec
                    .iter()
                    .map(|(r, c)| (r.clone(), c.clone()))
                    .collect(),
                summary,
                project_id_opt,
                None,
                None,
                true,
                None,
                false,
                None,
                telegram_override.clone(),
            )
            .await?;
        let answer = result["answer"]
            .as_str()
            .unwrap_or("無法獲得有效回答")
            .to_string();
        let final_reply = format!("{}{}", prefix, answer);
        send_long_message(bot_token, chat_id, &final_reply).await?;
        answer
    };

    // 存入對話歷史紀錄
    save_message(pool, &conv_id, "user", query).await?;
    save_message(pool, &conv_id, "assistant", &answer).await?;

    Ok(())
}

// --- Telegram API 封裝工具 ---

/// 發送簡訊
pub async fn send_message(bot_token: &str, chat_id: i64, text: &str) -> Result<(), String> {
    send_long_message(bot_token, chat_id, text).await
}

/// 發送草稿訊息並回傳 message_id，後續可用 editMessageText 更新同一條訊息
async fn send_message_draft(
    bot_token: &str,
    chat_id: i64,
    _draft_id: i64,
    text: &str,
) -> Result<i64, String> {
    let client = Client::new();
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let resp = client
        .post(&url)
        .json(&json!({
            "chat_id": chat_id,
            "text": text,
        }))
        .send()
        .await
        .map_err(|e| format!("send_message_draft 失敗: {}", e))?;

    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    if body["ok"].as_bool() != Some(true) {
        return Err(format!(
            "send_message_draft API 錯誤: {}",
            body["description"].as_str().unwrap_or("unknown")
        ));
    }
    body["result"]["message_id"]
        .as_i64()
        .ok_or_else(|| "send_message_draft 未回傳 message_id".to_string())
}

/// 發送長訊息（自動分段處理）
async fn send_long_message(bot_token: &str, chat_id: i64, text: &str) -> Result<(), String> {
    let client = Client::new();
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);

    // 以 4096 字符為限切分
    let chunks = split_message(text, TELEGRAM_MSG_LIMIT);

    for chunk in &chunks {
        let resp = client
            .post(&url)
            .json(&json!({
                "chat_id": chat_id,
                "text": chunk,
                "parse_mode": "Markdown",
            }))
            .send()
            .await
            .map_err(|e| format!("sendMessage 失敗: {}", e))?;

        // 若 Markdown 解析失敗，改用純文本重發
        let body: Value = resp.json().await.map_err(|e| e.to_string())?;
        if body["ok"].as_bool() != Some(true) {
            let desc = body["description"].as_str().unwrap_or("");
            eprintln!("[TelegramBot] sendMessage 失敗 ({}): {}", chat_id, desc);

            if desc.contains("parse") || desc.contains("markdown") {
                println!("[TelegramBot] 嘗試以純文本模式重新發送至 {}...", chat_id);
                let resp2 = client
                    .post(&url)
                    .json(&json!({
                        "chat_id": chat_id,
                        "text": chunk,
                    }))
                    .send()
                    .await
                    .map_err(|e| format!("純文本重發失敗: {}", e))?;

                let body2: Value = resp2.json().await.map_err(|e| e.to_string())?;
                if body2["ok"].as_bool() != Some(true) {
                    return Err(format!(
                        "Telegram API 錯誤: {}",
                        body2["description"].as_str().unwrap_or("unknown")
                    ));
                }
            } else {
                return Err(format!("Telegram API 錯誤: {}", desc));
            }
        }

        // 多段發送時加入微小延遲
        if chunks.len() > 1 {
            sleep(Duration::from_millis(300)).await;
        }
    }

    Ok(())
}

/// 將長訊息切分為可接受的區塊
fn split_message(text: &str, limit: usize) -> Vec<String> {
    if text.len() <= limit {
        return vec![text.to_string()];
    }

    let mut parts = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= limit {
            parts.push(remaining.to_string());
            break;
        }

        // 嘗試在換行處切斷
        let split_at = remaining[..limit].rfind('\n').unwrap_or(limit);

        let (chunk, rest) = remaining.split_at(split_at);
        parts.push(chunk.to_string());
        remaining = rest.trim_start_matches('\n');
    }

    parts
}

// --- 對話上下文管理功能 ---

/// 確保 telegram_state 表存在
async fn ensure_telegram_table(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS telegram_state (\
         chat_id INTEGER PRIMARY KEY, \
         current_conversation_id TEXT NOT NULL, \
         updated_at TEXT NOT NULL)",
    )
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 若 current_project_id 欄位不存在則新增
    let _ = sqlx::query(
        "ALTER TABLE telegram_state ADD COLUMN current_project_id TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;

    Ok(())
}

/// 設置特定 chat 的當前對話上下文
async fn set_telegram_context(
    pool: &SqlitePool,
    chat_id: i64,
    conversation_id: &str,
) -> Result<(), String> {
    ensure_telegram_table(pool).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO telegram_state (chat_id, current_conversation_id, updated_at) \
         VALUES (?, ?, ?) \
         ON CONFLICT(chat_id) DO UPDATE SET current_conversation_id = excluded.current_conversation_id, updated_at = excluded.updated_at"
    )
    .bind(chat_id)
    .bind(conversation_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 獲取特定 chat 的當前對話 ID
async fn get_telegram_context(pool: &SqlitePool, chat_id: i64) -> Result<String, String> {
    ensure_telegram_table(pool).await?;
    let row: Option<String> =
        sqlx::query_scalar("SELECT current_conversation_id FROM telegram_state WHERE chat_id = ?")
            .bind(chat_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(row.unwrap_or_default())
}

/// 設置特定 chat 的當前專案上下文
async fn set_telegram_project_context(
    pool: &SqlitePool,
    chat_id: i64,
    project_id: &str,
) -> Result<(), String> {
    ensure_telegram_table(pool).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO telegram_state (chat_id, current_conversation_id, current_project_id, updated_at) \
         VALUES (?, '', ?, ?) \
         ON CONFLICT(chat_id) DO UPDATE SET current_project_id = excluded.current_project_id, updated_at = excluded.updated_at"
    )
    .bind(chat_id)
    .bind(project_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 獲取特定 chat 的當前專案 ID
async fn get_telegram_project_context(pool: &SqlitePool, chat_id: i64) -> Result<String, String> {
    ensure_telegram_table(pool).await?;
    let row: Option<String> =
        sqlx::query_scalar("SELECT current_project_id FROM telegram_state WHERE chat_id = ?")
            .bind(chat_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(row.unwrap_or_default())
}

// --- DB 數據操作輔助功能 ---

/// 獲取對話歷史
async fn get_conversation_history(
    pool: &SqlitePool,
    conversation_id: &str,
    limit: i32,
) -> Result<Vec<(String, String)>, String> {
    let rows = sqlx::query(
        "SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY created_at DESC LIMIT ?"
    )
    .bind(conversation_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 順序調整回正向
    let mut history: Vec<(String, String)> = rows
        .into_iter()
        .map(|r| (r.get::<String, _>("role"), r.get::<String, _>("content")))
        .collect();
    history.reverse();

    Ok(history)
}

/// 儲存對話中的單筆訊息
async fn save_message(
    pool: &SqlitePool,
    conversation_id: &str,
    role: &str,
    content: &str,
) -> Result<(), String> {
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, content, metadata, created_at) VALUES (?, ?, ?, ?, '{}', ?)"
    )
    .bind(&id)
    .bind(conversation_id)
    .bind(role)
    .bind(content)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "UPDATE conversations SET updated_at = ?, message_count = message_count + 1 WHERE id = ?",
    )
    .bind(&now)
    .bind(conversation_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 計算目錄大小 (MB)
fn dir_size_mb(path: &str) -> f64 {
    let path = std::path::Path::new(path);
    if !path.exists() {
        return 0.0;
    }
    let mut total: u64 = 0;
    if let Ok(entries) = walkdir::WalkDir::new(path)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
    {
        for entry in entries {
            if entry.file_type().is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total as f64 / (1024.0 * 1024.0)
}
