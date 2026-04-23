use chrono::{Duration, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::prompts;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::settings::store::get_settings;

pub struct ReminderEngine {
    pool: SqlitePool,
}

#[derive(Clone, Copy)]
enum ReminderMessageLanguage {
    ZhTw,
    ZhCn,
    En,
}

impl ReminderMessageLanguage {
    fn from_code(language: &str) -> Self {
        match language {
            "zh-CN" => Self::ZhCn,
            "en" => Self::En,
            _ => Self::ZhTw,
        }
    }
}

fn reminder_event_type_label(lang: ReminderMessageLanguage, event_type: &str) -> &str {
    match (lang, event_type) {
        (ReminderMessageLanguage::ZhTw, "meeting") => "會議",
        (ReminderMessageLanguage::ZhTw, "deliverable") => "交付物",
        (ReminderMessageLanguage::ZhTw, "event") => "事件",
        (ReminderMessageLanguage::ZhTw, "appointment") => "預約",
        (ReminderMessageLanguage::ZhCn, "meeting") => "会议",
        (ReminderMessageLanguage::ZhCn, "deliverable") => "交付物",
        (ReminderMessageLanguage::ZhCn, "event") => "事件",
        (ReminderMessageLanguage::ZhCn, "appointment") => "预约",
        (_, _) => event_type,
    }
}

fn reminder_status_label(lang: ReminderMessageLanguage, status: &str) -> &str {
    match (lang, status) {
        (ReminderMessageLanguage::ZhTw, "cancelled") => "已取消",
        (ReminderMessageLanguage::ZhTw, "completed") => "已完成",
        (ReminderMessageLanguage::ZhCn, "cancelled") => "已取消",
        (ReminderMessageLanguage::ZhCn, "completed") => "已完成",
        (_, _) => status,
    }
}

fn reminder_status_updated_message(
    lang: ReminderMessageLanguage,
    title: &str,
    status: &str,
) -> String {
    let status = reminder_status_label(lang, status);
    match lang {
        ReminderMessageLanguage::ZhTw => {
            format!("提醒狀態已更新\n\n標題：{}\n狀態：{}", title, status)
        }
        ReminderMessageLanguage::ZhCn => {
            format!("提醒状态已更新\n\n标题：{}\n状态：{}", title, status)
        }
        ReminderMessageLanguage::En => {
            format!("Reminder status updated\n\nTitle: {}\nStatus: {}", title, status)
        }
    }
}

fn reminder_created_message(
    lang: ReminderMessageLanguage,
    title: &str,
    time_display: &str,
    event_type: &str,
) -> String {
    let event_type = reminder_event_type_label(lang, event_type);
    match lang {
        ReminderMessageLanguage::ZhTw => format!(
            "提醒已建立\n\n標題：{}\n時間：{}\n類型：{}\n\n請在程式內檢查或編輯。",
            title, time_display, event_type
        ),
        ReminderMessageLanguage::ZhCn => format!(
            "提醒已建立\n\n标题：{}\n时间：{}\n类型：{}\n\n请在程序内检查或编辑。",
            title, time_display, event_type
        ),
        ReminderMessageLanguage::En => format!(
            "Reminder created\n\nTitle: {}\nTime: {}\nType: {}\n\nPlease review or edit it in the app.",
            title, time_display, event_type
        ),
    }
}

impl ReminderEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn extract_reminders(
        &self,
        conversation_id: &str,
        summary: &str,
        dialogue: &str,
        conversation_timestamp: &str,
        project_id: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let settings = get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let primary_cfg = settings.ai_models.content_processor_llm.clone();
        let fallback_cfg = settings.ai_models.chat_llm.clone();

        let result = self
            .attempt_extraction(&primary_cfg, conversation_timestamp, summary, dialogue)
            .await;

        let json = match result {
            Ok(json) => json,
            Err(e) => {
                println!(
                    "[ReminderEngine] Primary extraction failed ({}), trying fallback model...",
                    e
                );
                self.attempt_extraction(&fallback_cfg, conversation_timestamp, summary, dialogue)
                    .await
                    .map_err(|e2| format!("All reminder extraction attempts failed: {}; {}", e, e2))
                    .unwrap_or_else(|e2| {
                        eprintln!(
                            "[ReminderEngine] Emergency reminder extraction fallback failed: {}",
                            e2
                        );
                        serde_json::Value::Null
                    })
            }
        };

        let reminders = match json["reminders"].as_array() {
            Some(arr) => arr.clone(),
            None => return Ok(vec![]),
        };

        let daily_time = &settings.reminders.daily_reminder_time;
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let mut created_ids = Vec::new();

        let valid_project_id: Option<&str> = if let Some(pid) = project_id {
            if pid.trim().is_empty() {
                None
            } else {
                let exists: bool =
                    sqlx::query_scalar::<_, i32>("SELECT COUNT(*) FROM projects WHERE id = ?")
                        .bind(pid)
                        .fetch_one(&self.pool)
                        .await
                        .unwrap_or(0)
                        > 0;
                if exists {
                    Some(pid)
                } else {
                    None
                }
            }
        } else {
            None
        };

        for item in &reminders {
            let title = item["title"].as_str().unwrap_or("").trim();
            if title.is_empty() {
                continue;
            }

            let event_date_raw = item["event_date"].as_str().unwrap_or("").trim();
            let event_time_raw = item["event_time"].as_str().map(|s| s.trim()).unwrap_or("");
            let (event_time, event_time_parse_failed) =
                normalize_event_time_with_status(event_time_raw);
            if event_time_parse_failed {
                eprintln!(
                    "[ReminderEngine] Failed to parse event_time; skipping imminent fallback: raw='{}', title='{}', conversation_id={}",
                    event_time_raw,
                    title,
                    conversation_id
                );
            }

            let event_date = if event_date_raw.is_empty() && event_time.is_some() {
                chrono::Local::now().format("%Y-%m-%d").to_string()
            } else {
                event_date_raw.to_string()
            };

            let date_status = item["date_status"]
                .as_str()
                .filter(|s| matches!(*s, "confirmed" | "time_inferred" | "range" | "month_only"))
                .unwrap_or("confirmed");

            let event_type = item["event_type"]
                .as_str()
                .filter(|s| matches!(*s, "meeting" | "deliverable" | "event" | "appointment"))
                .unwrap_or("event");

            let status_req = item["status"].as_str().unwrap_or("active");

            if status_req == "cancelled" || status_req == "completed" {
                let db_status = if status_req == "cancelled" {
                    "dismissed"
                } else {
                    "completed"
                };

                let rows_affected = sqlx::query(
                    "UPDATE reminders SET status = ?, updated_at = ? \
                     WHERE (conversation_id = ? OR title = ?) \
                     AND (event_date = ? OR event_date IS NULL) \
                     AND status = 'active'",
                )
                .bind(db_status)
                .bind(&now)
                .bind(conversation_id)
                .bind(title)
                .bind(if event_date.is_empty() {
                    None
                } else {
                    Some(&event_date)
                })
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?
                .rows_affected();

                if rows_affected > 0 {
                    println!(
                        "[ReminderEngine] Updated reminder status to {}: {}",
                        db_status, title
                    );
                    if settings.telegram.enabled
                        && !settings.telegram.bot_token.is_empty()
                        && !settings.telegram.allowed_user_ids.is_empty()
                    {
                        let op_text = if status_req == "cancelled" {
                            "cancelled"
                        } else {
                            "completed"
                        };
                        let lang = ReminderMessageLanguage::from_code(&settings.general.language);
                        let msg = reminder_status_updated_message(lang, title, op_text);
                        for &user_id in &settings.telegram.allowed_user_ids {
                            let _ = crate::background::telegram_bot::send_message(
                                &settings.telegram.bot_token,
                                user_id,
                                &msg,
                            )
                            .await;
                        }
                    }
                }
                continue;
            }

            if self
                .is_duplicate(
                    conversation_id,
                    title,
                    if event_date.is_empty() {
                        None
                    } else {
                        Some(event_date.as_str())
                    },
                    event_time.as_deref(),
                )
                .await?
            {
                continue;
            }
            let event_date_end = item["event_date_end"]
                .as_str()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());
            let description = item["description"].as_str().unwrap_or("").trim();
            let confidence = item["confidence"]
                .as_f64()
                .map(|f| f.clamp(0.0, 1.0) as f32)
                .unwrap_or(0.7);
            let pending_confirm = if confidence < 0.49 { 1_i32 } else { 0_i32 };

            let reminder_id = Uuid::now_v7().to_string();

            sqlx::query(
                "INSERT INTO reminders (id, conversation_id, space_id, project_id, title, description, \
                 event_type, date_status, event_date, event_date_end, event_time, confidence, \
                 status, pending_confirm, created_at, updated_at) \
                 VALUES (?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?)"
            )
            .bind(&reminder_id)
            .bind(conversation_id)
            .bind(valid_project_id)
            .bind(title)
            .bind(description)
            .bind(event_type)
            .bind(date_status)
            .bind(if event_date.is_empty() { None } else { Some(&event_date) })
            .bind(event_date_end)
            .bind(event_time.as_deref())
            .bind(confidence)
            .bind(pending_confirm)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

            if !event_date.is_empty() {
                let mut schedule = generate_notification_schedule(
                    event_type,
                    date_status,
                    &event_date,
                    event_time.as_deref(),
                    daily_time,
                );
                if schedule.is_empty() && !event_time_parse_failed {
                    if let Some(fallback) =
                        generate_imminent_fallback(&event_date, event_time.as_deref())
                    {
                        schedule.push(fallback);
                    }
                }
                for (intent, scheduled_at) in &schedule {
                    let notif_id = Uuid::now_v7().to_string();
                    sqlx::query(
                        "INSERT INTO reminder_notifications (id, reminder_id, intent, scheduled_at, channel, created_at) \
                         VALUES (?, ?, ?, ?, 'both', ?)"
                    )
                    .bind(&notif_id)
                    .bind(&reminder_id)
                    .bind(intent)
                    .bind(scheduled_at)
                    .bind(&now)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| e.to_string())?;
                }
            }

            println!(
                "[ReminderEngine] Created reminder: {} (type={}, status={}, confidence={:.2}, pending={})",
                title, event_type, date_status, confidence, pending_confirm
            );

            if settings.telegram.enabled
                && !settings.telegram.bot_token.is_empty()
                && !settings.telegram.allowed_user_ids.is_empty()
            {
                let lang = ReminderMessageLanguage::from_code(&settings.general.language);
                let time_display = match (&event_date, &event_time) {
                    (d, Some(t)) if !d.is_empty() => format!("{} {}", d, t),
                    (d, None) if !d.is_empty() => d.to_string(),
                    _ => match lang {
                        ReminderMessageLanguage::ZhTw => "未設定".to_string(),
                        ReminderMessageLanguage::ZhCn => "未设置".to_string(),
                        ReminderMessageLanguage::En => "not set".to_string(),
                    },
                };
                let confirm_msg = reminder_created_message(lang, title, &time_display, event_type);

                let bot_token = settings.telegram.bot_token.clone();
                for &user_id in &settings.telegram.allowed_user_ids {
                    let _ = crate::background::telegram_bot::send_message(
                        &bot_token,
                        user_id,
                        &confirm_msg,
                    )
                    .await;
                }
            }

            created_ids.push(reminder_id);
        }

        Ok(created_ids)
    }

    async fn attempt_extraction(
        &self,
        cfg: &crate::settings::store::ModelSettings,
        conversation_timestamp: &str,
        summary: &str,
        dialogue: &str,
    ) -> Result<serde_json::Value, String> {
        let api_key = cfg.api_key.clone().unwrap_or_default();
        if api_key.is_empty() && cfg.provider != "ollama" {
            return Err("Missing API key".to_string());
        }

        let input = format!(
            "Conversation summary:\n{}\n\nConversation content:\n{}",
            summary, dialogue
        );
        let prompt = format!(
            "{}{}\n\n{}",
            prompts::REMINDER_EXTRACT_PROMPT,
            conversation_timestamp,
            input
        );

        let provider = OpenAiProvider::new(
            api_key,
            cfg.base_url.clone(),
            cfg.model.clone(),
            cfg.provider.clone(),
        );
        let opts = LLMOptions {
            temperature: 0.1,
            max_tokens: 800,
            stream: false,
            think_mode: None,
        };

        match tokio::time::timeout(
            std::time::Duration::from_secs(45),
            provider.complete_json(&prompt, opts.clone()),
        )
        .await
        {
            Ok(Ok(json)) => Ok(normalize_reminder_json(json)),
            Ok(Err(e)) => {
                let err_text = e.to_string();
                if let Some(v) = try_parse_reminder_json_from_error(&err_text) {
                    println!("[ReminderEngine] Parsed reminders from JSON error text.");
                    return Ok(v);
                }

                match tokio::time::timeout(
                    std::time::Duration::from_secs(45),
                    provider.complete(&prompt, opts),
                )
                .await
                {
                    Ok(Ok(text)) => {
                        if let Some(v) = try_parse_reminder_json(&text) {
                            println!("[ReminderEngine] Parsed reminders from text fallback.");
                            Ok(v)
                        } else if text.trim().is_empty() || is_empty_raw_parse_error(&err_text) {
                            println!("[ReminderEngine] Empty reminder extraction result, using empty reminders.");
                            Ok(serde_json::json!({ "reminders": [] }))
                        } else {
                            Err(format!(
                                "{}; text fallback returned non-JSON content (len={})",
                                err_text,
                                text.chars().count()
                            ))
                        }
                    }
                    Ok(Err(e2)) => {
                        if is_empty_raw_parse_error(&err_text) {
                            println!("[ReminderEngine] JSON parse returned empty and text fallback failed; using empty reminders.");
                            Ok(serde_json::json!({ "reminders": [] }))
                        } else {
                            Err(format!("{}; text fallback error: {}", err_text, e2))
                        }
                    }
                    Err(_) => {
                        if is_empty_raw_parse_error(&err_text) {
                            println!("[ReminderEngine] JSON parse returned empty and text fallback timed out; using empty reminders.");
                            Ok(serde_json::json!({ "reminders": [] }))
                        } else {
                            Err(err_text)
                        }
                    }
                }
            }
            Err(_) => Err("Reminder extraction timed out".to_string()),
        }
    }

    async fn is_duplicate(
        &self,
        conversation_id: &str,
        title: &str,
        event_date: Option<&str>,
        event_time: Option<&str>,
    ) -> Result<bool, String> {
        if let Some(date) = event_date {
            let count: i64 = if let Some(time) = event_time {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM reminders \
                     WHERE conversation_id = ? AND event_date = ? AND event_time = ? AND status = 'active'"
                )
                .bind(conversation_id)
                .bind(date)
                .bind(time)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| e.to_string())?
            } else {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM reminders \
                     WHERE conversation_id = ? AND event_date = ? AND event_time IS NULL AND status = 'active'"
                )
                .bind(conversation_id)
                .bind(date)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| e.to_string())?
            };
            if count > 0 {
                return Ok(true);
            }
        }

        let count: i64 = match (event_date, event_time) {
            (Some(date), Some(time)) => {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM reminders WHERE title = ? AND event_date = ? AND event_time = ? AND status = 'active'"
                )
                .bind(title)
                .bind(date)
                .bind(time)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| e.to_string())?
            }
            (Some(date), None) => {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM reminders WHERE title = ? AND event_date = ? AND event_time IS NULL AND status = 'active'"
                )
                .bind(title)
                .bind(date)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| e.to_string())?
            }
            (None, Some(time)) => {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM reminders WHERE title = ? AND event_date IS NULL AND event_time = ? AND status = 'active'"
                )
                .bind(title)
                .bind(time)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| e.to_string())?
            }
            (None, None) => {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM reminders WHERE title = ? AND event_date IS NULL AND event_time IS NULL AND status = 'active'"
                )
                .bind(title)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| e.to_string())?
            }
        };
        Ok(count > 0)
    }

    pub async fn confirm_reminder(&self, reminder_id: &str, accept: bool) -> Result<(), String> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        if accept {
            sqlx::query("UPDATE reminders SET pending_confirm = 0, updated_at = ? WHERE id = ?")
                .bind(&now)
                .bind(reminder_id)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            sqlx::query("UPDATE reminders SET status = 'dismissed', pending_confirm = 0, updated_at = ? WHERE id = ?")
                .bind(&now)
                .bind(reminder_id)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub async fn update_reminder_status(
        &self,
        reminder_id: &str,
        status: &str,
    ) -> Result<(), String> {
        if !matches!(status, "active" | "completed" | "dismissed" | "expired") {
            return Err("Invalid status".to_string());
        }
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query("UPDATE reminders SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status)
            .bind(&now)
            .bind(reminder_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn snooze_reminder(&self, reminder_id: &str, minutes: i64) -> Result<(), String> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let snooze_until = (Utc::now() + Duration::minutes(minutes))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        let pending_notif_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM reminder_notifications \
             WHERE reminder_id = ? AND sent_at IS NULL \
             ORDER BY datetime(scheduled_at) ASC LIMIT 1",
        )
        .bind(reminder_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        if let Some(notif_id) = pending_notif_id {
            sqlx::query("UPDATE reminder_notifications SET scheduled_at = ? WHERE id = ?")
                .bind(&snooze_until)
                .bind(&notif_id)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            let new_notif_id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO reminder_notifications (id, reminder_id, intent, scheduled_at, channel, created_at) \
                 VALUES (?, ?, 'imminent', ?, 'both', ?)"
            )
            .bind(&new_notif_id)
            .bind(reminder_id)
            .bind(&snooze_until)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        }

        let updated = sqlx::query("UPDATE reminders SET updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(reminder_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        if updated.rows_affected() == 0 {
            return Err("Reminder not found".to_string());
        }
        Ok(())
    }

    pub async fn get_active_reminders(&self) -> Result<Vec<Value>, String> {
        let rows = sqlx::query(
             "SELECT r.*, \
              (SELECT COUNT(*) FROM reminder_notifications WHERE reminder_id = r.id AND sent_at IS NULL) as pending_notifs \
              FROM reminders r \
              WHERE r.status = 'active' \
              AND (r.event_date >= date('now', 'localtime') OR r.event_date IS NULL OR r.event_date = '') \
              ORDER BY r.event_date ASC NULLS LAST"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut results = Vec::new();
        for row in &rows {
            results.push(serde_json::json!({
                "id": row.get::<String, _>("id"),
                "conversationId": row.get::<Option<String>, _>("conversation_id"),
                "spaceId": row.get::<Option<String>, _>("space_id"),
                "projectId": row.get::<Option<String>, _>("project_id"),
                "title": row.get::<String, _>("title"),
                "description": row.get::<Option<String>, _>("description"),
                "eventType": row.get::<String, _>("event_type"),
                "dateStatus": row.get::<String, _>("date_status"),
                "eventDate": row.get::<Option<String>, _>("event_date"),
                "eventDateEnd": row.get::<Option<String>, _>("event_date_end"),
                "eventTime": row.get::<Option<String>, _>("event_time"),
                "confidence": row.get::<f64, _>("confidence"),
                "status": row.get::<String, _>("status"),
                "pendingConfirm": row.get::<i32, _>("pending_confirm"),
                "pendingNotifs": row.get::<i64, _>("pending_notifs"),
                "createdAt": row.get::<String, _>("created_at"),
                "updatedAt": row.get::<String, _>("updated_at"),
            }));
        }
        Ok(results)
    }

    pub async fn get_pending_reminders(&self) -> Result<Vec<Value>, String> {
        let rows = sqlx::query(
            "SELECT * FROM reminders WHERE pending_confirm = 1 AND status = 'active' ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut results = Vec::new();
        for row in &rows {
            results.push(serde_json::json!({
                "id": row.get::<String, _>("id"),
                "conversationId": row.get::<Option<String>, _>("conversation_id"),
                "title": row.get::<String, _>("title"),
                "description": row.get::<Option<String>, _>("description"),
                "eventType": row.get::<String, _>("event_type"),
                "dateStatus": row.get::<String, _>("date_status"),
                "eventDate": row.get::<Option<String>, _>("event_date"),
                "eventTime": row.get::<Option<String>, _>("event_time"),
                "confidence": row.get::<f64, _>("confidence"),
                "pendingConfirm": row.get::<i32, _>("pending_confirm"),
                "createdAt": row.get::<String, _>("created_at"),
            }));
        }
        Ok(results)
    }
}


fn generate_notification_schedule(
    event_type: &str,
    date_status: &str,
    event_date: &str,
    event_time: Option<&str>,
    daily_reminder_time: &str,
) -> Vec<(String, String)> {
    let date = match NaiveDate::parse_from_str(event_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return vec![],
    };

    let daily_time = NaiveTime::parse_from_str(daily_reminder_time, "%H:%M")
        .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap());

    let today = Local::now().date_naive();
    let mut schedule: Vec<(String, String)> = Vec::new();

    if matches!(date_status, "range" | "month_only") {
        let confirm_at = if date > today + Duration::days(7) {
            date - Duration::days(7)
        } else if date > today {
            today
        } else {
            return vec![];
        };
        schedule.push((
            "confirm_date".to_string(),
            naive_local_to_utc_str(confirm_at.and_time(daily_time)),
        ));
        return filter_future(schedule);
    }

    let has_time = event_time.is_some();
    let time = event_time
        .and_then(|t| NaiveTime::parse_from_str(t, "%H:%M").ok())
        .unwrap_or(daily_time);

    match event_type {
        "meeting" => {
            let prepare_date = date - Duration::days(1);
            schedule.push((
                "prepare".to_string(),
                naive_local_to_utc_str(prepare_date.and_time(daily_time)),
            ));
            if has_time {
                let event_dt = date.and_time(time);
                let imminent = event_dt - Duration::hours(1);
                schedule.push(("imminent".to_string(), naive_local_to_utc_str(imminent)));
                let now_dt = event_dt - Duration::minutes(10);
                schedule.push(("now".to_string(), naive_local_to_utc_str(now_dt)));
            } else {
                let event_dt = date.and_time(time);
                let now_dt = event_dt - Duration::minutes(30);
                schedule.push(("now".to_string(), naive_local_to_utc_str(now_dt)));
            }
        }
        "deliverable" => {
            let days_until = (date - today).num_days();
            if days_until >= 7 {
                let start_date = date - Duration::days(7);
                schedule.push((
                    "start".to_string(),
                    naive_local_to_utc_str(start_date.and_time(daily_time)),
                ));
            }
            if days_until >= 3 {
                let mid_date = date - Duration::days(3);
                schedule.push((
                    "midcheck".to_string(),
                    naive_local_to_utc_str(mid_date.and_time(daily_time)),
                ));
            }
            if days_until >= 1 {
                let urgent_date = date - Duration::days(1);
                schedule.push((
                    "urgent".to_string(),
                    naive_local_to_utc_str(urgent_date.and_time(daily_time)),
                ));
            }
            schedule.push((
                "final".to_string(),
                naive_local_to_utc_str(date.and_time(daily_time)),
            ));
        }
        "appointment" => {
            let prepare_date = date - Duration::days(1);
            schedule.push((
                "prepare".to_string(),
                naive_local_to_utc_str(prepare_date.and_time(daily_time)),
            ));
            if has_time {
                let event_dt = date.and_time(time);
                let imminent = event_dt - Duration::hours(1);
                schedule.push(("imminent".to_string(), naive_local_to_utc_str(imminent)));
            }
        }
        _ => {
            let days_until = (date - today).num_days();
            if days_until >= 3 {
                let start_date = date - Duration::days(3);
                schedule.push((
                    "start".to_string(),
                    naive_local_to_utc_str(start_date.and_time(daily_time)),
                ));
            }
            schedule.push((
                "final".to_string(),
                naive_local_to_utc_str(date.and_time(daily_time)),
            ));
        }
    }

    filter_future(schedule)
}

fn filter_future(schedule: Vec<(String, String)>) -> Vec<(String, String)> {
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    schedule
        .into_iter()
        .filter(|(_, scheduled_at)| scheduled_at.as_str() > now.as_str())
        .collect()
}

fn naive_local_to_utc_str(naive_local: NaiveDateTime) -> String {
    let local_dt = match Local.from_local_datetime(&naive_local) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(early, _) => early,
        LocalResult::None => {
            let shifted = naive_local + Duration::hours(1);
            match Local.from_local_datetime(&shifted) {
                LocalResult::Single(dt) => dt,
                LocalResult::Ambiguous(early, _) => early,
                LocalResult::None => {
                    let offset_secs = Local::now().offset().local_minus_utc();
                    let utc_naive = naive_local - Duration::seconds(offset_secs as i64);
                    return format!("{}Z", utc_naive.format("%Y-%m-%dT%H:%M:%S"));
                }
            }
        }
    };
    local_dt
        .with_timezone(&Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn generate_imminent_fallback(
    event_date: &str,
    event_time: Option<&str>,
) -> Option<(String, String)> {
    let local_now = Local::now();
    let today = local_now.date_naive();
    let date = NaiveDate::parse_from_str(event_date, "%Y-%m-%d").ok()?;
    if date != today {
        return None;
    }

    let event_naive_time = event_time.and_then(|t| NaiveTime::parse_from_str(t, "%H:%M").ok());

    if let Some(t) = event_naive_time {
        let event_local = date.and_time(t);

        if event_local + Duration::minutes(10) <= local_now.naive_local() {
            return None;
        }

        let remind_local = event_local - Duration::minutes(5);
        if remind_local <= local_now.naive_local() {
            let utc_at = Utc::now() - Duration::seconds(1);
            Some((
                "imminent".to_string(),
                utc_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ))
        } else {
            Some(("imminent".to_string(), naive_local_to_utc_str(remind_local)))
        }
    } else {
        let utc_at = Utc::now() - Duration::seconds(1);
        Some((
            "imminent".to_string(),
            utc_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        ))
    }
}

fn normalize_reminder_json(v: serde_json::Value) -> serde_json::Value {
    if v.get("reminders").and_then(|x| x.as_array()).is_some() {
        v
    } else if let Some(arr) = v.as_array() {
        serde_json::json!({ "reminders": arr })
    } else {
        serde_json::json!({ "reminders": [] })
    }
}

#[cfg(test)]
fn normalize_event_time(raw: &str) -> Option<String> {
    normalize_event_time_with_status(raw).0
}

fn normalize_event_time_with_status(raw: &str) -> (Option<String>, bool) {
    let text = raw.trim().replace('\u{ff1a}', ":");
    if text.is_empty() {
        return (None, false);
    }

    for fmt in ["%H:%M", "%H:%M:%S", "%I:%M %p", "%I:%M%p"] {
        if let Ok(t) = NaiveTime::parse_from_str(&text, fmt) {
            return (Some(t.format("%H:%M").to_string()), false);
        }
    }

    if let Some((hour, minute)) = extract_hhmm_fragment(&text) {
        let lower = text.to_lowercase();
        let has_pm = lower.contains("pm");
        let has_am = lower.contains("am");
        let mut h = hour;
        if has_pm && h < 12 {
            h += 12;
        } else if has_am && h == 12 {
            h = 0;
        }
        if h < 24 && minute < 60 {
            return (Some(format!("{:02}:{:02}", h, minute)), false);
        }
    }

    (None, true)
}

fn extract_hhmm_fragment(text: &str) -> Option<(u32, u32)> {
    let bytes = text.as_bytes();
    for i in 0..bytes.len() {
        if !bytes[i].is_ascii_digit() {
            continue;
        }

        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_digit() && j - i < 2 {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b':' {
            continue;
        }
        if j + 2 >= bytes.len() {
            continue;
        }
        if !bytes[j + 1].is_ascii_digit() || !bytes[j + 2].is_ascii_digit() {
            continue;
        }

        let hour_str = &text[i..j];
        let min_str = &text[j + 1..j + 3];
        let hour = hour_str.parse::<u32>().ok()?;
        let minute = min_str.parse::<u32>().ok()?;
        if hour < 24 && minute < 60 {
            return Some((hour, minute));
        }
    }
    None
}

fn try_parse_reminder_json_from_error(err_text: &str) -> Option<serde_json::Value> {
    let raw_idx = err_text.find("Raw:")?;
    let raw = err_text[raw_idx + 4..].trim();
    try_parse_reminder_json(raw)
}

fn is_empty_raw_parse_error(err_text: &str) -> bool {
    if !(err_text.contains("Failed to parse JSON string from LLM")
        && err_text.contains("EOF while parsing a value"))
    {
        return false;
    }

    let Some(raw_idx) = err_text.find("Raw:") else {
        return false;
    };

    err_text[raw_idx + 4..].trim().is_empty()
}

fn try_parse_reminder_json(raw: &str) -> Option<serde_json::Value> {
    let mut text = raw.trim();
    if text.is_empty() {
        return None;
    }

    if let Some(end_idx) = text.find("</think>") {
        text = text[end_idx + "</think>".len()..].trim();
    }
    if text.starts_with("```json") {
        text = text
            .trim_start_matches("```json")
            .trim_end_matches("```")
            .trim();
    } else if text.starts_with("```") {
        text = text
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
    }

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        return Some(normalize_reminder_json(v));
    }

    if text.starts_with("\"reminders\"") {
        let wrapped = format!("{{{}}}", text);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&wrapped) {
            return Some(normalize_reminder_json(v));
        }
    }

    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        if start < end {
            let obj = &text[start..=end];
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(obj) {
                return Some(normalize_reminder_json(v));
            }
        }
    }

    if let Some(key_pos) = text.find("\"reminders\"") {
        if let Some(bracket_start_rel) = text[key_pos..].find('[') {
            let bracket_start = key_pos + bracket_start_rel;
            let mut depth = 0_i32;
            let mut bracket_end = None;
            for (idx, ch) in text[bracket_start..].char_indices() {
                match ch {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            bracket_end = Some(bracket_start + idx);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(end_idx) = bracket_end {
                let arr_str = &text[bracket_start..=end_idx];
                if let Ok(arr) = serde_json::from_str::<serde_json::Value>(arr_str) {
                    if let Some(items) = arr.as_array() {
                        return Some(serde_json::json!({ "reminders": items }));
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        generate_imminent_fallback, is_empty_raw_parse_error, naive_local_to_utc_str,
        normalize_event_time, normalize_event_time_with_status,
    };
    use chrono::{Local, NaiveDate, NaiveTime};

    #[test]
    fn test_naive_local_to_utc_str_has_utc_format() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
        let time = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
        let out = naive_local_to_utc_str(date.and_time(time));
        assert!(out.ends_with('Z'));
        assert!(out.contains('T'));
    }

    #[test]
    fn test_generate_imminent_fallback_for_today_without_time() {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let result = generate_imminent_fallback(&today, None);
        assert!(result.is_some());
        let (intent, scheduled_at) = result.unwrap();
        assert_eq!(intent, "imminent");
        assert!(scheduled_at.ends_with('Z'));
    }

    #[test]
    fn test_detect_empty_raw_parse_error() {
        let err = "Parse error: Failed to parse JSON string from LLM: EOF while parsing a value at line 1 column 0\nRaw: ";
        assert!(is_empty_raw_parse_error(err));

        let err_non_empty = "Parse error: Failed to parse JSON string from LLM: expected value at line 1\nRaw: {\"reminders\":[]}";
        assert!(!is_empty_raw_parse_error(err_non_empty));
    }

    #[test]
    fn test_normalize_event_time_handles_ampm() {
        assert_eq!(normalize_event_time("10:55 PM"), Some("22:55".to_string()));
        assert_eq!(normalize_event_time("12:05 AM"), Some("00:05".to_string()));
    }

    #[test]
    fn test_normalize_event_time_handles_noisy_text() {
        assert_eq!(
            normalize_event_time("10:55 from noisy text"),
            Some("10:55".to_string())
        );
        assert_eq!(
            normalize_event_time("meeting at 7:30"),
            Some("07:30".to_string())
        );
    }

    #[test]
    fn test_normalize_event_time_with_status_marks_failed_parse() {
        let (parsed, failed) = normalize_event_time_with_status("evening time");
        assert_eq!(parsed, None);
        assert!(failed);

        let (parsed_ok, failed_ok) = normalize_event_time_with_status("22:30");
        assert_eq!(parsed_ok, Some("22:30".to_string()));
        assert!(!failed_ok);
    }
}
