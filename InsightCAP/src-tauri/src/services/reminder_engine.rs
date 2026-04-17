use chrono::{Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Utc};
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

impl ReminderEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 從對話摘要中提取提醒事項，並建立 reminders + notifications 資料。
    /// 回傳建立的 reminder IDs。
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

        // 嘗試使用內容處理模型提取
        let result = self.attempt_extraction(&primary_cfg, conversation_timestamp, summary, dialogue).await;
        
        let json = match result {
            Ok(json) => json,
            Err(e) => {
                println!("[ReminderEngine] 主要模型提取失敗 ({})，嘗試切換至聊天模型...", e);
                self.attempt_extraction(&fallback_cfg, conversation_timestamp, summary, dialogue).await
                    .map_err(|e2| format!("所有模型提取皆失敗: {}, 且 {}", e, e2))
                    .unwrap_or_else(|e2| {
                        eprintln!("[ReminderEngine] 緊急恢復提取失敗: {}", e2);
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
                let exists: bool = sqlx::query_scalar::<_, i32>(
                    "SELECT COUNT(*) FROM projects WHERE id = ?"
                )
                .bind(pid)
                .fetch_one(&self.pool)
                .await
                .unwrap_or(0) > 0;
                if exists { Some(pid) } else { None }
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
            let event_time = item["event_time"].as_str().map(|s| s.trim()).filter(|s| !s.is_empty());

            // 修正：如果日期為空但時間不為空，預設為「今天」(Local Now)
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

            // --- 處理取消或完成機制 ---
            if status_req == "cancelled" || status_req == "completed" {
                let db_status = if status_req == "cancelled" { "dismissed" } else { "completed" };
                
                // 尋找匹配的活動中提醒
                let rows_affected = sqlx::query(
                    "UPDATE reminders SET status = ?, updated_at = ? \
                     WHERE (conversation_id = ? OR title = ?) \
                     AND (event_date = ? OR event_date IS NULL) \
                     AND status = 'active'"
                )
                .bind(db_status)
                .bind(&now)
                .bind(conversation_id)
                .bind(title)
                .bind(if event_date.is_empty() { None } else { Some(&event_date) })
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?
                .rows_affected();

                if rows_affected > 0 {
                    println!("[ReminderEngine] 已根據對話更新提醒狀態為 {}: {}", db_status, title);
                    if settings.telegram.enabled {
                        let op_text = if status_req == "cancelled" { "已取消" } else { "已完成" };
                        let _ = crate::background::telegram_bot::send_message(
                            &settings.telegram.bot_token,
                            settings.telegram.allowed_user_ids[0],
                            &format!("⚠️ *提醒狀態更新*\n\n項目：{}\n狀態：{}", title, op_text)
                        ).await;
                    }
                }
                continue;
            }

            // 重複檢查 (僅針對 active)
            if self.is_duplicate(
                conversation_id,
                title,
                if event_date.is_empty() { None } else { Some(event_date.as_str()) },
                event_time,
            ).await? {
                continue;
            }
            let event_date_end = item["event_date_end"].as_str().map(|s| s.trim()).filter(|s| !s.is_empty());
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
            .bind(event_time)
            .bind(confidence)
            .bind(pending_confirm)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

            // 建立通知排程
            if !event_date.is_empty() {
                let mut schedule = generate_notification_schedule(
                    event_type,
                    date_status,
                    &event_date,
                    event_time.as_deref(),
                    daily_time,
                );
                // 如果是今天且未來排程已空，觸發 imminent fallback
                if schedule.is_empty() {
                    if let Some(fallback) = generate_imminent_fallback(&event_date, event_time.as_deref()) {
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
                "[ReminderEngine] 建立提醒: {} (type={}, status={}, confidence={:.2}, pending={})",
                title, event_type, date_status, confidence, pending_confirm
            );

            // --- 新增：向 Telegram 發送設定成功訊息 ---
            if settings.telegram.enabled && !settings.telegram.bot_token.is_empty() && !settings.telegram.allowed_user_ids.is_empty() {
                let time_display = match (&event_date, &event_time) {
                    (d, Some(t)) if !d.is_empty() => format!("{} {}", d, t),
                    (d, None) if !d.is_empty() => d.to_string(),
                    _ => "待定".to_string(),
                };
                let confirm_msg = format!("✅ *已成功設定提醒*\n\n標題：{}\n時間：{}\n類型：{}\n\n系統將準時在上述時間通知您。", title, time_display, event_type);
                
                let bot_token = settings.telegram.bot_token.clone();
                for &user_id in &settings.telegram.allowed_user_ids {
                    let _ = crate::background::telegram_bot::send_message(&bot_token, user_id, &confirm_msg).await;
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
            return Err("缺少 API Key".to_string());
        }

        let input = format!("對話摘要：\n{}\n\n對話內容：\n{}", summary, dialogue);
        let prompt = format!("{}{}\n\n{}", prompts::REMINDER_EXTRACT_PROMPT, conversation_timestamp, input);

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
            provider.complete_json(&prompt, opts),
        ).await {
            Ok(Ok(json)) => Ok(json),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err("提取超時".to_string()),
        }
    }

    /// 重複性檢查：分為兩層
    /// 1. 同一對話 + 同一日期 + 同一時間（通常是防重發）
    /// 2. 標題 + 日期 + 時間（跨對話重複）
    async fn is_duplicate(
        &self,
        conversation_id: &str,
        title: &str,
        event_date: Option<&str>,
        event_time: Option<&str>,
    ) -> Result<bool, String> {
        // Layer 1: same conversation + same date + same time
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

        // Layer 2: exact title + date + time (cross-conversation)
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

    /// 核心操作：確認/否認 pending reminder
    pub async fn confirm_reminder(
        &self,
        reminder_id: &str,
        accept: bool,
    ) -> Result<(), String> {
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

    /// 更新提醒狀態
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

    /// 撱嗅???嚗????潮? notifications 撱嗅?????
    pub async fn snooze_reminder(
        &self,
        reminder_id: &str,
        minutes: i64,
    ) -> Result<(), String> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        // 閮?撱嗅???
        let snooze_until = (Utc::now() + Duration::minutes(minutes)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        // 撠??拍??芰??notification 撱嗅?
        sqlx::query(
            "UPDATE reminder_notifications SET scheduled_at = ? \
             WHERE id = (SELECT id FROM reminder_notifications \
             WHERE reminder_id = ? AND sent_at IS NULL ORDER BY scheduled_at ASC LIMIT 1)"
        )
        .bind(&snooze_until)
        .bind(reminder_id)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        // 更新 reminder 的 updated_at
        sqlx::query("UPDATE reminders SET updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(reminder_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 獲取所有活動中的提醒（用於列表顯示）
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

    /// 獲取待確認的提醒
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

// ─── 輔助函式：通知排程邏輯 ────────────────────────────────────────────────────────────

/// 根據 event_type + date_status 產生對應的通知時點。
/// 回傳 Vec<(intent, scheduled_at_iso8601)>。
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

    // 使用本地時間計算（避免 UTC 跨日問題）
    let today = Local::now().date_naive();
    let mut schedule: Vec<(String, String)> = Vec::new();

    // range / month_only 類型提示用戶 confirm_date
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

    // event_time 是否明確存在
    let has_time = event_time.is_some();
    let time = event_time
        .and_then(|t| NaiveTime::parse_from_str(t, "%H:%M").ok())
        .unwrap_or(daily_time);

    match event_type {
        "meeting" => {
            let prepare_date = date - Duration::days(1);
            schedule.push(("prepare".to_string(), naive_local_to_utc_str(prepare_date.and_time(daily_time))));
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
                schedule.push(("start".to_string(), naive_local_to_utc_str(start_date.and_time(daily_time))));
            }
            if days_until >= 3 {
                let mid_date = date - Duration::days(3);
                schedule.push(("midcheck".to_string(), naive_local_to_utc_str(mid_date.and_time(daily_time))));
            }
            if days_until >= 1 {
                let urgent_date = date - Duration::days(1);
                schedule.push(("urgent".to_string(), naive_local_to_utc_str(urgent_date.and_time(daily_time))));
            }
            schedule.push(("final".to_string(), naive_local_to_utc_str(date.and_time(daily_time))));
        }
        "appointment" => {
            let prepare_date = date - Duration::days(1);
            schedule.push(("prepare".to_string(), naive_local_to_utc_str(prepare_date.and_time(daily_time))));
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
                schedule.push(("start".to_string(), naive_local_to_utc_str(start_date.and_time(daily_time))));
            }
            schedule.push(("final".to_string(), naive_local_to_utc_str(date.and_time(daily_time))));
        }
    }

    filter_future(schedule)
}

/// 過濾掉已經過去的時間點
fn filter_future(schedule: Vec<(String, String)>) -> Vec<(String, String)> {
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    schedule
        .into_iter()
        .filter(|(_, scheduled_at)| scheduled_at.as_str() > now.as_str())
        .collect()
}

/// 將本地 NaiveDateTime 轉換為 UTC ISO-8601 字串
fn naive_local_to_utc_str(naive_local: NaiveDateTime) -> String {
    let offset_secs = Local::now().offset().local_minus_utc();
    let utc_naive = naive_local - Duration::seconds(offset_secs as i64);
    // 使用與其它系統一致的格式：加 Z 且無毫秒
    format!("{}Z", utc_naive.format("%Y-%m-%dT%H:%M:%S"))
}

/// 當緊急提取時：若該事件在今天且未過時，
/// 但正式 schedule 已過（因 filter_future 過濾），則
/// 產生一個 "imminent" 通知放在 event_time - 5 分鐘（或是即刻 +30 秒）。
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

        // 如果事件已經過期超過 10 分鐘，不再發送通知
        if event_local + Duration::minutes(10) <= local_now.naive_local() {
            return None;
        }

        let remind_local = event_local - Duration::minutes(5);
        if remind_local <= local_now.naive_local() {
            // 已離當前太近或已過 5 分鐘內，立即發送
            let utc_at = Utc::now() - Duration::seconds(1);
            Some(("imminent".to_string(), utc_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)))
        } else {
            Some(("imminent".to_string(), naive_local_to_utc_str(remind_local)))
        }
    } else {
        // 未指定時間，預設為即刻發送
        let utc_at = Utc::now() - Duration::seconds(1);
        Some(("imminent".to_string(), utc_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)))
    }
}
