use insightcap_lib::db::connection::init_db;
use insightcap_lib::db::AppState;
use insightcap_lib::providers::embedding::fastembed::FastEmbedder;
use insightcap_lib::providers::embedding::Embedder;
use insightcap_lib::services::memory_engine::MemoryEngine;
use insightcap_lib::vector_store::local::VectorStore;
use sqlx::Row;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(">>> Memory System (Data/Pattern/Log) Stress Test <<<");

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

    let engine = MemoryEngine::new(pool.clone(), vector_store, embedder);

    let test_cases = vec![
        (
            "conv-data-001",
            "This conversation discussed the differences between SQL and NoSQL databases. We explored ACID properties vs BASE properties, and scenarios for using each type in production environments.",
            "data"
        ),
        (
            "conv-pattern-001",
            "We established a new development pattern: any performance-critical Rust logic should be moved to the 'src/bin' directory as a standalone binary first for profiling and stress testing before integration.",
            "pattern"
        ),
        (
            "conv-log-001",
            "CRITICAL FIX: Resolved a crash on Windows where pdfium.dll was missing. Learned that manual library placement is unreliable. The permanent fix is to modify the build script to automate DLL copying. Avoid manual DLL copying in target/debug.",
            "log"
        )
    ];

    println!("\n--- Step 0: Preparing Dummy Conversations ---");
    let now = chrono::Utc::now().to_rfc3339();
    for (id, _, _) in &test_cases {
        sqlx::query("INSERT OR IGNORE INTO conversations (id, title, created_at, updated_at) VALUES (?, 'Test Conversation', ?, ?)")
            .bind(id).bind(&now).bind(&now).execute(&pool).await?;
    }

    println!("\n--- Step 1: Processing Conversation Summaries ---");
    for (id, summary, expected) in &test_cases {
        println!("  Extracing from: {} (Expected: {})", id, expected);
        let chunk_id = engine
            .process_conversation_summary(id, summary, None)
            .await?;
        println!("  Created memory_chunk: {}", chunk_id);
    }

    println!("\n--- Step 2: Verifying Results in Database ---");
    let rows = sqlx::query("SELECT id, knowledge_type, content, tags, confidence, pending_confirm FROM memory_chunks WHERE conversation_id LIKE 'conv-%'")
        .fetch_all(&pool).await?;

    println!("  Memory Fragments Found ({}):", rows.len());
    for r in rows {
        let k_name: String = r.get("id");
        let k_type: String = r.get("knowledge_type");
        let content: String = r.get("content");
        let confidence: f32 = r.get("confidence");
        let pending: i32 = r.get("pending_confirm");
        let tags: String = r.get("tags");

        println!(
            "  - ID: {} [{}] (Conf: {:.2}, Pending: {}) Content: {}...",
            k_name,
            k_type.to_uppercase(),
            confidence,
            pending,
            &content.chars().take(40).collect::<String>()
        );
        println!("    Tags: {}", tags);
    }

    println!("\n>>> TEST COMPLETE. Cleaning up test data...");
    sqlx::query("DELETE FROM memory_chunks WHERE conversation_id LIKE 'conv-%'")
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM conversations WHERE id LIKE 'conv-%'")
        .execute(&pool)
        .await?;
    println!("  Cleanup done.");

    Ok(())
}
