use insightcap_lib::db::connection::init_db;
use insightcap_lib::db::AppState;
use insightcap_lib::providers::embedding::fastembed::FastEmbedder;
use insightcap_lib::providers::embedding::Embedder;
use insightcap_lib::services::space_engine::SpaceEngine;
use insightcap_lib::vector_store::local::VectorStore;
use sqlx::Row;
use std::path::PathBuf;
use std::sync::Arc;

async fn print_spaces(pool: &sqlx::SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let rows = sqlx::query("SELECT name, chunk_count FROM spaces WHERE is_archived = 0")
        .fetch_all(pool)
        .await?;
    println!("  [STATUS] Spaces ({}):", rows.len());
    if rows.is_empty() {
        println!("    (No active spaces)");
    }
    for r in rows {
        println!(
            "    - {}: {} items",
            r.get::<String, _>("name"),
            r.get::<i64, _>("chunk_count")
        );
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(">>> Space Evolution & Reclustering Test <<<");

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
    let embedder = Arc::new(FastEmbedder::new("MultilingualE5Small")?);
    let vector_store = VectorStore::load_or_create(&kb_path, embedder.dimension())?;
    let (shutdown_tx, _) = tokio::sync::watch::channel(false);
    let state = AppState::new(
        pool.clone(),
        kb_path,
        vector_store,
        embedder,
        Arc::new(shutdown_tx),
    );

    let engine = SpaceEngine::new(
        state.db.clone(),
        state.embedder.clone(),
        state.vector_store.clone(),
    );

    println!("\n--- Step CLEANUP: Removing old test data ---");
    sqlx::query("DELETE FROM captures WHERE id LIKE 'space-test-%'")
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM sources WHERE id = 'space-test'")
        .execute(&pool)
        .await?;

    println!("\n--- Step 0: Initial State ---");
    print_spaces(&pool).await?;

    // Create dummy source
    let now_iso = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT OR IGNORE INTO sources (id, type, title, clean_content, captured_at, updated_at) VALUES ('space-test', 'text', 'Space Evolution Test Stub', '', ?, ?)")
        .bind(&now_iso).bind(&now_iso).execute(&pool).await?;

    println!("\n--- Step 1: Injecting Rust technical docs (Wave 1) ---");
    let docs = vec![
        "Rust async-std vs tokio runtime benchmarks and performance comparison.",
        "How to use async/await in Rust with the latest edition features.",
        "Exploring memory safety in Rust: Ownership and Borrowing rules explained.",
        "Zero-cost abstractions in Rust: How they work under the hood.",
        "Efficient CLI development in Rust using the clap crate.",
    ];

    for (i, content) in docs.iter().enumerate() {
        let id = format!("space-test-w1-{}", i);
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT OR REPLACE INTO captures (id, source_id, type, raw_content, clean_content, capture_method, status, created_at, updated_at) VALUES (?, 'space-test', 'text', ?, ?, 'import', 'processed', ?, ?)")
            .bind(&id).bind(*content).bind(*content).bind(&now).bind(&now).execute(&pool).await?;

        let res = engine.assign_to_space(&id, content).await?;
        println!("  Assigned '{}...' to: {:?}", &content[..20], res);
    }
    print_spaces(&pool).await?;

    println!("\n--- Step 2: Injecting unrelated Cooking content (Wave 2) ---");
    let cooking = vec![
        "Best Italian pasta carbonara recipe using fresh eggs and pecorino cheese.",
        "How to bake sourdough bread at home: A step-by-step fermentation guide.",
        "The history of pizza in Naples and the traditional margherita style.",
    ];
    for (i, content) in cooking.iter().enumerate() {
        let id = format!("space-test-w2-{}", i);
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("INSERT OR REPLACE INTO captures (id, source_id, type, raw_content, clean_content, capture_method, status, created_at, updated_at) VALUES (?, 'space-test', 'text', ?, ?, 'import', 'processed', ?, ?)")
            .bind(&id).bind(*content).bind(*content).bind(&now).bind(&now).execute(&pool).await?;

        let res = engine.assign_to_space(&id, content).await?;
        println!("  Assigned Cooking content to: {:?}", res);
    }
    print_spaces(&pool).await?;

    println!("\n--- Step 3: Triggering Recluster (Testing Merge/Dynamic Combo) ---");
    // Inject something very similar to Wave 1 but broader
    let broad_rust =
        "A comprehensive guide to Rust programming language, focusing on system safety and speed.";
    let id_b = "space-test-w3-broad";
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT OR REPLACE INTO captures (id, source_id, type, raw_content, clean_content, capture_method, status, created_at, updated_at) VALUES (?, 'space-test', 'text', ?, ?, 'import', 'processed', ?, ?)")
        .bind(id_b).bind(broad_rust).bind(broad_rust).bind(&now).bind(&now).execute(&pool).await?;
    engine.assign_to_space(id_b, broad_rust).await?;

    println!("  Running global recluster...");
    let updated = engine.recluster_all().await?;
    println!("  Recluster done. Updated {} items.", updated);
    print_spaces(&pool).await?;

    println!(
        "\n--- Step 4: Deleting Cooking content and Reclustering (Observe Space decrease) ---"
    );
    sqlx::query("DELETE FROM captures WHERE id LIKE 'space-test-w2-%'")
        .execute(&pool)
        .await?;
    println!("  Running cleanup recluster...");
    engine.recluster_all().await?;
    print_spaces(&pool).await?;

    println!("\n>>> TEST COMPLETE. Cleaning up test samples...");
    sqlx::query("DELETE FROM sources WHERE id = 'space-test'")
        .execute(&pool)
        .await?;
    engine.recluster_all().await?;
    print_spaces(&pool).await?;

    Ok(())
}
