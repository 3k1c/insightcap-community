use std::time::Duration;

use chrono::{Datelike, NaiveTime, Utc, Weekday};
use sqlx::Row;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::sleep;

use tauri_plugin_notification::NotificationExt;

use crate::background::telegram_bot::send_message;
use crate::db::AppState;
use crate::settings::store::get_settings;

/// 輪詢間隔：60 秒
const POLL_INTERVAL_SECS: u64 = 60;

/// 啟動提醒排程 worker
/// - 每 60 秒檢查一次 reminder_notifications 表
/// - 發送至前端通訊站 (sonner toast)
/// - 發送 Telegram 訊息 (若啟用)
/// - 發送 OS 系統通知
/// - 清理過期提醒
pub fn start_reminder_scheduler(app: AppHandle) {
    let shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        println!("[ReminderScheduler] Worker 啟動");
        if let Err(e) = process_due_notifications(&app, false).await {
            eprintln!("[ReminderScheduler] 啟動時首次檢查失敗: {}", e);
        }
        let mut shutdown_rx = shutdown_rx;
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[ReminderScheduler] 收到停止訊號，退出中...");
                        break;
                    }
                }
                _ = sleep(Duration::from_secs(POLL_INTERVAL_SECS)) => {
                    // 更新死鎖監測計數器
                    app.state::<crate::db::AppState>().reminder_loop_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    if let Err(e) = process_due_notifications(&app, false).await {
                        eprintln!("[ReminderScheduler] 處理失敗: {}", e);
                    }
                }
            }
        }
    });
}

pub async fn process_due_notifications(app: &AppHandle, force: bool) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    // 1. 檢查是否啟用
    let settings = get_settings(pool).await.map_err(|e| e.to_string())?;
    println!(
        "[ReminderScheduler] 檢查待發通知... 提醒功能: {}, Telegram: {}, 強制模式: {}",
        settings.reminders.enabled, settings.telegram.enabled, force
    );
    if !settings.reminders.enabled && !force {
        println!("[ReminderScheduler] 提醒功能關閉且非強制模式，跳過處理");
        return Ok(0);
    }

    // 2. 檢查靜默時段（強制模式不限制）
    if !force
        && is_quiet_hours(
            &settings.reminders.quiet_hours_start,
            &settings.reminders.quiet_hours_end,
            settings.reminders.weekend_quiet,
        )
    {
        println!("[ReminderScheduler] 目前處於靜默時段，暫緩通知");
        return Ok(0);
    }

    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    // 3. 獲取當前應發送的通知（按 reminder_id 分組，每組只取最新的一筆過期通知）
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
        "[ReminderScheduler] 查詢完成，找到 {} 筆符合條件的通知 (參數 now='{}')",
        count, now
    );

    for row in &rows {
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

        // 跳過 pending_confirm 的提醒（尚在預測中，不發布）
        if pending_confirm == 1 {
            continue;
        }

        // 轉換為人性化顯示標籤
        let intent_display = match intent.as_str() {
            "start" => "提醒：準備開始",
            "midcheck" => "提醒：進度中檢",
            "urgent" => "緊急提醒！",
            "final" => "最後提醒！",
            "prepare" => "預先提醒",
            "imminent" => "即將揭幕",
            "now" => "就是現在！",
            "confirm_date" => "確認日期提醒",
            _ => "提醒",
        };

        let time_str = match (&event_date, &event_time) {
            (Some(d), Some(t)) => format!("{} {}", d, t),
            (Some(d), None) => d.clone(),
            _ => "".to_string(),
        };

        let full_msg = format!(
            "【{}】\n標題：{}\n時間：{}\n類型：{}",
            intent_display, title, time_str, event_type
        );

        // a. 發送 Tauri event 給前端 (In-app Toast)
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

        // b. 發送 Telegram 訊息
        if settings.telegram.enabled && !settings.telegram.bot_token.is_empty() {
            let uids = &settings.telegram.allowed_user_ids;
            if uids.is_empty() {
                eprintln!(
                    "[ReminderScheduler] Telegram 已啟用但「授權 User ID」清單為空，跳過發送。"
                );
            } else {
                println!(
                    "[ReminderScheduler] 嘗試發送 Telegram 通知至 {} 位用戶: {:?}",
                    uids.len(),
                    uids
                );
                let bot_token = settings.telegram.bot_token.clone();
                for &user_id in uids {
                    match send_message(&bot_token, user_id, &full_msg).await {
                        Ok(_) => println!("[ReminderScheduler] Telegram 成功發送至 {}", user_id),
                        Err(e) => {
                            eprintln!("[ReminderScheduler] Telegram 發送失敗 ({}): {}", user_id, e)
                        }
                    }
                }
            }
        }

        // c. 發送 OS 系統通知
        let _ = app
            .notification()
            .builder()
            .title(intent_display)
            .body(&title)
            .show();

        // 4. 標記處理完成
        // 特別注意：我們會將該提醒項「所有已過期且未發送」的通知全部標記為已發送，
        // 避免在下次輪詢時又觸發較舊的通知點。
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
            "[ReminderScheduler] 已發送通知: {} (intent={}, reminder={})",
            title, intent, reminder_id
        );
    }

    // 5. 清理過期提醒（使用本地日期；只要早於今天就標記為 expired）
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

/// 判斷當前是否處於靜默時段
fn is_quiet_hours(start: &str, end: &str, weekend_quiet: bool) -> bool {
    let now = chrono::Local::now();
    is_quiet_hours_internal(start, end, weekend_quiet, now)
}

/// 內部測試用：判斷特定時間是否處於靜默時段
pub fn is_quiet_hours_internal(
    start: &str,
    end: &str,
    weekend_quiet: bool,
    now: chrono::DateTime<chrono::Local>,
) -> bool {
    // 週末靜默檢查
    if weekend_quiet {
        let weekday = now.weekday();
        if matches!(weekday, Weekday::Sat | Weekday::Sun) {
            println!("[ReminderScheduler] 週末靜默中");
            return true;
        }
    }

    let current_time = now.time();
    let start_time = NaiveTime::parse_from_str(start, "%H:%M")
        .unwrap_or(NaiveTime::from_hms_opt(22, 0, 0).unwrap());
    let end_time = NaiveTime::parse_from_str(end, "%H:%M")
        .unwrap_or(NaiveTime::from_hms_opt(8, 0, 0).unwrap());

    // 詳細時區診斷記錄
    let offset_secs = now.offset().local_minus_utc();
    println!(
        "[ReminderScheduler] 時間診斷：Local={} (Offset={}h), UTC={}",
        now.format("%H:%M:%S"),
        offset_secs / 3600,
        chrono::Utc::now().format("%H:%M:%S")
    );

    let is_quiet = if start_time <= end_time {
        current_time >= start_time && current_time <= end_time
    } else {
        // 跨夜 (如 22:00 - 08:00)
        current_time >= start_time || current_time <= end_time
    };

    if is_quiet {
        println!(
            "[ReminderScheduler] 進入靜默時段 ({}-{}，當前 {})",
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
        // 2026-04-19 為週日
        let now = Local
            .with_ymd_and_hms(2026, 4, 19, 11, 0, 0)
            .single()
            .unwrap();
        assert!(is_quiet_hours_internal("22:00", "08:00", true, now));
    }
}
