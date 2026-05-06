use insightcap_lib::db::connection::init_db;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    println!("Attempting to manually add is_user_managed to spaces...");
    let _ = sqlx::query("ALTER TABLE spaces ADD COLUMN is_user_managed INTEGER DEFAULT 0")
        .execute(&pool)
        .await;

    println!("Refetching schema...");
    let row: (String,) =
        sqlx::query_as("SELECT sql FROM sqlite_master WHERE type='table' AND name='spaces'")
            .fetch_one(&pool)
            .await?;
    println!("Updated Spaces Schema: {}", row.0);

    Ok(())
}
