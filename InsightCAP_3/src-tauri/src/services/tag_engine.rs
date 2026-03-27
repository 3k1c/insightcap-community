use sqlx::SqlitePool;

pub struct TagEngine {
    pool: SqlitePool,
}

impl TagEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn process_new_capture(&self, capture_id: &str, content: &str) -> Result<Vec<String>, String> {
        // 1. 本地規則或簡單關鍵字擷取 (Phase 2 後續接入 LLM)
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

        use crate::providers::llm::LLMProvider;
        let generated_tags = if let Some(llm) = opt_provider {
            let prompt = format!("Extract 3-5 tags from the following text to represent its core concepts. Return ONLY a comma-separated list of short tags in Traditional Chinese. NO other text.\n\nText:\n{}", content);
            match llm.complete(&prompt, crate::providers::llm::LLMOptions::default()).await {
                Ok(response) => {
                    response.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).take(5).collect()
                },
                Err(_) => vec!["自動標記".to_string(), "待分類".to_string()]
            }
        } else {
            vec!["自動標記".to_string(), "待分類".to_string()]
        };

        // 2. 更新 captures tags (JSON)
        let tags_json = serde_json::to_string(&generated_tags).unwrap();
        sqlx::query(
            "UPDATE captures SET tags = ? WHERE id = ?"
        )
        .bind(&tags_json)
        .bind(capture_id)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        // 3. Upsert 到 tags 表
        for tag in &generated_tags {
            let now = chrono::Utc::now().to_rfc3339();
            let id = uuid::Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO tags (id, name, source, use_count, recent_count, created_at, updated_at) 
                 VALUES (?, ?, 'ai', 1, 1, ?, ?)
                 ON CONFLICT(name) DO UPDATE SET 
                    use_count = use_count + 1,
                    recent_count = recent_count + 1,
                    updated_at = excluded.updated_at"
            )
            .bind(&id)
            .bind(tag)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        }

        Ok(generated_tags)
    }
}
