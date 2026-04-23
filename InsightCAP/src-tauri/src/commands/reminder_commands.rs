use sqlx::Row;
use tauri::{Manager, State};

use crate::background::reminder_scheduler::process_due_notifications;
use crate::db::AppState;
use crate::services::reminder_engine::ReminderEngine;

#[derive(Clone, Copy)]
enum CommandLanguage {
    ZhTw,
    ZhCn,
    En,
}

impl CommandLanguage {
    fn from_code(language: &str) -> Self {
        match language {
            "zh-CN" => Self::ZhCn,
            "en" => Self::En,
            _ => Self::ZhTw,
        }
    }
}

fn telegram_config_warning(lang: CommandLanguage, kind: &str) -> String {
    match (lang, kind) {
        (CommandLanguage::ZhTw, "token") => "（Telegram 已啟用，但缺少 Bot Token）".to_string(),
        (CommandLanguage::ZhTw, "users") => "（Telegram 已啟用，但缺少允許的 user ID）".to_string(),
        (CommandLanguage::ZhCn, "token") => "（Telegram 已启用，但缺少 Bot Token）".to_string(),
        (CommandLanguage::ZhCn, "users") => "（Telegram 已启用，但缺少允许的 user ID）".to_string(),
        (CommandLanguage::En, "token") => {
            " (Telegram enabled but Bot Token is missing)".to_string()
        }
        (CommandLanguage::En, "users") => {
            " (Telegram enabled but allowed user IDs are missing)".to_string()
        }
        (_, _) => String::new(),
    }
}

fn trigger_success_message(
    lang: CommandLanguage,
    count: usize,
    telegram_users: Option<usize>,
) -> String {
    match (lang, telegram_users) {
        (CommandLanguage::ZhTw, Some(users)) => {
            format!("已成功觸發 {count} 個通知，並嘗試發送給 {users} 位 Telegram 使用者。")
        }
        (CommandLanguage::ZhTw, None) => format!("已成功觸發 {count} 個通知。"),
        (CommandLanguage::ZhCn, Some(users)) => {
            format!("已成功触发 {count} 个通知，并尝试发送给 {users} 位 Telegram 用户。")
        }
        (CommandLanguage::ZhCn, None) => format!("已成功触发 {count} 个通知。"),
        (CommandLanguage::En, Some(users)) => {
            format!("Triggered {count} notifications successfully, and attempted to send to {users} Telegram users.")
        }
        (CommandLanguage::En, None) => format!("Triggered {count} notifications successfully."),
    }
}

fn test_signal_sent_message(lang: CommandLanguage, users: usize) -> String {
    match lang {
        CommandLanguage::ZhTw => {
            format!("資料庫目前沒有到期項目，但測試訊號已發送給 {users} 位 Telegram 使用者。")
        }
        CommandLanguage::ZhCn => {
            format!("数据库目前没有到期项目，但测试信号已发送给 {users} 位 Telegram 用户。")
        }
        CommandLanguage::En => {
            format!(
                "No due items in database, but test signals were sent to {users} Telegram users."
            )
        }
    }
}

fn no_due_notifications_message(lang: CommandLanguage, warning: &str) -> String {
    match lang {
        CommandLanguage::ZhTw => format!("資料庫目前沒有到期通知{}。", warning),
        CommandLanguage::ZhCn => format!("数据库目前没有到期通知{}。", warning),
        CommandLanguage::En => format!(
            "There are currently no due notifications in database{}.",
            warning
        ),
    }
}

fn telegram_test_message(lang: CommandLanguage) -> &'static str {
    match lang {
        CommandLanguage::ZhTw => "*InsightCAP 測試通知*\n\n你的 Telegram 提醒設定有效且已連線。\n日後排定會議或截止日期時，通知會發送到這裡。",
        CommandLanguage::ZhCn => "*InsightCAP 测试通知*\n\n你的 Telegram 提醒设置有效且已连接。\n日后排定会议或截止日期时，通知会发送到这里。",
        CommandLanguage::En => "*InsightCAP Test Notification*\n\nYour Telegram reminder setup is valid and connected.\nWhen meetings or deadlines are scheduled, notifications will be sent here.",
    }
}

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

#[tauri::command]
pub async fn trigger_urgent_reminder_check(
    state: State<'_, AppState>,
    conversation_id: String,
    recent_messages: String,
) -> Result<Vec<String>, String> {
    let pool = &state.db;

    let summary: String = sqlx::query_scalar::<_, String>(
        "SELECT COALESCE(summary, '') FROM conversations WHERE id = ?",
    )
    .bind(&conversation_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
    .unwrap_or_default();

    let engine = ReminderEngine::new(pool.clone());
    let now = chrono::Utc::now().to_rfc3339();

    println!(
        "[UrgentReminder] Start urgent check: {} (has_summary: {})",
        conversation_id,
        !summary.is_empty()
    );

    let ids: Vec<String> = engine
        .extract_reminders(&conversation_id, &summary, &recent_messages, &now, None)
        .await?;

    if !ids.is_empty() {
        let count = ids.len();
        println!("[UrgentReminder] Extracted {} new reminders", count);
    } else {
        println!("[UrgentReminder] No valid reminders extracted from recent dialogue");
    }

    Ok(ids)
}

#[tauri::command]
pub async fn trigger_test_reminder(app: tauri::AppHandle) -> Result<String, String> {
    println!("[ReminderTest] Triggering test notification...");

    let state = app.state::<crate::db::AppState>();
    let settings = crate::settings::store::get_settings(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    let lang = CommandLanguage::from_code(&settings.general.language);

    let mut warning = String::new();
    if settings.telegram.enabled {
        if settings.telegram.bot_token.is_empty() {
            warning = telegram_config_warning(lang, "token");
        } else if settings.telegram.allowed_user_ids.is_empty() {
            warning = telegram_config_warning(lang, "users");
        }
    }

    let count = process_due_notifications(&app, true).await?;
    if count > 0 {
        let telegram_users = if settings.telegram.enabled && !settings.telegram.bot_token.is_empty()
        {
            Some(settings.telegram.allowed_user_ids.len())
        } else {
            None
        };
        Ok(trigger_success_message(lang, count, telegram_users))
    } else {
        if settings.telegram.enabled
            && !settings.telegram.bot_token.is_empty()
            && !settings.telegram.allowed_user_ids.is_empty()
        {
            for &user_id in &settings.telegram.allowed_user_ids {
                let _ = crate::background::telegram_bot::send_message(
                    &settings.telegram.bot_token,
                    user_id,
                    telegram_test_message(lang),
                )
                .await;
            }
            Ok(test_signal_sent_message(
                lang,
                settings.telegram.allowed_user_ids.len(),
            ))
        } else {
            Ok(no_due_notifications_message(lang, &warning))
        }
    }
}

#[tauri::command]
pub async fn clear_pending_notifications(state: State<'_, AppState>) -> Result<usize, String> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let result = sqlx::query("UPDATE reminder_notifications SET sent_at = ? WHERE sent_at IS NULL")
        .bind(&now)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result.rows_affected() as usize)
}

#[tauri::command]
pub async fn telegram_get_allowed_user_ids(bot_token: String) -> Result<Vec<i64>, String> {
    println!("[TelegramBot] Fetching user IDs from recent conversations...");
    let client = reqwest::Client::new();
    let url = format!("https://api.telegram.org/bot{}/getUpdates", bot_token);

    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

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

#[tauri::command]
pub async fn test_telegram_notification(
    bot_token: String,
    user_ids: Vec<i64>,
    language: Option<String>,
) -> Result<(), String> {
    println!(
        "[TelegramTest] Sending test notifications to: {:?}",
        user_ids
    );
    let lang = CommandLanguage::from_code(language.as_deref().unwrap_or("zh-TW"));
    let msg = telegram_test_message(lang);
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

#[tauri::command]
pub async fn debug_list_notifications(
    state: tauri::State<'_, crate::db::AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query(
        "SELECT n.*, r.title FROM reminder_notifications n \
         JOIN reminders r ON n.reminder_id = r.id \
         WHERE n.sent_at IS NULL ORDER BY n.scheduled_at ASC LIMIT 5",
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

#[tauri::command]
pub async fn verify_db_time_format(
    state: tauri::State<'_, crate::db::AppState>,
) -> Result<serde_json::Value, String> {
    let runtime_now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT scheduled_at FROM reminder_notifications WHERE sent_at IS NULL ORDER BY created_at DESC LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let db_str = row
        .map(|(s,)| s)
        .unwrap_or_else(|| "N/A (empty database)".to_string());

    let ms_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reminder_notifications WHERE scheduled_at LIKE '%.%'",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let has_mismatch = if db_str == "N/A (empty database)" {
        false
    } else {
        db_str.len() != runtime_now.len()
            || !db_str.ends_with('Z')
            || db_str.contains('.') != runtime_now.contains('.')
            || ms_count > 0
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
            format!("Detected {} records with milliseconds. This can break SQL '<=' string comparisons; normalize time fields.", ms_count)
        } else if has_mismatch {
            "Detected inconsistent time formats (possibly missing trailing Z).".to_string()
        } else {
            "Time format is consistent and clean (no milliseconds). Background polling should work normally.".to_string()
        }
    }))
}
#[tauri::command]
pub async fn manual_extract_reminders(
    state: tauri::State<'_, crate::db::AppState>,
    conversation_id: String,
) -> Result<String, String> {
    let pool = &state.db;

    let msgs = sqlx::query(
        "SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY created_at ASC",
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

    let summary: Option<String> =
        sqlx::query_scalar("SELECT summary FROM conversations WHERE id = ?")
            .bind(&conversation_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    let summary = summary.unwrap_or_else(|| "(no summary)".to_string());
    let now = chrono::Utc::now().to_rfc3339();

    let project_id: Option<String> =
        sqlx::query_scalar("SELECT project_id FROM conversations WHERE id = ?")
            .bind(&conversation_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    let engine = ReminderEngine::new(pool.clone());
    let created_ids = engine
        .extract_reminders(
            &conversation_id,
            &summary,
            &dialogue,
            &now,
            project_id.as_deref(),
        )
        .await?;

    Ok(format!(
        "Extracted {} reminders successfully.",
        created_ids.len()
    ))
}

#[tauri::command]
pub async fn check_reminder_health(
    state: tauri::State<'_, crate::db::AppState>,
) -> Result<serde_json::Value, String> {
    let pool = &state.db;
    let now_str = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let stuck_rows = sqlx::query(
        "SELECT n.id, r.title, r.status, r.pending_confirm FROM reminder_notifications n \
         JOIN reminders r ON n.reminder_id = r.id \
         WHERE n.sent_at IS NULL AND n.scheduled_at <= ? \
         AND (r.status != 'active' OR r.pending_confirm = 1)",
    )
    .bind(&now_str)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let yesterday = (chrono::Utc::now() - chrono::Duration::hours(24))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let stale_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reminder_notifications WHERE sent_at IS NULL AND scheduled_at < ?",
    )
    .bind(&yesterday)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let loop_count = state
        .reminder_loop_count
        .load(std::sync::atomic::Ordering::Relaxed);

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

    let reminders = sqlx::query(
        "SELECT * FROM reminders WHERE project_id = ? ORDER BY event_date ASC NULLS LAST",
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

    let chunks = sqlx::query(
        "SELECT id, content, knowledge_type, created_at FROM memory_chunks \
         WHERE project_id = ? AND knowledge_type IN ('pattern', 'log') \
         ORDER BY created_at DESC LIMIT 20",
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
