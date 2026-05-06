use chrono::{Duration, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use regex::Regex;
use serde_json::Value;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::collections::HashSet;
use std::sync::OnceLock;
use uuid::Uuid;

use crate::prompts;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::settings::store::get_settings;

pub struct ReminderEngine {
    pool: SqlitePool,
}

const REMINDER_EXTRACTION_BATCH_SIZE: usize = 8;
const REMINDER_EXTRACTION_BATCH_TRIGGER: usize = 12;
const REMINDER_EXTRACTION_MAX_TOKENS: usize = 1600;

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
            format!(
                "Reminder status updated\n\nTitle: {}\nStatus: {}",
                title, status
            )
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

    pub async fn create_reminder(
        &self,
        title: &str,
        description: Option<&str>,
        event_type: &str,
        event_date: &str,
        event_time_raw: Option<&str>,
    ) -> Result<String, String> {
        let title = title.trim();
        if title.is_empty() {
            return Err("Title is required".to_string());
        }

        if !matches!(
            event_type,
            "meeting" | "deliverable" | "event" | "appointment"
        ) {
            return Err("Invalid event type".to_string());
        }

        NaiveDate::parse_from_str(event_date, "%Y-%m-%d")
            .map_err(|_| "Invalid event date".to_string())?;

        let (event_time, event_time_parse_failed) =
            normalize_event_time_with_status(event_time_raw.unwrap_or(""));
        if event_time_parse_failed {
            return Err("Invalid event time".to_string());
        }

        let settings = get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let daily_time = &settings.reminders.daily_reminder_time;
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        let duplicate_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM reminders \
             WHERE title = ? AND event_type = ? AND event_date = ? \
             AND ((event_time IS NULL AND ? IS NULL) OR event_time = ?) \
             AND status = 'active'",
        )
        .bind(title)
        .bind(event_type)
        .bind(event_date)
        .bind(event_time.as_deref())
        .bind(event_time.as_deref())
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        if duplicate_count > 0 {
            return Err("Duplicate reminder already exists".to_string());
        }

        let description = description.map(str::trim).filter(|value| !value.is_empty());

        let mut schedule = generate_notification_schedule(
            event_type,
            "confirmed",
            event_date,
            event_time.as_deref(),
            daily_time,
        );
        if schedule.is_empty() {
            if let Some(fallback) = generate_imminent_fallback(event_date, event_time.as_deref()) {
                schedule.push(fallback);
            }
        }
        let unschedulable_reason = determine_unschedulable_reason(event_date, &schedule);
        let pending_confirm = if unschedulable_reason.is_some() { 1 } else { 0 };
        let final_description =
            merge_description_with_reason(description, unschedulable_reason.as_deref());

        if let Some(existing_id) =
            find_reschedule_target_tx(&mut tx, None, title, event_type, event_date, 90).await?
        {
            sqlx::query(
                "UPDATE reminders SET title = ?, description = ?, event_type = ?, date_status = 'confirmed', \
                 event_date = ?, event_date_end = NULL, event_time = ?, confidence = 1.0, pending_confirm = ?, updated_at = ? \
                 WHERE id = ?",
            )
            .bind(title)
            .bind(final_description.as_deref())
            .bind(event_type)
            .bind(event_date)
            .bind(event_time.as_deref())
            .bind(pending_confirm)
            .bind(&now)
            .bind(&existing_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

            sqlx::query(
                "DELETE FROM reminder_notifications WHERE reminder_id = ? AND sent_at IS NULL",
            )
            .bind(&existing_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

            insert_reminder_notifications_tx(&mut tx, &existing_id, &schedule, &now).await?;
            tx.commit().await.map_err(|e| e.to_string())?;
            return Ok(existing_id);
        }

        let reminder_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO reminders (id, conversation_id, space_id, project_id, title, description, \
             event_type, date_status, event_date, event_date_end, event_time, confidence, \
             status, pending_confirm, created_at, updated_at) \
             VALUES (?, NULL, NULL, NULL, ?, ?, ?, 'confirmed', ?, NULL, ?, 1.0, 'active', ?, ?, ?)",
        )
        .bind(&reminder_id)
        .bind(title)
        .bind(final_description.as_deref())
        .bind(event_type)
        .bind(event_date)
        .bind(event_time.as_deref())
        .bind(pending_confirm)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        insert_reminder_notifications_tx(&mut tx, &reminder_id, &schedule, &now).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(reminder_id)
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
        let json = self
            .extract_reminder_json(
                &primary_cfg,
                &fallback_cfg,
                conversation_timestamp,
                summary,
                dialogue,
            )
            .await?;

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

        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;

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
            let description = item["description"].as_str().unwrap_or("").trim();

            if status_req == "cancelled" || status_req == "completed" {
                let db_status = if status_req == "cancelled" {
                    "dismissed"
                } else {
                    "completed"
                };
                let rows_affected = update_existing_reminder_status_tx(
                    &mut tx,
                    Some(conversation_id),
                    title,
                    event_type,
                    if event_date.is_empty() {
                        None
                    } else {
                        Some(event_date.as_str())
                    },
                    db_status,
                    &now,
                    90,
                )
                .await?;

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

            let event_date_end = item["event_date_end"]
                .as_str()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());
            let confidence = item["confidence"]
                .as_f64()
                .map(|f| f.clamp(0.0, 1.0) as f32)
                .unwrap_or(0.7);
            let reminder_id = upsert_extracted_reminder_tx(
                &mut tx,
                conversation_id,
                valid_project_id,
                title,
                description,
                event_type,
                date_status,
                if event_date.is_empty() {
                    None
                } else {
                    Some(event_date.as_str())
                },
                event_date_end,
                event_time.as_deref(),
                event_time_parse_failed,
                confidence,
                daily_time,
                &now,
            )
            .await?;

            let pending_confirm: i32 =
                sqlx::query_scalar("SELECT pending_confirm FROM reminders WHERE id = ?")
                    .bind(&reminder_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;

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

        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(created_ids)
    }

    async fn extract_reminder_json(
        &self,
        primary_cfg: &crate::settings::store::ModelSettings,
        fallback_cfg: &crate::settings::store::ModelSettings,
        conversation_timestamp: &str,
        summary: &str,
        dialogue: &str,
    ) -> Result<serde_json::Value, String> {
        let candidate_items = collect_reminder_candidate_items(dialogue);
        if candidate_items.len() <= REMINDER_EXTRACTION_BATCH_TRIGGER {
            return self
                .extract_reminder_json_single_pass(
                    primary_cfg,
                    fallback_cfg,
                    conversation_timestamp,
                    summary,
                    dialogue,
                )
                .await;
        }

        let mut pending_chunks = chunk_reminder_candidate_items(&candidate_items);
        println!(
            "[ReminderEngine] Large reminder batch detected, split into {} extraction chunks.",
            pending_chunks.len()
        );

        let initial_chunk_count = pending_chunks.len();
        let mut processed_chunks = 0usize;
        let mut merged = Vec::new();

        while let Some(chunk_items) = pending_chunks.pop() {
            let dialogue_chunk = chunk_items.join("\n");
            let expected_count = chunk_items.len();
            let json = self
                .extract_reminder_json_single_pass(
                    primary_cfg,
                    fallback_cfg,
                    conversation_timestamp,
                    summary,
                    &dialogue_chunk,
                )
                .await?;
            let llm_items = json["reminders"].as_array().cloned().unwrap_or_default();
            let items = merge_reminder_items(
                llm_items,
                parse_structured_reminder_candidate_items(&chunk_items),
            );
            let actual_count = items.len();

            if should_split_extraction_chunk(expected_count, actual_count) {
                let midpoint = expected_count / 2;
                if midpoint > 0 {
                    println!(
                        "[ReminderEngine] Chunk undercount detected (expected ~{}, got {}), splitting into {} + {}.",
                        expected_count,
                        actual_count,
                        midpoint,
                        expected_count - midpoint
                    );
                    pending_chunks.push(chunk_items[midpoint..].to_vec());
                    pending_chunks.push(chunk_items[..midpoint].to_vec());
                    continue;
                }
            }

            processed_chunks += 1;
            println!(
                "[ReminderEngine] Extraction chunk {}/{} returned {} reminders.",
                processed_chunks, initial_chunk_count, actual_count
            );
            merged.extend(items);
        }

        Ok(serde_json::json!({
            "reminders": dedupe_reminder_items(merged)
        }))
    }

    async fn extract_reminder_json_single_pass(
        &self,
        primary_cfg: &crate::settings::store::ModelSettings,
        fallback_cfg: &crate::settings::store::ModelSettings,
        conversation_timestamp: &str,
        summary: &str,
        dialogue: &str,
    ) -> Result<serde_json::Value, String> {
        let result = self
            .attempt_extraction(primary_cfg, conversation_timestamp, summary, dialogue)
            .await;

        match result {
            Ok(json) => Ok(json),
            Err(e) => {
                println!(
                    "[ReminderEngine] Primary extraction failed ({}), trying fallback model...",
                    e
                );
                fallback_extraction_or_empty(
                    self.attempt_extraction(
                        fallback_cfg,
                        conversation_timestamp,
                        summary,
                        dialogue,
                    )
                    .await,
                    e,
                )
            }
        }
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
            max_tokens: REMINDER_EXTRACTION_MAX_TOKENS,
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
        let pending_rows = sqlx::query(
            "SELECT id, scheduled_at FROM reminder_notifications \
             WHERE reminder_id = ? AND sent_at IS NULL \
             ORDER BY datetime(scheduled_at) ASC",
        )
        .bind(reminder_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        if !pending_rows.is_empty() {
            let latest_sent_at: Option<String> = sqlx::query_scalar(
                "SELECT scheduled_at FROM reminder_notifications \
                 WHERE reminder_id = ? AND sent_at IS NOT NULL \
                 ORDER BY datetime(scheduled_at) DESC LIMIT 1",
            )
            .bind(reminder_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

            let latest_sent_dt = latest_sent_at.as_deref().and_then(parse_rfc3339_utc_to_utc);

            for row in pending_rows {
                let notif_id: String = row.get("id");
                let scheduled_at: String = row.get("scheduled_at");
                let Some(old_dt) = parse_rfc3339_utc_to_utc(&scheduled_at) else {
                    continue;
                };
                let mut new_dt = old_dt + Duration::minutes(minutes);
                if let Some(latest_sent_dt) = latest_sent_dt {
                    if new_dt <= latest_sent_dt {
                        new_dt = latest_sent_dt + Duration::seconds(1);
                    }
                }

                sqlx::query("UPDATE reminder_notifications SET scheduled_at = ? WHERE id = ?")
                    .bind(new_dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                    .bind(&notif_id)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| e.to_string())?;
            }
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

fn fallback_extraction_or_empty(
    fallback_result: Result<serde_json::Value, String>,
    primary_error: String,
) -> Result<serde_json::Value, String> {
    fallback_result
        .map_err(|e2| {
            format!(
                "All reminder extraction attempts failed: {}; {}",
                primary_error, e2
            )
        })
        .or_else(|combined_error| {
            eprintln!(
                "[ReminderEngine] Emergency reminder extraction fallback failed: {}",
                combined_error
            );
            Ok(serde_json::json!({ "reminders": [] }))
        })
}

#[cfg(test)]
fn build_reminder_extraction_dialogue_chunks(dialogue: &str) -> Vec<String> {
    let trimmed = dialogue.trim();
    if trimmed.is_empty() {
        return vec![String::new()];
    }

    let items = collect_reminder_candidate_items(dialogue);
    if items.len() <= REMINDER_EXTRACTION_BATCH_TRIGGER {
        return vec![trimmed.to_string()];
    }

    chunk_reminder_candidate_items(&items)
        .into_iter()
        .map(|chunk| chunk.join("\n"))
        .collect()
}

fn collect_reminder_candidate_items(dialogue: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = Vec::new();

    for raw_line in dialogue.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                items.push(current.join("\n"));
                current.clear();
            }
            continue;
        }

        if is_reminder_candidate_start(line) {
            if !current.is_empty() {
                items.push(current.join("\n"));
                current.clear();
            }
            current.push(line.to_string());
        } else if !current.is_empty() {
            current.push(line.to_string());
        }
    }

    if !current.is_empty() {
        items.push(current.join("\n"));
    }

    items
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .collect()
}

fn is_reminder_candidate_start(line: &str) -> bool {
    let candidate = strip_speaker_prefix(line);
    reminder_candidate_start_regex().is_match(candidate)
        || reminder_date_regex().is_match(candidate)
}

fn strip_speaker_prefix(line: &str) -> &str {
    line.strip_prefix("User:")
        .or_else(|| line.strip_prefix("Assistant:"))
        .or_else(|| line.strip_prefix("user:"))
        .or_else(|| line.strip_prefix("assistant:"))
        .map(str::trim)
        .unwrap_or(line)
}

fn reminder_candidate_start_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^\s*(?:[-*•]\s+|\d+[\.\)、:：-]\s*|\(?\d+\)\s*|\d{4}[-/]\d{1,2}[-/]\d{1,2}\b)")
            .expect("valid reminder candidate regex")
    })
}

fn reminder_date_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\b\d{4}[-/]\d{1,2}[-/]\d{1,2}\b").expect("valid reminder date regex")
    })
}

fn dedupe_reminder_items(items: Vec<Value>) -> Vec<Value> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for item in items {
        if let Some(key) = reminder_item_key(&item) {
            if !seen.insert(key) {
                continue;
            }
        }
        deduped.push(item);
    }

    deduped
}

fn normalize_reminder_title(title: &str) -> String {
    title
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn is_within_reschedule_window(
    existing_event_date: Option<&str>,
    new_event_date: &str,
    window_days: i64,
) -> bool {
    let Some(existing_str) = existing_event_date else {
        return true;
    };
    let Ok(existing_date) = NaiveDate::parse_from_str(existing_str, "%Y-%m-%d") else {
        return true;
    };
    let Ok(new_date) = NaiveDate::parse_from_str(new_event_date, "%Y-%m-%d") else {
        return true;
    };

    (existing_date - new_date).num_days().abs() <= window_days
}

fn determine_unschedulable_reason(
    event_date: &str,
    schedule: &[(String, String)],
) -> Option<String> {
    if !schedule.is_empty() {
        return None;
    }

    match NaiveDate::parse_from_str(event_date, "%Y-%m-%d") {
        Ok(date) if date < Local::now().date_naive() => {
            Some("schedule_unavailable:event_date_in_past".to_string())
        }
        Ok(_) => Some("schedule_unavailable:no_future_slot".to_string()),
        Err(_) => Some("schedule_unavailable:no_future_slot".to_string()),
    }
}

fn merge_description_with_reason(
    description: Option<&str>,
    reason: Option<&str>,
) -> Option<String> {
    match (description, reason) {
        (Some(desc), Some(reason)) if !desc.contains(reason) => {
            Some(format!("{}\n{}", desc.trim(), reason))
        }
        (Some(desc), _) => Some(desc.trim().to_string()),
        (None, Some(reason)) => Some(reason.to_string()),
        (None, None) => None,
    }
}

fn parse_rfc3339_utc_to_utc(raw: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

async fn find_reschedule_target_tx(
    tx: &mut Transaction<'_, Sqlite>,
    conversation_id: Option<&str>,
    title: &str,
    event_type: &str,
    new_event_date: &str,
    window_days: i64,
) -> Result<Option<String>, String> {
    let rows = if let Some(conversation_id) = conversation_id {
        sqlx::query(
            "SELECT id, title, event_date FROM reminders \
             WHERE conversation_id = ? AND event_type = ? AND status = 'active'",
        )
        .bind(conversation_id)
        .bind(event_type)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query(
            "SELECT id, title, event_date FROM reminders \
             WHERE conversation_id IS NULL AND event_type = ? AND status = 'active'",
        )
        .bind(event_type)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| e.to_string())?
    };

    let normalized_title = normalize_reminder_title(title);
    for row in rows {
        let candidate_title: String = row.get("title");
        let candidate_date: Option<String> = row.get("event_date");
        if normalize_reminder_title(&candidate_title) != normalized_title {
            continue;
        }
        if !is_within_reschedule_window(candidate_date.as_deref(), new_event_date, window_days) {
            continue;
        }
        let id: String = row.get("id");
        return Ok(Some(id));
    }

    Ok(None)
}

async fn update_existing_reminder_status_tx(
    tx: &mut Transaction<'_, Sqlite>,
    conversation_id: Option<&str>,
    title: &str,
    event_type: &str,
    event_date: Option<&str>,
    status: &str,
    now: &str,
    window_days: i64,
) -> Result<u64, String> {
    let Some(reminder_id) = find_reschedule_target_tx(
        tx,
        conversation_id,
        title,
        event_type,
        event_date.unwrap_or(""),
        window_days,
    )
    .await?
    else {
        return Ok(0);
    };

    let rows_affected = sqlx::query("UPDATE reminders SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(now)
        .bind(&reminder_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();

    sqlx::query("DELETE FROM reminder_notifications WHERE reminder_id = ? AND sent_at IS NULL")
        .bind(&reminder_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows_affected)
}

#[allow(clippy::too_many_arguments)]
async fn upsert_extracted_reminder_tx(
    tx: &mut Transaction<'_, Sqlite>,
    conversation_id: &str,
    project_id: Option<&str>,
    title: &str,
    description: &str,
    event_type: &str,
    date_status: &str,
    event_date: Option<&str>,
    event_date_end: Option<&str>,
    event_time: Option<&str>,
    event_time_parse_failed: bool,
    confidence: f32,
    daily_time: &str,
    now: &str,
) -> Result<String, String> {
    let event_date_str = event_date.unwrap_or("");
    let mut schedule = if !event_date_str.is_empty() {
        generate_notification_schedule(
            event_type,
            date_status,
            event_date_str,
            event_time,
            daily_time,
        )
    } else {
        Vec::new()
    };
    if schedule.is_empty() && !event_time_parse_failed {
        if let Some(date) = event_date {
            if let Some(fallback) = generate_imminent_fallback(date, event_time) {
                schedule.push(fallback);
            }
        }
    }

    let unschedulable_reason =
        event_date.and_then(|date| determine_unschedulable_reason(date, &schedule));
    let pending_confirm = if confidence < 0.49 || unschedulable_reason.is_some() {
        1_i32
    } else {
        0_i32
    };
    let final_description =
        merge_description_with_reason(Some(description), unschedulable_reason.as_deref());

    let existing_id = find_reschedule_target_tx(
        tx,
        Some(conversation_id),
        title,
        event_type,
        event_date.unwrap_or(""),
        90,
    )
    .await?;

    if let Some(existing_id) = existing_id {
        sqlx::query(
            "UPDATE reminders SET project_id = ?, title = ?, description = ?, event_type = ?, date_status = ?, \
             event_date = ?, event_date_end = ?, event_time = ?, confidence = ?, pending_confirm = ?, updated_at = ? \
             WHERE id = ?",
        )
        .bind(project_id)
        .bind(title)
        .bind(final_description.as_deref())
        .bind(event_type)
        .bind(date_status)
        .bind(event_date)
        .bind(event_date_end)
        .bind(event_time)
        .bind(confidence)
        .bind(pending_confirm)
        .bind(now)
        .bind(&existing_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM reminder_notifications WHERE reminder_id = ? AND sent_at IS NULL")
            .bind(&existing_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;

        insert_reminder_notifications_tx(tx, &existing_id, &schedule, now).await?;
        return Ok(existing_id);
    }

    let duplicate_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reminders \
         WHERE conversation_id = ? AND title = ? AND event_type = ? \
         AND ((event_date IS NULL AND ? IS NULL) OR event_date = ?) \
         AND ((event_time IS NULL AND ? IS NULL) OR event_time = ?) \
         AND status = 'active'",
    )
    .bind(conversation_id)
    .bind(title)
    .bind(event_type)
    .bind(event_date)
    .bind(event_date)
    .bind(event_time)
    .bind(event_time)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    if duplicate_count > 0 {
        let existing_id: String = sqlx::query_scalar(
            "SELECT id FROM reminders \
             WHERE conversation_id = ? AND title = ? AND event_type = ? \
             AND ((event_date IS NULL AND ? IS NULL) OR event_date = ?) \
             AND ((event_time IS NULL AND ? IS NULL) OR event_time = ?) \
             AND status = 'active' LIMIT 1",
        )
        .bind(conversation_id)
        .bind(title)
        .bind(event_type)
        .bind(event_date)
        .bind(event_date)
        .bind(event_time)
        .bind(event_time)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
        return Ok(existing_id);
    }

    let reminder_id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO reminders (id, conversation_id, space_id, project_id, title, description, \
         event_type, date_status, event_date, event_date_end, event_time, confidence, \
         status, pending_confirm, created_at, updated_at) \
         VALUES (?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?)",
    )
    .bind(&reminder_id)
    .bind(conversation_id)
    .bind(project_id)
    .bind(title)
    .bind(final_description.as_deref())
    .bind(event_type)
    .bind(date_status)
    .bind(event_date)
    .bind(event_date_end)
    .bind(event_time)
    .bind(confidence)
    .bind(pending_confirm)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    insert_reminder_notifications_tx(tx, &reminder_id, &schedule, now).await?;
    Ok(reminder_id)
}

async fn insert_reminder_notifications_tx(
    tx: &mut Transaction<'_, Sqlite>,
    reminder_id: &str,
    schedule: &[(String, String)],
    now: &str,
) -> Result<(), String> {
    for (intent, scheduled_at) in schedule {
        let notif_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO reminder_notifications (id, reminder_id, intent, scheduled_at, channel, created_at) \
             VALUES (?, ?, ?, ?, 'both', ?)",
        )
        .bind(&notif_id)
        .bind(reminder_id)
        .bind(intent)
        .bind(scheduled_at)
        .bind(now)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn chunk_reminder_candidate_items(items: &[String]) -> Vec<Vec<String>> {
    items
        .chunks(REMINDER_EXTRACTION_BATCH_SIZE)
        .map(|chunk| chunk.to_vec())
        .collect()
}

fn should_split_extraction_chunk(expected_count: usize, actual_count: usize) -> bool {
    expected_count > 1 && (actual_count == 0 || actual_count + 1 < expected_count)
}

fn merge_reminder_items(primary: Vec<Value>, fallback: Vec<Value>) -> Vec<Value> {
    let mut merged = primary;
    merged.extend(fallback);
    dedupe_reminder_items(merged)
}

fn parse_structured_reminder_candidate_items(items: &[String]) -> Vec<Value> {
    items
        .iter()
        .filter_map(|item| parse_structured_reminder_candidate_item(item))
        .collect()
}

fn parse_structured_reminder_candidate_item(item: &str) -> Option<Value> {
    let line = item.lines().next()?.trim();
    let line = strip_speaker_prefix(line);
    let captures = structured_reminder_line_regex().captures(line)?;

    let event_date = captures.name("date")?.as_str().replace('/', "-");
    let title = captures.name("title")?.as_str().trim();
    if title.is_empty() {
        return None;
    }

    let event_time = captures
        .name("time")
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    Some(serde_json::json!({
        "title": title,
        "event_date": event_date,
        "event_date_end": "",
        "event_time": event_time,
        "date_status": "confirmed",
        "event_type": "event",
        "description": title,
        "status": "active",
        "confidence": 0.85
    }))
}

fn structured_reminder_line_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"^\s*(?:[-*•]\s+|\d+[\.\)、:：-]\s*|\(?\d+\)\s*)?(?P<date>\d{4}[-/]\d{1,2}[-/]\d{1,2})(?:\s+(?P<time>\d{1,2}:\d{2}))?\s+(?P<title>.+?)\s*$"#,
        )
        .expect("valid structured reminder regex")
    })
}

fn reminder_item_key(item: &Value) -> Option<String> {
    let title = item.get("title")?.as_str()?.trim();
    if title.is_empty() {
        return None;
    }

    let event_date = item
        .get("event_date")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let event_time = item
        .get("event_time")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let status = item
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("active")
        .trim();

    Some(format!(
        "{}|{}|{}|{}",
        title.to_lowercase(),
        event_date,
        event_time,
        status
    ))
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

    if let Some(items) = try_parse_partial_reminder_items(text) {
        return Some(serde_json::json!({ "reminders": items }));
    }

    None
}

fn try_parse_partial_reminder_items(text: &str) -> Option<Vec<serde_json::Value>> {
    let key_pos = text.find("\"reminders\"")?;
    let bracket_start_rel = text[key_pos..].find('[')?;
    let bracket_start = key_pos + bracket_start_rel;
    let slice = &text[bracket_start..];
    let mut items = Vec::new();
    let mut object_start = None;
    let mut object_depth = 0_i32;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in slice.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if object_depth == 0 {
                    object_start = Some(idx);
                }
                object_depth += 1;
            }
            '}' => {
                if object_depth == 0 {
                    continue;
                }
                object_depth -= 1;
                if object_depth == 0 {
                    if let Some(start) = object_start.take() {
                        let candidate = &slice[start..=idx];
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(candidate) {
                            items.push(value);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_reminder_extraction_dialogue_chunks, dedupe_reminder_items,
        generate_imminent_fallback, is_empty_raw_parse_error, merge_reminder_items,
        naive_local_to_utc_str, normalize_event_time, normalize_event_time_with_status,
        parse_structured_reminder_candidate_items,
    };
    use crate::db::connection::init_db;
    use chrono::{Duration, Local, NaiveDate, NaiveTime, Utc};
    use serde_json::json;
    use sqlx::SqlitePool;
    use tempfile::tempdir;

    async fn setup_test_engine() -> (super::ReminderEngine, SqlitePool, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let kb_path = dir.path().join("kb");
        std::fs::create_dir_all(&kb_path).expect("create kb dir");
        let pool = init_db(&kb_path, None).await.expect("init db");
        let engine = super::ReminderEngine::new(pool.clone());
        (engine, pool, dir)
    }

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

    #[test]
    fn test_build_reminder_extraction_dialogue_chunks_splits_large_batch() {
        let mut dialogue = String::new();
        for i in 1..=18 {
            if !dialogue.is_empty() {
                dialogue.push('\n');
            }
            dialogue.push_str(&format!("{i}. 2027-10-{i:02} 完成任務 {i}"));
        }

        let chunks = build_reminder_extraction_dialogue_chunks(&dialogue);
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].contains("1. 2027-10-01"));
        assert!(chunks[1].contains("9. 2027-10-09"));
        assert!(chunks[2].contains("17. 2027-10-17"));
    }

    #[test]
    fn test_build_reminder_extraction_dialogue_chunks_keeps_small_batch() {
        let dialogue =
            "1. 2027-10-01 完成任務 1\n2. 2027-10-02 完成任務 2\n3. 2027-10-03 完成任務 3";
        let chunks = build_reminder_extraction_dialogue_chunks(dialogue);
        assert_eq!(chunks, vec![dialogue.to_string()]);
    }

    #[test]
    fn test_dedupe_reminder_items_removes_duplicate_entries() {
        let items = vec![
            json!({
                "title": "提交報告",
                "event_date": "2027-10-01",
                "event_time": "09:00",
                "status": "active"
            }),
            json!({
                "title": "提交報告",
                "event_date": "2027-10-01",
                "event_time": "09:00",
                "status": "active"
            }),
            json!({
                "title": "提交報告",
                "event_date": "2027-10-01",
                "event_time": "10:00",
                "status": "active"
            }),
        ];

        let deduped = dedupe_reminder_items(items);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_try_parse_reminder_json_salvages_truncated_array() {
        let raw = r#"{
  "reminders": [
    {
      "title": "Batch Reminder #9",
      "event_date": "2027-10-09",
      "event_date_end": "",
      "event_time": "09:00",
      "date_status": "confirmed",
      "event_type": "event",
      "description": "Batch Reminder #9",
      "status": "active",
      "confidence": 1.0
    },
    {
      "title": "Batch Reminder #10",
      "event_date": "2027-10-10",
      "event_date_end": "",
      "event_time": "09:00",
      "date_status": "confirmed",
      "event_type": "event",
      "description": "Batch Reminder #10",
      "status": "active",
      "confidence": 1.0
    },
    {
      "title": "Batch Reminder #11",
      "event_date": "2027-10-11",
      "event_date_end": "",
      "event_time": "09:00",
      "date_status": "confirmed",
      "event_type": "event",
      "description": "Batch Reminder #11",
      "status": "active",
      "confidence": 1.0
    },
    {
      "title": "Batch Reminder #12",
      "event_date": "2027-10-12",
      "event_date_end": "",
      "event_time": "09:00",
      "date_status": "confirmed",
      "event_type": "event",
      "description": "Batch Reminder #12",
      "status": "active",
      "confidence": 1.0
    },
    {
      "title": "Batch Reminder #13",
      "event_date": "2027-10-13",
      "event_date_end": "",
      "event_time": "09:00",
      "date_status": "confirmed",
      "event_type": "event",
      "description": "Batch Reminder #13",
      "status": "active",
      "confidence": 1.0
    }"#;

        let parsed = super::try_parse_reminder_json(raw).expect("should salvage partial array");
        let reminders = parsed["reminders"].as_array().unwrap();
        assert_eq!(reminders.len(), 5);
        assert_eq!(reminders[0]["title"], "Batch Reminder #9");
        assert_eq!(reminders[4]["title"], "Batch Reminder #13");
    }

    #[test]
    fn test_should_split_extraction_chunk_when_undercounted() {
        assert!(super::should_split_extraction_chunk(8, 5));
        assert!(super::should_split_extraction_chunk(4, 0));
        assert!(!super::should_split_extraction_chunk(4, 4));
        assert!(!super::should_split_extraction_chunk(4, 3));
    }

    #[test]
    fn test_parse_structured_reminder_candidate_items_extracts_all_entries() {
        let items = vec![
            "1. 2027-10-01 09:00 Batch Reminder #1".to_string(),
            "2. 2027/10/02 10:30 Batch Reminder #2".to_string(),
            "3. 2027-10-03 Batch Reminder #3".to_string(),
        ];

        let parsed = parse_structured_reminder_candidate_items(&items);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0]["title"], "Batch Reminder #1");
        assert_eq!(parsed[0]["event_date"], "2027-10-01");
        assert_eq!(parsed[0]["event_time"], "09:00");
        assert_eq!(parsed[1]["event_date"], "2027-10-02");
        assert_eq!(parsed[1]["event_time"], "10:30");
        assert_eq!(parsed[2]["event_time"], "");
    }

    #[test]
    fn test_merge_reminder_items_fills_missing_llm_entries() {
        let llm_items = vec![
            json!({
                "title": "Batch Reminder #1",
                "event_date": "2027-10-01",
                "event_time": "09:00",
                "status": "active"
            }),
            json!({
                "title": "Batch Reminder #3",
                "event_date": "2027-10-03",
                "event_time": "09:00",
                "status": "active"
            }),
        ];
        let fallback_items = parse_structured_reminder_candidate_items(&[
            "1. 2027-10-01 09:00 Batch Reminder #1".to_string(),
            "2. 2027-10-02 09:00 Batch Reminder #2".to_string(),
            "3. 2027-10-03 09:00 Batch Reminder #3".to_string(),
        ]);

        let merged = merge_reminder_items(llm_items, fallback_items);
        assert_eq!(merged.len(), 3);
        assert!(merged
            .iter()
            .any(|item| item["title"] == "Batch Reminder #2"));
    }

    #[test]
    fn test_normalize_reminder_title_ignores_case_whitespace_and_punctuation() {
        assert_eq!(
            super::normalize_reminder_title(" Project-Review !! "),
            super::normalize_reminder_title("project review")
        );
        assert_eq!(
            super::normalize_reminder_title("提交：週報"),
            super::normalize_reminder_title("提交週報")
        );
    }

    #[test]
    fn test_is_within_reschedule_window_limits_match_range() {
        let base = "2027-10-01";
        assert!(super::is_within_reschedule_window(
            Some("2027-12-01"),
            base,
            90
        ));
        assert!(!super::is_within_reschedule_window(
            Some("2028-02-01"),
            base,
            90
        ));
        assert!(super::is_within_reschedule_window(None, base, 90));
    }

    #[tokio::test]
    async fn test_reschedule_request_should_replace_existing_active_reminder_instead_of_duplication(
    ) {
        let (engine, pool, _dir) = setup_test_engine().await;
        let first_date = (Local::now().date_naive() + Duration::days(10))
            .format("%Y-%m-%d")
            .to_string();
        let second_date = (Local::now().date_naive() + Duration::days(12))
            .format("%Y-%m-%d")
            .to_string();

        engine
            .create_reminder("Project Review", None, "event", &first_date, Some("09:00"))
            .await
            .expect("create first reminder");
        engine
            .create_reminder("Project Review", None, "event", &second_date, Some("09:00"))
            .await
            .expect("create rescheduled reminder");

        let active_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM reminders WHERE title = 'Project Review' AND status = 'active'",
        )
        .fetch_one(&pool)
        .await
        .expect("count active reminders");

        assert_eq!(
            active_count, 1,
            "延期應覆蓋原本 active reminder，而不是留下兩筆並存"
        );
    }

    #[tokio::test]
    async fn test_snooze_should_delay_all_pending_notifications_for_same_reminder() {
        let (engine, pool, _dir) = setup_test_engine().await;
        let event_date = (Local::now().date_naive() + Duration::days(10))
            .format("%Y-%m-%d")
            .to_string();

        let reminder_id = engine
            .create_reminder(
                "Quarterly Report",
                None,
                "deliverable",
                &event_date,
                Some("09:00"),
            )
            .await
            .expect("create deliverable reminder");

        let before: Vec<String> = sqlx::query_scalar(
            "SELECT scheduled_at FROM reminder_notifications WHERE reminder_id = ? ORDER BY datetime(scheduled_at) ASC",
        )
        .bind(&reminder_id)
        .fetch_all(&pool)
        .await
        .expect("load notifications before snooze");

        engine
            .snooze_reminder(&reminder_id, 60)
            .await
            .expect("snooze reminder");

        let after: Vec<String> = sqlx::query_scalar(
            "SELECT scheduled_at FROM reminder_notifications WHERE reminder_id = ? ORDER BY datetime(scheduled_at) ASC",
        )
        .bind(&reminder_id)
        .fetch_all(&pool)
        .await
        .expect("load notifications after snooze");

        assert_eq!(before.len(), after.len(), "Snooze 不應改變通知數量");
        assert!(
            before.iter().zip(after.iter()).all(|(old, new)| old != new),
            "延期後，所有相關 notifications 都應一起順延"
        );
    }

    #[tokio::test]
    async fn test_unschedulable_reminder_should_not_succeed_silently_without_followup_mechanism() {
        let (engine, pool, _dir) = setup_test_engine().await;
        let past_date = (Local::now().date_naive() - Duration::days(2))
            .format("%Y-%m-%d")
            .to_string();

        let result = engine
            .create_reminder(
                "Past-due Follow-up",
                None,
                "event",
                &past_date,
                Some("09:00"),
            )
            .await;

        match result {
            Err(_) => {}
            Ok(reminder_id) => {
                let pending_confirm: i32 =
                    sqlx::query_scalar("SELECT pending_confirm FROM reminders WHERE id = ?")
                        .bind(&reminder_id)
                        .fetch_one(&pool)
                        .await
                        .expect("load pending_confirm");

                let pending_notifications: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM reminder_notifications WHERE reminder_id = ? AND sent_at IS NULL",
                )
                .bind(&reminder_id)
                .fetch_one(&pool)
                .await
                .expect("count pending notifications");

                assert!(
                    pending_confirm == 1 || pending_notifications > 0,
                    "無法安排的 reminder 不應靜默成功：至少要 pending_confirm 或建立 fallback notification"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_upsert_extracted_reminder_tx_reschedules_same_conversation_reminder() {
        let (engine, pool, _dir) = setup_test_engine().await;
        let conversation_id = "conv-reschedule-1";
        sqlx::query(
            "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?, 'Test', '2026-04-27T10:00:00Z', '2026-04-27T10:00:00Z')",
        )
        .bind(conversation_id)
        .execute(&pool)
        .await
        .expect("insert conversation");

        let first_date = (Local::now().date_naive() + Duration::days(10))
            .format("%Y-%m-%d")
            .to_string();
        let second_date = (Local::now().date_naive() + Duration::days(12))
            .format("%Y-%m-%d")
            .to_string();
        let reminder_id = engine
            .create_reminder("Project Review", None, "event", &first_date, Some("09:00"))
            .await
            .expect("create base reminder");

        sqlx::query("UPDATE reminders SET conversation_id = ? WHERE id = ?")
            .bind(conversation_id)
            .bind(&reminder_id)
            .execute(&pool)
            .await
            .expect("attach conversation id");

        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let mut tx = pool.begin().await.expect("begin tx");
        let updated_id = super::upsert_extracted_reminder_tx(
            &mut tx,
            conversation_id,
            None,
            "Project Review",
            "rescheduled",
            "event",
            "confirmed",
            Some(second_date.as_str()),
            None,
            Some("09:00"),
            false,
            0.9,
            "09:00",
            &now,
        )
        .await
        .expect("upsert extracted");
        tx.commit().await.expect("commit tx");

        assert_eq!(updated_id, reminder_id);

        let active_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM reminders WHERE conversation_id = ? AND title = 'Project Review' AND status = 'active'",
        )
        .bind(conversation_id)
        .fetch_one(&pool)
        .await
        .expect("count active");
        assert_eq!(active_count, 1);

        let stored_date: Option<String> =
            sqlx::query_scalar("SELECT event_date FROM reminders WHERE id = ?")
                .bind(&reminder_id)
                .fetch_one(&pool)
                .await
                .expect("load updated date");
        assert_eq!(stored_date.as_deref(), Some(second_date.as_str()));
    }

    #[tokio::test]
    async fn test_update_existing_reminder_status_tx_clears_pending_notifications() {
        let (engine, pool, _dir) = setup_test_engine().await;
        let conversation_id = "conv-cancel-1";
        sqlx::query(
            "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?, 'Test', '2026-04-27T10:00:00Z', '2026-04-27T10:00:00Z')",
        )
        .bind(conversation_id)
        .execute(&pool)
        .await
        .expect("insert conversation");

        let event_date = (Local::now().date_naive() + Duration::days(10))
            .format("%Y-%m-%d")
            .to_string();
        let reminder_id = engine
            .create_reminder("Cancel Me", None, "event", &event_date, Some("09:00"))
            .await
            .expect("create reminder");
        sqlx::query("UPDATE reminders SET conversation_id = ? WHERE id = ?")
            .bind(conversation_id)
            .bind(&reminder_id)
            .execute(&pool)
            .await
            .expect("attach conversation id");

        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let mut tx = pool.begin().await.expect("begin tx");
        let affected = super::update_existing_reminder_status_tx(
            &mut tx,
            Some(conversation_id),
            "Cancel Me",
            "event",
            Some(event_date.as_str()),
            "dismissed",
            &now,
            90,
        )
        .await
        .expect("update status");
        tx.commit().await.expect("commit tx");

        assert_eq!(affected, 1);

        let status: String = sqlx::query_scalar("SELECT status FROM reminders WHERE id = ?")
            .bind(&reminder_id)
            .fetch_one(&pool)
            .await
            .expect("load status");
        assert_eq!(status, "dismissed");

        let pending_notifications: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM reminder_notifications WHERE reminder_id = ? AND sent_at IS NULL",
        )
        .bind(&reminder_id)
        .fetch_one(&pool)
        .await
        .expect("count pending notifications");
        assert_eq!(pending_notifications, 0);
    }
}
