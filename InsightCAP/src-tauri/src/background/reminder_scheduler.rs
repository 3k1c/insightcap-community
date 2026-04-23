use std::time::Duration;

use chrono::{Datelike, NaiveTime, Utc, Weekday};
use sqlx::Row;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::sleep;

use tauri_plugin_notification::NotificationExt;

use crate::background::telegram_bot::send_message;
use crate::db::AppState;
use crate::settings::store::get_settings;

const POLL_INTERVAL_SECS: u64 = 60;

#[derive(Clone, Copy)]
enum ReminderLanguage {
    ZhTw,
    ZhCn,
    En,
}

impl ReminderLanguage {
    fn from_code(language: &str) -> Self {
        match language {
            "zh-CN" => Self::ZhCn,
            "en" => Self::En,
            _ => Self::ZhTw,
        }
    }
}

fn reminder_intent_label(lang: ReminderLanguage, intent: &str) -> &'static str {
    match (lang, intent) {
        (ReminderLanguage::ZhTw, "start") => "提醒：開始處理",
        (ReminderLanguage::ZhTw, "midcheck") => "提醒：進度檢查",
        (ReminderLanguage::ZhTw, "urgent") => "緊急提醒",
        (ReminderLanguage::ZhTw, "final") => "最後提醒",
        (ReminderLanguage::ZhTw, "prepare") => "準備提醒",
        (ReminderLanguage::ZhTw, "imminent") => "即將開始",
        (ReminderLanguage::ZhTw, "now") => "正在發生",
        (ReminderLanguage::ZhTw, "confirm_date") => "日期確認提醒",
        (ReminderLanguage::ZhTw, _) => "提醒",
        (ReminderLanguage::ZhCn, "start") => "提醒：开始处理",
        (ReminderLanguage::ZhCn, "midcheck") => "提醒：进度检查",
        (ReminderLanguage::ZhCn, "urgent") => "紧急提醒",
        (ReminderLanguage::ZhCn, "final") => "最后提醒",
        (ReminderLanguage::ZhCn, "prepare") => "准备提醒",
        (ReminderLanguage::ZhCn, "imminent") => "即将开始",
        (ReminderLanguage::ZhCn, "now") => "正在发生",
        (ReminderLanguage::ZhCn, "confirm_date") => "日期确认提醒",
        (ReminderLanguage::ZhCn, _) => "提醒",
        (ReminderLanguage::En, "start") => "Reminder: get started",
        (ReminderLanguage::En, "midcheck") => "Reminder: progress check",
        (ReminderLanguage::En, "urgent") => "Urgent reminder",
        (ReminderLanguage::En, "final") => "Final reminder",
        (ReminderLanguage::En, "prepare") => "Preparation reminder",
        (ReminderLanguage::En, "imminent") => "Imminent reminder",
        (ReminderLanguage::En, "now") => "Happening now",
        (ReminderLanguage::En, "confirm_date") => "Date confirmation reminder",
        (ReminderLanguage::En, _) => "Reminder",
    }
}

fn reminder_type_label(lang: ReminderLanguage, event_type: &str) -> &str {
    match (lang, event_type) {
        (ReminderLanguage::ZhTw, "meeting") => "會議",
        (ReminderLanguage::ZhTw, "deliverable") => "交付物",
        (ReminderLanguage::ZhTw, "event") => "事件",
        (ReminderLanguage::ZhTw, "appointment") => "預約",
        (ReminderLanguage::ZhCn, "meeting") => "会议",
        (ReminderLanguage::ZhCn, "deliverable") => "交付物",
        (ReminderLanguage::ZhCn, "event") => "事件",
        (ReminderLanguage::ZhCn, "appointment") => "预约",
        (_, _) => event_type,
    }
}

fn reminder_message(
    lang: ReminderLanguage,
    intent: &str,
    title: &str,
    time_str: &str,
    event_type: &str,
) -> String {
    let intent_display = reminder_intent_label(lang, intent);
    let type_display = reminder_type_label(lang, event_type);

    match lang {
        ReminderLanguage::ZhTw => format!(
            "[{}]\n標題：{}\n時間：{}\n類型：{}",
            intent_display, title, time_str, type_display
        ),
        ReminderLanguage::ZhCn => format!(
            "[{}]\n标题：{}\n时间：{}\n类型：{}",
            intent_display, title, time_str, type_display
        ),
        ReminderLanguage::En => format!(
            "[{}]\nTitle: {}\nTime: {}\nType: {}",
            intent_display, title, time_str, type_display
        ),
    }
}

pub fn start_reminder_scheduler(app: AppHandle) {
    let shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        println!("[ReminderScheduler] Worker started");
        if let Err(e) = process_due_notifications(&app, false).await {
            eprintln!("[ReminderScheduler] Initial check failed at startup: {}", e);
        }
        let mut shutdown_rx = shutdown_rx;
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[ReminderScheduler] Stop signal received, exiting...");
                        break;
                    }
                }
                _ = sleep(Duration::from_secs(POLL_INTERVAL_SECS)) => {
                    app.state::<crate::db::AppState>().reminder_loop_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    if let Err(e) = process_due_notifications(&app, false).await {
                        eprintln!("[ReminderScheduler] Processing failed: {}", e);
                    }
                }
            }
        }
    });
}

pub async fn process_due_notifications(app: &AppHandle, force: bool) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    let settings = get_settings(pool).await.map_err(|e| e.to_string())?;
    println!(
        "[ReminderScheduler] Checking pending notifications... reminders: {}, telegram: {}, force: {}",
        settings.reminders.enabled, settings.telegram.enabled, force
    );
    if !settings.reminders.enabled && !force {
        println!("[ReminderScheduler] Reminder feature disabled and not in force mode, skipping");
        return Ok(0);
    }

    if !force
        && is_quiet_hours(
            &settings.reminders.quiet_hours_start,
            &settings.reminders.quiet_hours_end,
            settings.reminders.weekend_quiet,
        )
    {
        println!("[ReminderScheduler] Currently in quiet hours, delaying notifications");
        return Ok(0);
    }

    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let rows = sqlx::query(
        "SELECT n.id, n.reminder_id, n.intent, n.scheduled_at, \
         r.title, r.event_type, r.date_status, r.event_date, r.event_time, r.pending_confirm \
         FROM reminder_notifications n \
         JOIN reminders r ON n.reminder_id = r.id \
         WHERE n.sent_at IS NULL AND datetime(n.scheduled_at) <= datetime(?) AND r.status = 'active' \
         AND r.pending_confirm = 0 \
         AND n.id = ( \
           SELECT id FROM reminder_notifications \
           WHERE reminder_id = n.reminder_id AND sent_at IS NULL AND datetime(scheduled_at) <= datetime(?) \
           ORDER BY datetime(scheduled_at) DESC LIMIT 1 \
         ) \
         ORDER BY n.scheduled_at ASC LIMIT 10"
    )
    .bind(&now)
    .bind(&now)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let count = rows.len();
    println!(
        "[ReminderScheduler] Query done, found {} matching notifications (now='{}')",
        count, now
    );

    for row in &rows {
        let lang = ReminderLanguage::from_code(&settings.general.language);
        let notif_id: String = row.get("id");
        let reminder_id: String = row.get("reminder_id");
        let intent: String = row.get("intent");
        let title: String = row.get("title");
        let event_type: String = row.get("event_type");
        let _date_status: String = row.get("date_status");
        let event_date: Option<String> = row.get("event_date");
        let event_time: Option<String> = row.get("event_time");
        let pending_confirm: i32 = row.get("pending_confirm");
        let scheduled_at: String = row.get("scheduled_at");

        if pending_confirm == 1 {
            continue;
        }

        let intent_display = reminder_intent_label(lang, &intent);

        let time_str = match (&event_date, &event_time) {
            (Some(d), Some(t)) => format!("{} {}", d, t),
            (Some(d), None) => d.clone(),
            _ => "".to_string(),
        };

        let full_msg = reminder_message(lang, &intent, &title, &time_str, &event_type);

        let _ = app.emit(
            "reminder-notification",
            serde_json::json!({
                "notificationId": notif_id,
                "reminderId": reminder_id,
                "intent": intent,
                "title": title,
                "eventType": event_type,
                "eventDate": event_date,
                "eventTime": event_time,
            }),
        );

        if settings.telegram.enabled && !settings.telegram.bot_token.is_empty() {
            let uids = &settings.telegram.allowed_user_ids;
            if uids.is_empty() {
                eprintln!(
                    "[ReminderScheduler] Telegram enabled but allowed user ID list is empty, skipping send."
                );
            } else {
                println!(
                    "[ReminderScheduler] Sending Telegram notifications to {} users: {:?}",
                    uids.len(),
                    uids
                );
                let bot_token = settings.telegram.bot_token.clone();
                for &user_id in uids {
                    match send_message(&bot_token, user_id, &full_msg).await {
                        Ok(_) => println!("[ReminderScheduler] Telegram sent to {}", user_id),
                        Err(e) => {
                            eprintln!(
                                "[ReminderScheduler] Telegram send failed ({}): {}",
                                user_id, e
                            )
                        }
                    }
                }
            }
        }

        let _ = app
            .notification()
            .builder()
            .title(intent_display)
            .body(&title)
            .show();

        sqlx::query(
            "UPDATE reminder_notifications SET sent_at = ? \
             WHERE reminder_id = ? AND datetime(scheduled_at) <= datetime(?) AND sent_at IS NULL",
        )
        .bind(&now)
        .bind(&reminder_id)
        .bind(&scheduled_at)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        println!(
            "[ReminderScheduler] Notification sent: {} (intent={}, reminder={})",
            title, intent, reminder_id
        );
    }

    let local_today = chrono::Local::now().format("%Y-%m-%d").to_string();
    sqlx::query(
        "UPDATE reminders SET status = 'expired', updated_at = ? \
         WHERE status = 'active' AND event_date IS NOT NULL AND event_date < ? \
         AND date_status IN ('confirmed', 'time_inferred')",
    )
    .bind(&now)
    .bind(&local_today)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(count)
}

fn is_quiet_hours(start: &str, end: &str, weekend_quiet: bool) -> bool {
    let now = chrono::Local::now();
    is_quiet_hours_internal(start, end, weekend_quiet, now)
}

pub fn is_quiet_hours_internal(
    start: &str,
    end: &str,
    weekend_quiet: bool,
    now: chrono::DateTime<chrono::Local>,
) -> bool {
    if weekend_quiet {
        let weekday = now.weekday();
        if matches!(weekday, Weekday::Sat | Weekday::Sun) {
            println!("[ReminderScheduler] In weekend quiet hours");
            return true;
        }
    }

    let current_time = now.time();
    let start_time = NaiveTime::parse_from_str(start, "%H:%M")
        .unwrap_or(NaiveTime::from_hms_opt(22, 0, 0).unwrap());
    let end_time = NaiveTime::parse_from_str(end, "%H:%M")
        .unwrap_or(NaiveTime::from_hms_opt(8, 0, 0).unwrap());

    let offset_secs = now.offset().local_minus_utc();
    println!(
        "[ReminderScheduler] Time diagnostics: Local={} (Offset={}h), UTC={}",
        now.format("%H:%M:%S"),
        offset_secs / 3600,
        chrono::Utc::now().format("%H:%M:%S")
    );

    let is_quiet = if start_time <= end_time {
        current_time >= start_time && current_time <= end_time
    } else {
        current_time >= start_time || current_time <= end_time
    };

    if is_quiet {
        println!(
            "[ReminderScheduler] Entered quiet hours ({}-{}, current {})",
            start,
            end,
            current_time.format("%H:%M")
        );
    }
    is_quiet
}

#[cfg(test)]
mod tests {
    use super::is_quiet_hours_internal;
    use chrono::{Local, TimeZone};

    #[test]
    fn test_quiet_hours_cross_midnight() {
        let now = Local
            .with_ymd_and_hms(2026, 4, 20, 23, 30, 0)
            .single()
            .unwrap();
        assert!(is_quiet_hours_internal("22:00", "08:00", false, now));
    }

    #[test]
    fn test_quiet_hours_non_quiet_window() {
        let now = Local
            .with_ymd_and_hms(2026, 4, 20, 14, 0, 0)
            .single()
            .unwrap();
        assert!(!is_quiet_hours_internal("22:00", "08:00", false, now));
    }

    #[test]
    fn test_weekend_quiet() {
        let now = Local
            .with_ymd_and_hms(2026, 4, 19, 11, 0, 0)
            .single()
            .unwrap();
        assert!(is_quiet_hours_internal("22:00", "08:00", true, now));
    }
}
