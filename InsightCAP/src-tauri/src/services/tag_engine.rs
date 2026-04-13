use sqlx::SqlitePool;

pub struct TagEngine {
    pool: SqlitePool,
}

impl TagEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn process_new_capture(&self, capture_id: &str, content: &str) -> Result<Vec<String>, String> {
        let settings = crate::settings::store::get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let cfg = settings.ai_models.content_processor_llm;
        
        let mut opt_provider: Option<crate::providers::llm::openai::OpenAiProvider> = None;
        let is_ollama = cfg.provider == "ollama";
        let api_key = cfg.api_key.unwrap_or_default();

        if !api_key.is_empty() || is_ollama {
            opt_provider = Some(crate::providers::llm::openai::OpenAiProvider::new(api_key, cfg.base_url, cfg.model, cfg.provider.clone()));
        }

        use crate::providers::llm::LLMProvider;
        let generated_tags = if let Some(llm) = opt_provider {
            let prompt = format!("Extract 3-5 tags from the following text to represent its core concepts. Return ONLY a comma-separated list of short tags in Traditional Chinese. NO other text.\n\nText:\n{}", &content[..content.len().min(2000)]);
            match llm.complete(&prompt, crate::providers::llm::LLMOptions::default()).await {
                Ok(response) => {
                    let tags: Vec<String> = response.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s.len() < 30).take(5).collect();
                    if tags.is_empty() { extract_keyword_tags(content) } else { tags }
                },
                Err(e) => {
                    eprintln!("[TagEngine] LLM failed, falling back to keyword extraction: {}", e);
                    extract_keyword_tags(content)
                }
            }
        } else {
            extract_keyword_tags(content)
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

    /// 以 Source 為單位生成標籤（輸入整份文件內容，標籤存入 sources.tags）
    pub async fn process_source(&self, source_id: &str, full_content: &str) -> Result<Vec<String>, String> {
        let settings = crate::settings::store::get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let cfg = settings.ai_models.content_processor_llm;

        let mut opt_provider: Option<crate::providers::llm::openai::OpenAiProvider> = None;
        let is_ollama = cfg.provider == "ollama";
        let api_key = cfg.api_key.unwrap_or_default();

        if !api_key.is_empty() || is_ollama {
            opt_provider = Some(crate::providers::llm::openai::OpenAiProvider::new(api_key, cfg.base_url, cfg.model, cfg.provider.clone()));
        }

        use crate::providers::llm::LLMProvider;
        // 取前 3000 bytes，但確保切在 char boundary
        let byte_limit = full_content.len().min(3000);
        let safe_limit = full_content.floor_char_boundary(byte_limit);
        let sample = &full_content[..safe_limit];
        let generated_tags = if let Some(llm) = opt_provider {
            let prompt = format!(
                "請分析以下文件，提取 3-5 個能代表整份文件核心主題的標籤。\
                 要求：標籤必須反映文件的主要知識領域，而非單一句子的細節。\
                 只回傳逗號分隔的繁體中文標籤，不含其他文字。\n\n文件內容：\n{}",
                sample
            );
            match llm.complete(&prompt, crate::providers::llm::LLMOptions::default()).await {
                Ok(response) => {
                    let tags: Vec<String> = response
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s.len() < 30)
                        .take(5)
                        .collect();
                    if tags.is_empty() { extract_keyword_tags(full_content) } else { tags }
                }
                Err(e) => {
                    eprintln!("[TagEngine] Source LLM failed, fallback: {}", e);
                    extract_keyword_tags(full_content)
                }
            }
        } else {
            extract_keyword_tags(full_content)
        };

        // 寫入 sources.tags
        let tags_json = serde_json::to_string(&generated_tags).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE sources SET tags = ?, updated_at = ? WHERE id = ?")
            .bind(&tags_json)
            .bind(&now)
            .bind(source_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        // 同步 upsert 到全域 tags 表
        self.upsert_tags(&generated_tags).await?;

        Ok(generated_tags)
    }

    /// 將已產生的 tags 寫入 tags 表（upsert + 更新頻率計數）
    pub async fn upsert_tags(&self, tags: &[String]) -> Result<(), String> {
        for tag in tags {
            if tag == "untagged" { continue; }
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
        Ok(())
    }
}

/// LLM 不可用時的本地關鍵字提取 fallback
fn extract_keyword_tags(content: &str) -> Vec<String> {
    use std::collections::HashMap;

    // 停用詞（中英文常見虛詞）
    const STOP_WORDS: &[&str] = &[
        "的", "了", "在", "是", "我", "有", "和", "就", "不", "人", "都", "一", "一個",
        "上", "也", "很", "到", "說", "要", "去", "你", "會", "著", "沒有", "看", "好",
        "自己", "這", "他", "她", "它", "們", "那", "被", "從", "把", "讓", "用", "對",
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "shall", "should",
        "may", "might", "must", "can", "could", "to", "of", "in", "for", "on", "with",
        "at", "by", "from", "as", "into", "through", "during", "before", "after",
        "and", "but", "or", "not", "no", "this", "that", "it", "i", "you", "he", "she",
        "we", "they", "what", "which", "who", "when", "where", "how", "all", "each",
        "every", "both", "few", "more", "most", "other", "some", "such", "than", "too",
        "very", "just", "about", "so", "if", "then", "also", "its", "my", "your",
    ];
    let stop: std::collections::HashSet<&str> = STOP_WORDS.iter().copied().collect();

    let mut freq: HashMap<String, usize> = HashMap::new();

    // 以空白 / 標點切分，統計詞頻
    let puncts = "，。！？、；：「」（）《》\u{201C}\u{201D}\u{2018}\u{2019}\u{2026}\u{2014}\u{00B7},.!?;:()[]{}";
    for word in content.split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || puncts.contains(c)) {
        let w = word.trim().to_lowercase();
        if w.len() < 2 || w.len() > 20 || stop.contains(w.as_str()) { continue; }
        if w.chars().all(|c| c.is_ascii_digit()) { continue; } // 純數字跳過
        *freq.entry(w).or_insert(0) += 1;
    }

    let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1));

    let tags: Vec<String> = pairs.into_iter()
        .take(5)
        .map(|(w, _)| w)
        .collect();

    if tags.is_empty() {
        vec!["未分類".to_string()]
    } else {
        tags
    }
}
