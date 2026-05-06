use insightcap_lib::db::connection::init_db;
use insightcap_lib::services::reminder_engine::ReminderEngine;
use sqlx::Row;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(">>> Reminder System Lifecycle Stress Test <<<");

    let user_profile = std::env::var("USERPROFILE").unwrap();
    let app_data_dir = PathBuf::from(user_profile)
        .join("AppData")
        .join("Roaming")
        .join("com.insightcap.app");

    let bootstrap_path = app_data_dir.join("bootstrap.json");
    let kb_path_str = if bootstrap_path.exists() {
        let content = std::fs::read_to_string(&bootstrap_path)?;
        let json: serde_json::Value = serde_json::from_str(&content)?;
        json["kb_path"].as_str().unwrap().to_string()
    } else {
        app_data_dir
            .join("insightcap_v2")
            .to_string_lossy()
            .to_string()
    };
    let kb_path = PathBuf::from(&kb_path_str);
    let db_key_hex = keyring::Entry::new("insightcap", "auto_login_key")
        .ok()
        .and_then(|e| e.get_password().ok());
    let pool = init_db(&kb_path, db_key_hex.as_deref()).await?;

    let engine = ReminderEngine::new(pool.clone());

    println!("\n--- Step 1: Reminder Detection & Extraction ---");
    let conv_id = "test-reminder-conv-001";
    let summary = "Discussed UI implementation and meeting schedule.";
    // Use a date in the future for testing
    let dialogue = "User: 我要在 2026-12-01 下午 2 點跟 UI 團隊開會探討設計稿。\nAssistant: 好的，我會為您記下這個提醒。";
    let timestamp = "Context: Today is 2026-04-27 Monday.";

    // Ensure conversation stub exists
    sqlx::query("INSERT OR IGNORE INTO conversations (id, title, created_at, updated_at) VALUES (?, 'Reminder Test', '2026-04-27T10:00:00', '2026-04-27T10:00:00')")
        .bind(conv_id).execute(&pool).await?;

    println!("  Extracting reminders from dialogue...");
    let ids = engine
        .extract_reminders(conv_id, summary, dialogue, timestamp, None)
        .await?;

    if ids.is_empty() {
        println!("  WARNING: No reminders extracted. LLM might have missed it.");
    } else {
        println!("  Extracted IDs: {:?}", ids);
        for id in &ids {
            let r = sqlx::query("SELECT title, event_date, event_time, status, pending_confirm FROM reminders WHERE id = ?")
                .bind(id).fetch_one(&pool).await?;
            println!(
                "  - [{}] Title: {}, Date: {:?}, Time: {:?}, Status: {}, Pending: {}",
                id,
                r.get::<String, _>("title"),
                r.get::<Option<String>, _>("event_date"),
                r.get::<Option<String>, _>("event_time"),
                r.get::<String, _>("status"),
                r.get::<i32, _>("pending_confirm")
            );
        }
    }

    if let Some(reminder_id) = ids.get(0).cloned() {
        println!("\n--- Step 2: Reminder Confirmation ---");
        engine.confirm_reminder(&reminder_id, true).await?;
        let pending: i32 = sqlx::query_scalar("SELECT pending_confirm FROM reminders WHERE id = ?")
            .bind(&reminder_id)
            .fetch_one(&pool)
            .await?;
        println!("  Reminder confirmed. Pending status: {}", pending);

        println!("\n--- Step 3: Reminder Snooze (Delay) ---");
        println!("  Snoozing for 30 minutes...");
        engine.snooze_reminder(&reminder_id, 30).await?;
        let next_notif: String = sqlx::query_scalar("SELECT scheduled_at FROM reminder_notifications WHERE reminder_id = ? AND sent_at IS NULL ORDER BY scheduled_at ASC LIMIT 1")
            .bind(&reminder_id).fetch_one(&pool).await?;
        println!("  Next notification scheduled at: {}", next_notif);

        println!("\n--- Step 4: Status Update (Manual Override) ---");
        println!("  Setting status to 'completed'...");
        engine
            .update_reminder_status(&reminder_id, "completed")
            .await?;
        let status: String = sqlx::query_scalar("SELECT status FROM reminders WHERE id = ?")
            .bind(&reminder_id)
            .fetch_one(&pool)
            .await?;
        println!("  Final Status: {}", status);
    }

    println!("\n--- Step 5: Handling Surprise Event (Update via Extraction) ---");
    // Simulate updating the same event due to new info in dialogue
    let dialogue_v2 =
        "User: 會議改期了，改到 2026-12-05 上午 10 點。\nAssistant: 沒問題，我來更新。";
    println!("  Extracting update from dialogue v2...");
    let updated_ids = engine
        .extract_reminders(
            conv_id,
            "Update meeting time.",
            dialogue_v2,
            timestamp,
            None,
        )
        .await?;
    println!("  Updated/New IDs: {:?}", updated_ids);
    for id in &updated_ids {
        let r = sqlx::query("SELECT title, event_date, event_time FROM reminders WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await?;
        println!(
            "  - [{}] Title: {}, Date: {:?}, Time: {:?}",
            id,
            r.get::<String, _>("title"),
            r.get::<Option<String>, _>("event_date"),
            r.get::<Option<String>, _>("event_time")
        );
    }

    println!("\n>>> TEST COMPLETE. Cleaning up test samples...");
    sqlx::query("DELETE FROM reminders WHERE conversation_id = ?")
        .bind(conv_id)
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM conversations WHERE id = ?")
        .bind(conv_id)
        .execute(&pool)
        .await?;
    println!("  Cleanup done.");

    Ok(())
}
