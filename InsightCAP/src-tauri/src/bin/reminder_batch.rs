use insightcap_lib::db::connection::init_db;
use insightcap_lib::services::reminder_engine::ReminderEngine;
use std::path::PathBuf;
use sqlx::Row;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(">>> Reminder System SCALE Stress Test <<<");

    let user_profile = std::env::var("USERPROFILE").unwrap();
    let app_data_dir = PathBuf::from(user_profile).join("AppData").join("Roaming").join("com.insightcap.app");
    let bootstrap_path = app_data_dir.join("bootstrap.json");
    let kb_path_str = if bootstrap_path.exists() {
        let content = std::fs::read_to_string(&bootstrap_path)?;
        let json: serde_json::Value = serde_json::from_str(&content)?;
        json["kb_path"].as_str().unwrap().to_string()
    } else {
        app_data_dir.join("insightcap_v2").to_string_lossy().to_string()
    };
    let kb_path = PathBuf::from(&kb_path_str);
    let db_key_hex = keyring::Entry::new("insightcap", "auto_login_key").ok().and_then(|e| e.get_password().ok());
    let pool = init_db(&kb_path, db_key_hex.as_deref()).await?;
    let engine = ReminderEngine::new(pool.clone());

    println!("\n--- Step 1: Manual Batch Creation (Testing DB and Scheduler) ---");
    let conv_id = "scale-test-001";
    sqlx::query("INSERT OR IGNORE INTO conversations (id, title, created_at, updated_at) VALUES (?, 'Load Test', '2026-04-27T10:00:00', '2026-04-27T10:00:00')")
        .bind(conv_id).execute(&pool).await?;

    // Create 30 reminders manually to test DB performance
    println!("  Creating 30 reminders manually...");
    for i in 1..=30 {
        let date = format!("2027-01-{:02}", i);
        let title = format!("Daily Review Task #{}", i);
        engine.create_reminder(&title, Some("Load test task"), "event", &date, Some("09:00")).await?;
    }
    
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reminders WHERE title LIKE 'Daily Review Task%'").fetch_one(&pool).await?;
    let notif_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM reminder_notifications WHERE reminder_id IN (SELECT id FROM reminders WHERE title LIKE 'Daily Review Task%')").fetch_one(&pool).await?;
    println!("  Created {} reminders and {} scheduled notifications.", count, notif_count);

    println!("\n--- Step 2: Extraction Test (Minimal 3 items) ---");
    let dialogue = r#"
        User: 請幫我加三個提醒：
        1. 2027-10-01 下午兩點見客戶；
        2. 2027-10-05 提交週報；
        3. 2027-10-10 朋友生日。
    "#;
    let timestamp = "Context: Current date is 2026-04-27.";
    let ids = engine.extract_reminders(conv_id, "Simple list", dialogue, timestamp, None).await?;
    println!("  Extracted {} reminders from simple dialogue.", ids.len());

    println!("\n--- Step 3: Global Snooze Test ---");
    if let Some(first_id) = ids.get(0) {
        println!("  Snoozing first extracted reminder...");
        engine.snooze_reminder(first_id, 60).await?;
        println!("  Snooze OK.");
    }

    println!("\n>>> SCALE TEST COMPLETE. Cleaning up...");
    sqlx::query("DELETE FROM reminders WHERE title LIKE 'Daily Review Task%' OR conversation_id = ?").bind(conv_id).execute(&pool).await?;
    sqlx::query("DELETE FROM conversations WHERE id = ?").bind(conv_id).execute(&pool).await?;
    println!("  Cleanup done.");

    Ok(())
}
