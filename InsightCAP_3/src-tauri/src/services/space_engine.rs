use sqlx::SqlitePool;

pub struct SpaceEngine {
    pool: SqlitePool,
}

impl SpaceEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 將給定的 Capture 分配到現有的 Space，或者建立新的 Space
    pub async fn assign_to_space(&self, capture_id: &str, content: &str) -> Result<Option<String>, String> {
        let settings = crate::settings::store::get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let cfg = settings.ai_models.content_processor_llm;
        
        let mut opt_provider: Option<crate::providers::llm::openai::OpenAiProvider> = None;
        let is_ollama = cfg.provider == "ollama";
        let api_key = cfg.api_key.unwrap_or_default();

        if !api_key.is_empty() || is_ollama {
            opt_provider = Some(crate::providers::llm::openai::OpenAiProvider::new(api_key, cfg.base_url, cfg.model));
        }

        if let Some(llm) = opt_provider {
            use crate::providers::llm::LLMProvider;
            let prompt = format!("Categorize the following text into one short category/space name (e.g. 程式開發, 數位行銷, 學習筆記). Return ONLY the category name in Traditional Chinese. NO punctuation.\n\nText:\n{}", content);
            if let Ok(category) = llm.complete(&prompt, crate::providers::llm::LLMOptions::default()).await {
                let clean_category = category.trim().to_string();
                if !clean_category.is_empty() {
                    // Find or create space
                    let space_id: Option<String> = sqlx::query_scalar("SELECT id FROM spaces WHERE name = ?")
                        .bind(&clean_category)
                        .fetch_optional(&self.pool)
                        .await
                        .unwrap_or(None);

                    let active_space_id = if let Some(id) = space_id {
                        id
                    } else {
                        // Create Space
                        let new_id = uuid::Uuid::now_v7().to_string();
                        let now = chrono::Utc::now().to_rfc3339();
                        sqlx::query("INSERT INTO spaces (id, name, chunk_count, created_at, updated_at) VALUES (?, ?, 0, ?, ?)")
                            .bind(&new_id)
                            .bind(&clean_category)
                            .bind(&now)
                            .bind(&now)
                            .execute(&self.pool)
                            .await
                            .map_err(|e| e.to_string())?;
                        new_id
                    };

                    // Link to capture
                    sqlx::query("UPDATE captures SET space_id = ? WHERE id = ?")
                        .bind(&active_space_id)
                        .bind(capture_id)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| e.to_string())?;

                    sqlx::query("UPDATE spaces SET chunk_count = chunk_count + 1 WHERE id = ?")
                        .bind(&active_space_id)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| e.to_string())?;

                    return Ok(Some(active_space_id));
                }
            }
        }

        println!("[SpaceEngine] 無法為 capture {} 分配 Space，保留預設 inbox 狀態", capture_id);
        Ok(None)
    }
}
