use insightcap_lib::db::connection::init_db;
use insightcap_lib::services::reminder_engine::ReminderEngine;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(">>> Reminder System SCALE Stress Test <<<");

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

    println!("\n--- Step 1: Manual Batch Creation (Testing DB and Scheduler) ---");
    let conv_id = "scale-test-001";
    sqlx::query(
        "DELETE FROM reminder_notifications WHERE reminder_id IN (
            SELECT id FROM reminders
            WHERE title LIKE 'Daily Review Task%' OR title LIKE 'Batch Reminder #%' OR conversation_id = ?
        )",
    )
    .bind(conv_id)
    .execute(&pool)
    .await?;
    sqlx::query(
        "DELETE FROM reminders
         WHERE title LIKE 'Daily Review Task%' OR title LIKE 'Batch Reminder #%' OR conversation_id = ?",
    )
    .bind(conv_id)
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM conversations WHERE id = ?")
        .bind(conv_id)
        .execute(&pool)
        .await?;
    sqlx::query("INSERT OR IGNORE INTO conversations (id, title, created_at, updated_at) VALUES (?, 'Load Test', '2026-04-27T10:00:00', '2026-04-27T10:00:00')")
        .bind(conv_id)
        .execute(&pool)
        .await?;

    println!("  Creating 30 reminders manually...");
    for i in 1..=30 {
        let date = format!("2027-01-{:02}", i);
        let title = format!("Daily Review Task #{}", i);
        engine
            .create_reminder(
                &title,
                Some("Load test task"),
                "event",
                &date,
                Some("09:00"),
            )
            .await?;
    }

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM reminders WHERE title LIKE 'Daily Review Task%'")
            .fetch_one(&pool)
            .await?;
    let notif_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reminder_notifications WHERE reminder_id IN (SELECT id FROM reminders WHERE title LIKE 'Daily Review Task%')",
    )
    .fetch_one(&pool)
    .await?;
    println!(
        "  Created {} reminders and {} scheduled notifications.",
        count, notif_count
    );

    println!("\n--- Step 2: Extraction Test (20 items) ---");
    let mut dialogue = String::from("User: 請幫我建立以下提醒：\n");
    let mut expected_titles = Vec::new();
    for i in 1..=20 {
        let title = format!("Batch Reminder #{}", i);
        expected_titles.push(title.clone());
        dialogue.push_str(&format!("{}. 2027-10-{:02} 09:00 {}\n", i, i, title));
    }

    let timestamp = "Context: Current date is 2026-04-27.";
    let ids = engine
        .extract_reminders(
            conv_id,
            "Large reminder batch extraction",
            &dialogue,
            timestamp,
            None,
        )
        .await?;
    println!("  Extracted {} reminders from batch dialogue.", ids.len());

    let extracted_titles: Vec<String> = sqlx::query_scalar(
        "SELECT title FROM reminders WHERE conversation_id = ? AND title LIKE 'Batch Reminder #%' ORDER BY title ASC",
    )
    .bind(conv_id)
    .fetch_all(&pool)
    .await?;
    let missing: Vec<String> = expected_titles
        .into_iter()
        .filter(|title| !extracted_titles.contains(title))
        .collect();
    println!("  Stored titles: {}", extracted_titles.len());
    println!("  Missing titles: {:?}", missing);

    println!("\n--- Step 3: Global Snooze Test ---");
    if let Some(first_id) = ids.get(0) {
        println!("  Snoozing first extracted reminder...");
        engine.snooze_reminder(first_id, 60).await?;
        println!("  Snooze OK.");
    }

    println!("\n>>> SCALE TEST COMPLETE. Cleaning up...");
    sqlx::query(
        "DELETE FROM reminders WHERE title LIKE 'Daily Review Task%' OR conversation_id = ?",
    )
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
