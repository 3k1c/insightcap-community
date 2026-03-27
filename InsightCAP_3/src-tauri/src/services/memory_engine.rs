use sqlx::SqlitePool;

pub struct MemoryEngine {
    pool: SqlitePool,
}

impl MemoryEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 從對話總結生成 MemoryChunks
    pub async fn process_conversation_summary(&self, conversation_id: &str, summary_text: &str) -> Result<(), String> {
        let chunk_id = uuid::Uuid::now_v7().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        
        sqlx::query(
            "INSERT INTO memory_chunks (id, source_id, knowledge_type, content, extracted_from_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&chunk_id)
        .bind(conversation_id) // 從對話來的 source 標識
        .bind("pattern") // 對話總結目前當作 pattern 碎片
        .bind(summary_text)
        .bind(conversation_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        println!("[MemoryEngine] Saved conversation summary as pattern chunk: {}", chunk_id);
        Ok(())
    }
}
