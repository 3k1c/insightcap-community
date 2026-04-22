
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
const DRAFT_THROTTLE_MS: u64 = 300;

static POLLING_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn start_telegram_bot(app: AppHandle) {
    if POLLING_ACTIVE.swap(true, Ordering::SeqCst) {
        println!("[TelegramBot] Already running, skipping duplicate start");
        return;
    }
    tauri::async_runtime::spawn(async move {
        println!("[TelegramBot] Worker started, waiting for settings...");
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
        let settings = {
            let state = app.state::<AppState>();
            match crate::settings::store::get_settings(&state.db).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[TelegramBot]       : {}", e);
                    sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
                    continue;
                }
            }
        };

        let tg = &settings.telegram;

        if !tg.enabled || tg.bot_token.trim().is_empty() {
            sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
            continue;
        }

        let bot_token = tg.bot_token.trim().to_string();
        let mut allowed_ids = tg.allowed_user_ids.clone();

        let url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout={}",
            bot_token, offset, POLL_TIMEOUT_SECS
        );

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[TelegramBot] getUpdates   : {}", e);
                sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
                continue;
            }
        };

        let body: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[TelegramBot]       : {}", e);
                sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
                continue;
            }
        };

        if body["ok"].as_bool() != Some(true) {
            let error_code = body["error_code"].as_i64().unwrap_or(0);
            if error_code == 409 {
                eprintln!("[TelegramBot] 409 Conflict:       Bot       60     ");
                sleep(Duration::from_secs(60)).await;
            } else {
                eprintln!("[TelegramBot] API   : {}", body);
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
            if let Some(uid) = update["update_id"].as_i64() {
                offset = uid + 1;
            }

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

                let _ = answer_callback_query(&bot_token, &cb_id).await;
                if let Err(e) =
                    handle_callback(&app, &bot_token, cb_chat_id, &cb_data, cb_msg_id).await
                {
                    eprintln!("[TelegramBot] callback     : {}", e);
                    let _ = send_message(&bot_token, cb_chat_id, &format!("        {}", e)).await;
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

            if allowed_ids.is_empty() {
                println!(
                    "[TelegramBot]            User ID: {} (Chat ID: {})",
                    user_id, chat_id
                );
                let state = app.state::<AppState>();
                if let Ok(mut settings) = crate::settings::store::get_settings(&state.db).await {
                    if !settings.telegram.allowed_user_ids.contains(&user_id) {
                        settings.telegram.allowed_user_ids.push(user_id);
                        if let Err(e) =
                            crate::settings::store::save_settings(&state.db, settings).await
                        {
                            eprintln!("[TelegramBot]      User ID   : {}", e);
                        } else {
                            allowed_ids.push(user_id);
                            let token_clone = bot_token.clone();
                            tokio::spawn(async move {
                                let _ = send_message(
                                    &token_clone,
                                    chat_id,
                                    "Telegram access enabled for this user.",
                                )
                                .await;
                            });
                        }
                    }
                }
            } else if !allowed_ids.contains(&user_id) {
                println!(
                    "[TelegramBot]        User ID: {} (Chat ID: {})",
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
                        eprintln!("[TelegramBot]       : {}", e);
                        let _ =
                            send_message(&token, chat_id, &format!("Photo capture failed: {}", e))
                                .await;
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
                        eprintln!("[TelegramBot]       : {}", e);
                        let _ = send_message(
                            &token,
                            chat_id,
                            &format!("Document capture failed: {}", e),
                        )
                        .await;
                    }
                });
            } else {
                let text_clone = text.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_message(&app_clone, &token, chat_id, &text_clone).await {
                        eprintln!("[TelegramBot]       : {}", e);
                        let _ =
                            send_message(&token, chat_id, &format!("Request failed: {}", e)).await;
                    }
                });
            }
        }

        if updates.is_empty() {
            sleep(Duration::from_millis(500)).await;
        }
    }
}

async fn handle_message(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    text: &str,
) -> Result<(), String> {
    if text.starts_with('/') {
        return handle_command(app, bot_token, chat_id, text).await;
    }

    if let Some(url) = extract_url(text) {
        return handle_url_capture(app, bot_token, chat_id, &url).await;
    }

    handle_rag_query(app, bot_token, chat_id, text).await
}

fn extract_url(text: &str) -> Option<String> {
    for word in text.split_whitespace() {
        if word.starts_with("http://") || word.starts_with("https://") {
            let url = word.trim_end_matches(|c| matches!(c, '.' | ',' | ')' | ']' | '>'));
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }
    }
    None
}

fn classify_url_label(url: &str) -> &'static str {
    let lower = url.to_lowercase();
    if lower.contains("youtube.com/watch") || lower.contains("youtu.be/") {
        "YouTube"
    } else if lower.contains("bilibili.com/video") || lower.contains("b23.tv") {
        "Bilibili"
    } else {
        "Web"
    }
}

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
        &format!("Received {label} link. Parsing and queuing..."),
    )
    .await?;

    let state = app.state::<AppState>();
    let pool = &state.db;

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
            &format!("{label} link already exists in repository."),
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
    .map_err(|e| format!("   inbox   : {}", e))?;

    send_message(
        bot_token,
        chat_id,
        &format!("{label} link queued successfully. Use /recent to view latest captures."),
    )
    .await
}

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
            send_message(
                bot_token,
                chat_id,
                "        InsightCAP    \n\n\
                                       \n\
                                     \n\n\
                      \n\
                 /new -      \n\
                 /list -       \n\
                 /rename <  > -        \n\
                 /project -       \n\
                 /newproject <  > -      \n\
                 /reminders -       \n\
                 /status -      \n\
                 /recent -       ",
            )
            .await
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
            send_message(
                bot_token,
                chat_id,
                "             /new /list /rename /project /newproject /reminders /status /recent",
            )
            .await
        }
    }
}

async fn handle_photo_capture(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    file_id: &str,
    caption: &str,
) -> Result<(), String> {
    send_message(bot_token, chat_id, "                 OCR   ...").await?;

    let client = Client::new();

    let get_file_url = format!(
        "https://api.telegram.org/bot{}/getFile?file_id={}",
        bot_token, file_id
    );
    let resp = client
        .get(&get_file_url)
        .send()
        .await
        .map_err(|e| format!("getFile   : {}", e))?;
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;

    if body["ok"].as_bool() != Some(true) {
        return Err(format!("getFile API   : {}", body));
    }

    let file_path = body["result"]["file_path"]
        .as_str()
        .ok_or_else(|| "     file_path".to_string())?
        .to_string();

    let download_url = format!(
        "https://api.telegram.org/file/bot{}/{}",
        bot_token, file_path
    );
    let image_bytes: Vec<u8> = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("      : {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("        : {}", e))?
        .to_vec();

    let state = app.state::<AppState>();
    let pool = &state.db;

    let inbox_id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let title: String = if caption.is_empty() {
        "Telegram     ".to_string()
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
    .map_err(|e| format!("   inbox   : {}", e))?;

    send_message(bot_token, chat_id, "        OCR          /recent    ").await
}

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
        .ok_or_else(|| "   file_id".to_string())?
        .to_string();
    let file_size = document["file_size"].as_i64().unwrap_or(0);

    if file_size > 20 * 1024 * 1024 {
        return send_message(bot_token, chat_id, "        (   20MB) Bot              ").await;
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
                "          : {}\n\
                       PDF, DOCX, Office                ",
                ext
            ),
        )
        .await;
    }

    let label = if is_image { "image" } else { "document" };
    send_message(
        bot_token,
        chat_id,
        &format!("     {} {}    ...", label, file_name),
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
        .map_err(|e| format!("getFile   : {}", e))?;
    let body: Value = resp.json().await.map_err(|e| e.to_string())?;

    if body["ok"].as_bool() != Some(true) {
        return Err(format!("getFile API   : {}", body));
    }

    let remote_path = body["result"]["file_path"]
        .as_str()
        .ok_or_else(|| "Missing file_path in Telegram API response".to_string())?
        .to_string();

    let download_url = format!(
        "https://api.telegram.org/file/bot{}/{}",
        bot_token, remote_path
    );
    let file_bytes: Vec<u8> = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("    : {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("      : {}", e))?
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
        .map_err(|e| format!("   inbox   : {}", e))?;

        send_message(bot_token, chat_id, "Image has been queued for processing.").await
    } else {
        let temp_path = std::env::temp_dir().join(format!("tg_{}_{}", inbox_id, file_name));
        std::fs::write(&temp_path, &file_bytes).map_err(|e| format!("        : {}", e))?;

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
            .map_err(|e| format!("     {}   : {}", file_name, e))?;

        if extracted_text.trim().is_empty() {
            return send_message(
                bot_token,
                chat_id,
                &format!("      {}              ", file_name),
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
        .map_err(|e| format!("   inbox   : {}", e))?;

        send_message(
            bot_token,
            chat_id,
            &format!("     {}             ", file_name),
        )
        .await
    }
}

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
        "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?, 'Untitled Conversation', ?, ?)",
    )
    .bind(&id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    set_telegram_context(pool, chat_id, &id).await?;

    send_message(
        bot_token,
        chat_id,
        "Started a new conversation and set it as current.",
    )
    .await
}

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
        return send_message(bot_token, chat_id, "               /new       ").await;
    }

    let current_conv = get_telegram_context(pool, chat_id)
        .await
        .unwrap_or_default();

    let mut keyboard: Vec<Value> = Vec::new();
    for row in &rows {
        let id: String = row.get("id");
        let title: String = row
            .try_get("title")
            .unwrap_or_else(|_| "Untitled Conversation".to_string());
        let marker = if id == current_conv { " (current)" } else { "" };
        keyboard.push(json!([
            {
                "text": format!("{}{}", title, marker),
                "callback_data": format!("conv_{}", id),
            },
            {
                "text": "Archive",
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
            "text": "Select a conversation:",
            "reply_markup": {
                "inline_keyboard": keyboard,
            },
        }))
        .send()
        .await
        .map_err(|e| format!("sendMessage   : {}", e))?;

    Ok(())
}

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
                let reply = format!("          {}", t);
                if let Some(msg_id) = keyboard_msg_id {
                    edit_message_text(bot_token, chat_id, msg_id, &reply).await
                } else {
                    send_message(bot_token, chat_id, &reply).await
                }
            }
            None => send_message(bot_token, chat_id, "Conversation not found.").await,
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
            return send_message(
                bot_token,
                chat_id,
                "Conversation already archived or not found.",
            )
            .await;
        };

        sqlx::query("UPDATE conversations SET is_archived = 1 WHERE id = ?")
            .bind(conv_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

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

        let reply = format!("Archived conversation: {}", t);
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
            return send_message(bot_token, chat_id, "Project not found.").await;
        };

        set_telegram_project_context(pool, chat_id, proj_id).await?;

        let reply = format!("Current project: {}", n);
        if let Some(msg_id) = keyboard_msg_id {
            edit_message_text(bot_token, chat_id, msg_id, &reply).await
        } else {
            send_message(bot_token, chat_id, &reply).await
        }
    } else if data == "new_project" {
        let reply =
            "Use /newproject <name> to create a project.\nExample: /newproject InsightCAP MVP";
        if let Some(msg_id) = keyboard_msg_id {
            edit_message_text(bot_token, chat_id, msg_id, reply).await
        } else {
            send_message(bot_token, chat_id, reply).await
        }
    } else if let Some(rmd_id) = data.strip_prefix("rmd_done_") {
        let state = app.state::<AppState>();
        let engine = crate::services::reminder_engine::ReminderEngine::new(state.db.clone());
        engine.update_reminder_status(rmd_id, "completed").await?;
        let reply = "Reminder marked as completed.";
        if let Some(msg_id) = keyboard_msg_id {
            edit_message_text(bot_token, chat_id, msg_id, reply).await
        } else {
            send_message(bot_token, chat_id, reply).await
        }
    } else if let Some(rmd_id) = data.strip_prefix("rmd_cancel_") {
        let state = app.state::<AppState>();
        let engine = crate::services::reminder_engine::ReminderEngine::new(state.db.clone());
        engine.update_reminder_status(rmd_id, "dismissed").await?;
        let reply = "Reminder dismissed.";
        if let Some(msg_id) = keyboard_msg_id {
            edit_message_text(bot_token, chat_id, msg_id, reply).await
        } else {
            send_message(bot_token, chat_id, reply).await
        }
    } else {
        Ok(())
    }
}

async fn handle_list_reminders(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let engine = crate::services::reminder_engine::ReminderEngine::new(state.db.clone());
    let reminders: Vec<Value> = engine.get_active_reminders().await?;

    if reminders.is_empty() {
        return send_message(bot_token, chat_id, "No active reminders.").await;
    }

    send_message(bot_token, chat_id, "Active reminders:").await?;

    for item in reminders {
        let id = item["id"].as_str().unwrap_or("");
        let title = item["title"].as_str().unwrap_or("Untitled reminder");
        let date = item["eventDate"].as_str().unwrap_or("No date");
        let time = item["eventTime"].as_str().unwrap_or("");
        let time_display = if time.is_empty() {
            date.to_string()
        } else {
            format!("{} {}", date, time)
        };
        let event_type = match item["eventType"].as_str().unwrap_or("") {
            "meeting" => "[Meeting]",
            "deliverable" => "[Deliverable]",
            "event" => "[Event]",
            "appointment" => "[Appointment]",
            _ => "[Reminder]",
        };

        let msg = format!("{}\n   {}\n  {}", event_type, title, time_display);

        let keyboard = json!([
            [
                { "text": "Done", "callback_data": format!("rmd_done_{}", id) },
                { "text": "Dismiss", "callback_data": format!("rmd_cancel_{}", id) }
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
            .map_err(|e| format!("sendMessage   : {}", e))?;
    }

    Ok(())
}

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
        .map_err(|e| format!("editMessageText   : {}", e))?;

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

async fn handle_status(app: &AppHandle, bot_token: &str, chat_id: i64) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

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

    let settings = crate::settings::store::get_settings(pool)
        .await
        .map_err(|e| e.to_string())?;
    let kb_size = dir_size_mb(&settings.knowledge.kb_path);

    let msg = format!(
        "    InsightCAP      \n\n\
                 \n\
               {:.1} MB\n\
              (Sources) {}\n\
              (Captures) {}\n\
              (Memory Chunks) {}\n\
               {}",
        kb_size, source_count, capture_count, chunk_count, conv_count
    );

    send_message(bot_token, chat_id, &msg).await
}

async fn handle_rename_conversation(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    new_name: &str,
) -> Result<(), String> {
    if new_name.is_empty() {
        return send_message(bot_token, chat_id, "           /rename <  >").await;
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
            "No active conversation. Use /new to create one.",
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
        &format!("Conversation renamed to: {}", new_name),
    )
    .await
}

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
        return send_message(bot_token, chat_id, "No captured sources yet.").await;
    }

    let mut msg = String::from("Recent captures:\n\n");
    for row in &rows {
        let title: String = row
            .try_get("title")
            .unwrap_or_else(|_| "Untitled Source".to_string());
        let source_type: String = row.try_get("type").unwrap_or_else(|_| "text".to_string());
        let captured_at: String = row.get("captured_at");
        let type_emoji = match source_type.as_str() {
            "url" => "[URL]",
            "file" | "pdf" => "[FILE]",
            "image" | "screenshot" => "[IMAGE]",
            "editor" => "[NOTE]",
            _ => "[ITEM]",
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
        let name: String = row
            .try_get("name")
            .unwrap_or_else(|_| "Untitled Project".to_string());
        let marker = if id == current_proj { " (current)" } else { "" };
        keyboard.push(json!([
            {
                "text": format!("{}{}", name, marker),
                "callback_data": format!("proj_{}", id),
            },
        ]));
    }

    keyboard.push(json!([{
        "text": "Create New Project",
        "callback_data": "new_project",
    }]));

    let text = if rows.is_empty() {
        "No projects yet. Use /newproject <name> to create one."
    } else {
        "Select a project:"
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
        .map_err(|e| format!("sendMessage   : {}", e))?;

    Ok(())
}

async fn handle_new_project(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    name: &str,
) -> Result<(), String> {
    if name.is_empty() {
        return send_message(bot_token, chat_id, "Usage: /newproject <name>").await;
    }

    let state = app.state::<AppState>();
    let pool = &state.db;

    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let default_tags = "[]";

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

    set_telegram_project_context(pool, chat_id, &id).await?;

    send_message(
        bot_token,
        chat_id,
        &format!("     {}                   ", name),
    )
    .await
}

async fn handle_rag_query(
    app: &AppHandle,
    bot_token: &str,
    chat_id: i64,
    query: &str,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    let conv_id_raw = get_telegram_context(pool, chat_id)
        .await
        .unwrap_or_default();
    let mut conv_id = conv_id_raw.clone();

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

    let conv_id = if conv_id.is_empty() {
        let new_id = Uuid::now_v7().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?, 'Untitled Conversation', ?, ?)"
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

    send_chat_action(bot_token, chat_id).await;

    let conv_title: String = sqlx::query_scalar("SELECT title FROM conversations WHERE id = ?")
        .bind(&conv_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "Untitled Conversation".to_string());

    let history = get_conversation_history(pool, &conv_id, 10).await?;

    let summary: Option<String> =
        sqlx::query_scalar("SELECT summary FROM conversations WHERE id = ?")
            .bind(&conv_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .and_then(|s: String| if s.is_empty() { None } else { Some(s) });

    let project_id = get_telegram_project_context(pool, chat_id)
        .await
        .unwrap_or_default();
    let project_id_opt = if project_id.is_empty() {
        None
    } else {
        Some(project_id.clone())
    };

    let settings = crate::settings::store::get_settings(pool)
        .await
        .map_err(|e| e.to_string())?;

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
            "AI is not configured. Please set your provider and API key in Settings.",
        )
        .await;
    }

    let llm = OpenAiProvider::new(api_key, cfg.base_url.clone(), cfg.model, cfg.provider);
    let use_streaming = settings.telegram.streaming == "partial";

    let prefix = format!("[{}]   \n", conv_title);
    let answer = if use_streaming {
        let token_str = bot_token.to_string();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        const DRAFT_ID: i64 = 1;
        let initial_draft = format!("{}{}", prefix, "Thinking...");
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
                                "Response is too long for draft mode. Sending full message...",
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
            .unwrap_or("No answer generated.")
            .to_string();
        let final_reply = format!("{}{}", prefix, answer);
        send_long_message(bot_token, chat_id, &final_reply).await?;
        answer
    };

    save_message(pool, &conv_id, "user", query).await?;
    save_message(pool, &conv_id, "assistant", &answer).await?;

    Ok(())
}


pub async fn send_message(bot_token: &str, chat_id: i64, text: &str) -> Result<(), String> {
    send_long_message(bot_token, chat_id, text).await
}

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
        .map_err(|e| format!("send_message_draft   : {}", e))?;

    let body: Value = resp.json().await.map_err(|e| e.to_string())?;
    if body["ok"].as_bool() != Some(true) {
        return Err(format!(
            "send_message_draft API   : {}",
            body["description"].as_str().unwrap_or("unknown")
        ));
    }
    body["result"]["message_id"]
        .as_i64()
        .ok_or_else(|| "send_message_draft     message_id".to_string())
}

async fn send_long_message(bot_token: &str, chat_id: i64, text: &str) -> Result<(), String> {
    let client = Client::new();
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);

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
            .map_err(|e| format!("sendMessage   : {}", e))?;

        let body: Value = resp.json().await.map_err(|e| e.to_string())?;
        if body["ok"].as_bool() != Some(true) {
            let desc = body["description"].as_str().unwrap_or("");
            eprintln!("[TelegramBot] sendMessage    ({}): {}", chat_id, desc);

            if desc.contains("parse") || desc.contains("markdown") {
                println!("[TelegramBot]               {}...", chat_id);
                let resp2 = client
                    .post(&url)
                    .json(&json!({
                        "chat_id": chat_id,
                        "text": chunk,
                    }))
                    .send()
                    .await
                    .map_err(|e| format!("       : {}", e))?;

                let body2: Value = resp2.json().await.map_err(|e| e.to_string())?;
                if body2["ok"].as_bool() != Some(true) {
                    return Err(format!(
                        "Telegram API   : {}",
                        body2["description"].as_str().unwrap_or("unknown")
                    ));
                }
            } else {
                return Err(format!("Telegram API   : {}", desc));
            }
        }

        if chunks.len() > 1 {
            sleep(Duration::from_millis(300)).await;
        }
    }

    Ok(())
}

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

        let split_at = remaining[..limit].rfind('\n').unwrap_or(limit);

        let (chunk, rest) = remaining.split_at(split_at);
        parts.push(chunk.to_string());
        remaining = rest.trim_start_matches('\n');
    }

    parts
}


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

    let _ = sqlx::query(
        "ALTER TABLE telegram_state ADD COLUMN current_project_id TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await;

    Ok(())
}

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

    let mut history: Vec<(String, String)> = rows
        .into_iter()
        .map(|r| (r.get::<String, _>("role"), r.get::<String, _>("content")))
        .collect();
    history.reverse();

    Ok(history)
}

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
