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
        let settings_json: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'models'")
            .fetch_optional(&self.pool).await.map_err(|e| e.to_string())?;

        let mut opt_provider: Option<crate::providers::llm::openai::OpenAiProvider> = None;
        if let Some(json_str) = settings_json {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(active) = val["activeProvider"].as_str() {
                    if active == "openai" {
                         if let Some(api_key) = val["openai"]["apiKey"].as_str() {
                             let base_url = val["openai"]["baseUrl"].as_str().map(|s| s.to_string());
                             let model = val["openai"]["model"].as_str().unwrap_or("gpt-4o-mini").to_string();
                             opt_provider = Some(crate::providers::llm::openai::OpenAiProvider::new(api_key.to_string(), base_url, model));
                         }
                    }
                }
            }
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
