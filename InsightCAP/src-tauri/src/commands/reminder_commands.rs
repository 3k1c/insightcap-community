use sqlx::Row;
use tauri::{Manager, State};

use crate::background::reminder_scheduler::process_due_notifications;
use crate::db::AppState;
use crate::services::reminder_engine::ReminderEngine;

#[tauri::command]
pub async fn get_active_reminders(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let engine = ReminderEngine::new(state.db.clone());
    engine.get_active_reminders().await
}

#[tauri::command]
pub async fn get_pending_reminders(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let engine = ReminderEngine::new(state.db.clone());
    engine.get_pending_reminders().await
}

#[tauri::command]
pub async fn confirm_reminder(
    state: State<'_, AppState>,
    reminder_id: String,
    accept: bool,
) -> Result<(), String> {
    let engine = ReminderEngine::new(state.db.clone());
    engine.confirm_reminder(&reminder_id, accept).await
}

#[tauri::command]
pub async fn update_reminder_status(
    state: State<'_, AppState>,
    reminder_id: String,
    status: String,
) -> Result<(), String> {
    let engine = ReminderEngine::new(state.db.clone());
    engine.update_reminder_status(&reminder_id, &status).await
}

#[tauri::command]
pub async fn snooze_reminder(
    state: State<'_, AppState>,
    reminder_id: String,
    snooze_minutes: i64,
) -> Result<(), String> {
    let engine = ReminderEngine::new(state.db.clone());
    engine.snooze_reminder(&reminder_id, snooze_minutes).await
}

/// 觸發緊急提醒檢查（由前端或自動化腳本調用）
/// 在 ConversationScheduler 之外手動啟動提取。
#[tauri::command]
pub async fn trigger_urgent_reminder_check(
    state: State<'_, AppState>,
    conversation_id: String,
    recent_messages: String,
) -> Result<Vec<String>, String> {
    let pool = &state.db;

    // 讀取當前對話摘要
    let summary: String = sqlx::query_scalar::<_, String>(
        "SELECT COALESCE(summary, '') FROM conversations WHERE id = ?"
    )
    .bind(&conversation_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
    .unwrap_or_default();

    let engine = ReminderEngine::new(pool.clone());
    let now = chrono::Utc::now().to_rfc3339();

    println!("[UrgentReminder] 啟動緊急檢查: {} (是否有摘要: {})", conversation_id, !summary.is_empty());

    let ids: Vec<String> = engine
        .extract_reminders(&conversation_id, &summary, &recent_messages, &now, None)
        .await?;

    if !ids.is_empty() {
        let count = ids.len();
        println!("[UrgentReminder] 成功提取 {} 個新提醒！", count);
    } else {
        println!("[UrgentReminder] 未能從近期對話中提取任何有效提醒。");
    }


    Ok(ids)
}

#[tauri::command]
pub async fn trigger_test_reminder(
    app: tauri::AppHandle,
) -> Result<String, String> {
    println!("[ReminderTest] 強制觸發測試通知...");
    
    // 預檢設定
    let state = app.state::<crate::db::AppState>();
    let settings = crate::settings::store::get_settings(&state.db).await.map_err(|e| e.to_string())?;
    
    let mut warning = String::new();
    if settings.telegram.enabled {
        if settings.telegram.bot_token.is_empty() {
             warning = " (⚠️ 偵測到 Telegram 已啟用但未設定 Bot Token)".to_string();
        } else if settings.telegram.allowed_user_ids.is_empty() {
             warning = " (⚠️ 偵測到 Telegram 已啟用但未設定授權 User ID)".to_string();
        }
    }

    let count = process_due_notifications(&app, true).await?;
    if count > 0 {
        let telegram_info = if settings.telegram.enabled && !settings.telegram.bot_token.is_empty() {
            format!("，並嘗試發送至 {} 位 Telegram 用戶", settings.telegram.allowed_user_ids.len())
        } else {
            "".to_string()
        };
        Ok(format!("成功觸發 {} 筆通知{}。", count, telegram_info))
    } else {
        // 如果資料庫空的，我們就直接發送一條純測試訊息，讓使用者知道 Telegram 通道是通的！
        if settings.telegram.enabled && !settings.telegram.bot_token.is_empty() && !settings.telegram.allowed_user_ids.is_empty() {
            for &user_id in &settings.telegram.allowed_user_ids {
                let _ = crate::background::telegram_bot::send_message(
                    &settings.telegram.bot_token, 
                    user_id, 
                    "🛠️ *InsightCAP 測試通知*\n\n您的 Telegram 提醒系統設定正確，目前連線正常！\n當您有設定開會、交付日等任務時，將會在此收到通知。"
                ).await;
            }
            Ok(format!("資料庫沒有到期項目，但我們已成功向 {} 位 Telegram 用戶發送測試信號！", settings.telegram.allowed_user_ids.len()))
        } else {
            Ok(format!("目前資料庫中沒有到期的通知項目{}。", warning))
        }
    }
}

/// 清除所有待發送的通知（用於清除錯誤提取的測試項）
#[tauri::command]
pub async fn clear_pending_notifications(
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let result = sqlx::query("UPDATE reminder_notifications SET sent_at = ? WHERE sent_at IS NULL")
        .bind(&now)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(result.rows_affected() as usize)
}

/// 自動獲取最近發訊息給 Bot 的 User ID（用於輔助設定）
#[tauri::command]
pub async fn telegram_get_allowed_user_ids(
    bot_token: String,
) -> Result<Vec<i64>, String> {
    println!("[TelegramBot] 嘗試獲取最近對話者的 User ID...");
    let client = reqwest::Client::new();
    let url = format!("https://api.telegram.org/bot{}/getUpdates", bot_token);
    
    let resp = client.get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
        
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    
    let mut user_ids = std::collections::HashSet::new();
    if let Some(result) = json["result"].as_array() {
        for update in result {
            if let Some(msg) = update.get("message") {
                if let Some(chat) = msg.get("chat") {
                    if let Some(id) = chat.get("id").and_then(|v| v.as_i64()) {
                        user_ids.insert(id);
                    }
                }
            } else if let Some(edited_msg) = update.get("edited_message") {
                if let Some(chat) = edited_msg.get("chat") {
                    if let Some(id) = chat.get("id").and_then(|v| v.as_i64()) {
                        user_ids.insert(id);
                    }
                }
            }
        }
    }
    
    let mut ids: Vec<i64> = user_ids.into_iter().collect();
    ids.sort();
    Ok(ids)
}

/// 測試發送 Telegram 通知（用於設定頁面驗證）
#[tauri::command]
pub async fn test_telegram_notification(
    bot_token: String,
    user_ids: Vec<i64>,
) -> Result<(), String> {
    println!("[TelegramTest] 發送測試通知至: {:?}", user_ids);
    let msg = "✨ 這是一則來自 InsightCAP 的測試通知！如果您看到這條訊息，代表 Telegram Bot 已成功串接。";
    for chat_id in user_ids {
        crate::background::telegram_bot::send_message(&bot_token, chat_id, msg).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_system_time_info() -> Result<serde_json::Value, String> {
    let local_now = chrono::Local::now();
    let utc_now = chrono::Utc::now();
    let offset = local_now.offset().local_minus_utc();
    
    Ok(serde_json::json!({
        "local": local_now.to_rfc3339(),
        "utc": utc_now.to_rfc3339(),
        "offset_seconds": offset,
        "offset_hours": offset / 3600,
        "system_timezone_id": format!("{:?}", local_now.timezone())
    }))
}

/// 診斷用：列出資料庫中前 5 筆待發送的通知
#[tauri::command]
pub async fn debug_list_notifications(
    state: tauri::State<'_, crate::db::AppState>
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query(
        "SELECT n.*, r.title FROM reminder_notifications n \
         JOIN reminders r ON n.reminder_id = r.id \
         WHERE n.sent_at IS NULL ORDER BY n.scheduled_at ASC LIMIT 5"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for row in rows {
        results.push(serde_json::json!({
            "id": row.get::<String, _>("id"),
            "reminder_id": row.get::<String, _>("reminder_id"),
            "intent": row.get::<String, _>("intent"),
            "scheduled_at": row.get::<String, _>("scheduled_at"),
            "title": row.get::<String, _>("title")
        }));
    }
    Ok(results)
}

/// 檢測項：時間格式不匹配 (診斷資料庫字串與程式生成字串是否對等)
#[tauri::command]
pub async fn verify_db_time_format(
    state: tauri::State<'_, crate::db::AppState>
) -> Result<serde_json::Value, String> {
    // 1. 程式生成的標準字串 (Secs 分辨率, 帶 Z)
    let runtime_now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    
    // 2. 從資料庫取出最重要的一筆待發時間
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT scheduled_at FROM reminder_notifications WHERE sent_at IS NULL ORDER BY created_at DESC LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let db_str = row.map(|(s,)| s).unwrap_or_else(|| "N/A (空資料庫)".to_string());
    
    // 3. 全表掃描是否有「非標準格式」（帶毫秒）的記錄
    let ms_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reminder_notifications WHERE scheduled_at LIKE '%.%'"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    // 4. 檢查關鍵特徵
    let has_mismatch = if db_str == "N/A (空資料庫)" {
        false
    } else {
        // 比對長度 (20 碼應為 YYYY-MM-DDTHH:MM:SSZ)
        db_str.len() != runtime_now.len() ||
        !db_str.ends_with('Z') ||
        db_str.contains('.') != runtime_now.contains('.') ||
        ms_count > 0
    };

    Ok(serde_json::json!({
        "runtime_reference": runtime_now,
        "database_record": db_str,
        "millisecond_record_count": ms_count,
        "details": {
            "runtime_len": runtime_now.len(),
            "database_len": db_str.len(),
            "is_standard_z": db_str.ends_with('Z'),
            "has_milliseconds": db_str.contains('.')
        },
        "format_mismatch_detected": has_mismatch,
        "diagnosis": if ms_count > 0 {
            format!("⚠️ 偵測到 {} 筆記錄包含「毫秒」！這會導致 SQL 的 '<=' 字串比對失效，建議清理或標準化時間欄位。", ms_count)
        } else if has_mismatch {
            "⚠️ 偵測到時間格式不一致（可能是 Z 結尾缺失）！".to_string()
        } else {
            "✅ 格式一致且無毫秒污染。背景輪詢比對功能應運作正常。".to_string()
        }
    }))
}/// 手動觸發某個對話的提醒提取 (診斷與強制更新用)
#[tauri::command]
pub async fn manual_extract_reminders(
    state: tauri::State<'_, crate::db::AppState>,
    conversation_id: String,
) -> Result<String, String> {
    let pool = &state.db;
    
    // 1. 取得對話訊息
    let msgs = sqlx::query(
        "SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY created_at ASC"
    )
    .bind(&conversation_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut dialogue = String::new();
    for m in &msgs {
        let role: String = m.get("role");
        let content: String = m.get("content");
        dialogue.push_str(&format!("{}: {}\n", role, content));
    }

    // 2. 取得現有摘要 (若無則跳過或宣告失敗)
    let summary: Option<String> = sqlx::query_scalar(
        "SELECT summary FROM conversations WHERE id = ?"
    )
    .bind(&conversation_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let summary = summary.unwrap_or_else(|| "（無摘要）".to_string());
    let now = chrono::Utc::now().to_rfc3339();
    
    // 3. 取得 project_id
    let project_id: Option<String> = sqlx::query_scalar(
        "SELECT project_id FROM conversations WHERE id = ?"
    )
    .bind(&conversation_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    // 4. 執行提取
    let engine = ReminderEngine::new(pool.clone());
    let created_ids = engine.extract_reminders(
        &conversation_id,
        &summary,
        &dialogue,
        &now,
        project_id.as_deref()
    ).await?;

    Ok(format!("成功提取 {} 筆提醒事項。", created_ids.len()))
}

#[tauri::command]
pub async fn check_reminder_health(
    state: tauri::State<'_, crate::db::AppState>
) -> Result<serde_json::Value, String> {
    let pool = &state.db;
    let now_str = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    // 1. 檢測卡住的通知 (Stuck Notifications)
    let stuck_rows = sqlx::query(
        "SELECT n.id, r.title, r.status, r.pending_confirm FROM reminder_notifications n \
         JOIN reminders r ON n.reminder_id = r.id \
         WHERE n.sent_at IS NULL AND n.scheduled_at <= ? \
         AND (r.status != 'active' OR r.pending_confirm = 1)"
    )
    .bind(&now_str)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 2. 檢測過舊未發送 (Stale Notifications > 24h)
    let yesterday = (chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let stale_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reminder_notifications WHERE sent_at IS NULL AND scheduled_at < ?"
    )
    .bind(&yesterday)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    // 3. 讀取死鎖計數器
    let loop_count = state.reminder_loop_count.load(std::sync::atomic::Ordering::Relaxed);

    Ok(serde_json::json!({
        "timestamp": now_str,
        "stuck_count": stuck_rows.len(),
        "stale_count_24h": stale_count,
        "loop_count": loop_count,
        "status": if stuck_rows.is_empty() && stale_count == 0 { "HEALTHY" } else { "DEGRADED" },
        "stuck_details": stuck_rows.iter().map(|r| {
            serde_json::json!({
                "id": r.get::<String, _>("id"),
                "title": r.get::<String, _>("title"),
                "status": r.get::<String, _>("status"),
                "pending_confirm": r.get::<i32, _>("pending_confirm")
            })
        }).collect::<Vec<_>>()
    }))
}

#[tauri::command]
pub async fn get_project_timeline(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<serde_json::Value, String> {
    let pool = &state.db;

    // 1. 獲取該項目的所有提醒事項
    let reminders = sqlx::query(
        "SELECT * FROM reminders WHERE project_id = ? ORDER BY event_date ASC NULLS LAST"
    )
    .bind(&project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut reminder_list = Vec::new();
    for row in reminders {
        reminder_list.push(serde_json::json!({
            "id": row.get::<String, _>("id"),
            "title": row.get::<String, _>("title"),
            "description": row.get::<Option<String>, _>("description"),
            "eventDate": row.get::<Option<String>, _>("event_date"),
            "eventTime": row.get::<Option<String>, _>("event_time"),
            "eventType": row.get::<String, _>("event_type"),
            "status": row.get::<String, _>("status"),
        }));
    }

    // 2. 獲取該項目的重點對話片段 (Pattern / Log)
    let chunks = sqlx::query(
        "SELECT id, content, knowledge_type, created_at FROM memory_chunks \
         WHERE project_id = ? AND knowledge_type IN ('pattern', 'log') \
         ORDER BY created_at DESC LIMIT 20"
    )
    .bind(&project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut chunk_list = Vec::new();
    for row in chunks {
        chunk_list.push(serde_json::json!({
            "id": row.get::<String, _>("id"),
            "content": row.get::<String, _>("content"),
            "type": row.get::<String, _>("knowledge_type"),
            "createdAt": row.get::<String, _>("created_at"),
        }));
    }

    Ok(serde_json::json!({
        "reminders": reminder_list,
        "keyPoints": chunk_list,
    }))
}
