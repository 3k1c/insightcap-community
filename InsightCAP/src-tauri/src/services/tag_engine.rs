use sqlx::SqlitePool;

pub struct TagEngine {
    pool: SqlitePool,
}

impl TagEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn process_new_capture(
        &self,
        capture_id: &str,
        content: &str,
    ) -> Result<Vec<String>, String> {
        let settings = crate::settings::store::get_settings(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let output_language = output_language_label(&settings.general.language);
        let cfg = settings.ai_models.content_processor_llm;

        let mut opt_provider: Option<crate::providers::llm::openai::OpenAiProvider> = None;
        let is_ollama = cfg.provider == "ollama";
        let api_key = cfg.api_key.unwrap_or_default();

        if !api_key.is_empty() || is_ollama {
            opt_provider = Some(crate::providers::llm::openai::OpenAiProvider::new(
                api_key,
                cfg.base_url,
                cfg.model,
                cfg.provider.clone(),
            ));
        }

        use crate::providers::llm::LLMProvider;
        let generated_tags = if let Some(llm) = opt_provider {
            let byte_limit = content.len().min(2000);
            let safe_limit = content.floor_char_boundary(byte_limit);
            let sample = &content[..safe_limit];
            let prompt = format!(
                "Extract 3-5 tags from the following text to represent its core concepts. \
                Return ONLY a comma-separated list of short tags in {output_language}. \
                Preserve dominant technical terms exactly as written. NO other text.\n\nText:\n{sample}"
            );
            match llm
                .complete(&prompt, crate::providers::llm::LLMOptions::default())
                .await
            {
                Ok(response) => {
                    let tags: Vec<String> = response
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s.len() < 30)
                        .take(5)
                        .collect();
                    if tags.is_empty() {
                        extract_keyword_tags(content)
                    } else {
                        tags
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[TagEngine] LLM failed, falling back to keyword extraction: {}",
                        e
                    );
                    extract_keyword_tags(content)
                }
            }
        } else {
            extract_keyword_tags(content)
        };

        let tags_json = serde_json::to_string(&generated_tags).unwrap();
        sqlx::query("UPDATE captures SET tags = ? WHERE id = ?")
            .bind(&tags_json)
            .bind(capture_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

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

    pub async fn process_source(
        &self,
        source_id: &str,
        full_content: &str,
    ) -> Result<Vec<String>, String> {
        let settings = crate::settings::store::get_settings(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let output_language = output_language_label(&settings.general.language);
        let cfg = settings.ai_models.content_processor_llm;

        let mut opt_provider: Option<crate::providers::llm::openai::OpenAiProvider> = None;
        let is_ollama = cfg.provider == "ollama";
        let api_key = cfg.api_key.unwrap_or_default();

        if !api_key.is_empty() || is_ollama {
            opt_provider = Some(crate::providers::llm::openai::OpenAiProvider::new(
                api_key,
                cfg.base_url,
                cfg.model,
                cfg.provider.clone(),
            ));
        }

        use crate::providers::llm::LLMProvider;
        let byte_limit = full_content.len().min(3000);
        let safe_limit = full_content.floor_char_boundary(byte_limit);
        let sample = &full_content[..safe_limit];
        let generated_tags = if let Some(llm) = opt_provider {
            let prompt = format!(
                "Extract 3-5 tags from the following text to represent its core concepts. \
                Return ONLY a comma-separated list of short tags in {output_language}. \
                Preserve dominant technical terms exactly as written. NO other text.\n\nText:\n{}",
                sample
            );
            match llm
                .complete(&prompt, crate::providers::llm::LLMOptions::default())
                .await
            {
                Ok(response) => {
                    let tags: Vec<String> = response
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s.len() < 30)
                        .take(5)
                        .collect();
                    if tags.is_empty() {
                        extract_keyword_tags(full_content)
                    } else {
                        tags
                    }
                }
                Err(e) => {
                    eprintln!("[TagEngine] Source LLM failed, fallback: {}", e);
                    extract_keyword_tags(full_content)
                }
            }
        } else {
            extract_keyword_tags(full_content)
        };

        let tags_json = serde_json::to_string(&generated_tags).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE sources SET tags = ?, updated_at = ? WHERE id = ?")
            .bind(&tags_json)
            .bind(&now)
            .bind(source_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        self.upsert_tags(&generated_tags).await?;

        Ok(generated_tags)
    }

    pub async fn upsert_tags(&self, tags: &[String]) -> Result<(), String> {
        for tag in tags {
            if tag == "untagged" {
                continue;
            }
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

fn output_language_label(language: &str) -> &'static str {
    match language {
        "zh-CN" => "Simplified Chinese",
        "en" => "English",
        _ => "Traditional Chinese",
    }
}

fn extract_keyword_tags(content: &str) -> Vec<String> {
    use std::collections::HashMap;

    const EN_STOP: &[&str] = &[
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "shall", "should", "may", "might", "must", "can",
        "could", "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into",
        "through", "during", "before", "after", "and", "but", "or", "not", "no", "this", "that",
        "it", "i", "you", "he", "she", "we", "they", "what", "which", "who", "when", "where",
        "how", "all", "each", "every", "both", "few", "more", "most", "other", "some", "such",
        "than", "too", "very", "just", "about", "so", "if", "then", "also", "its", "my", "your",
    ];
    const ZH_STOP: &[&str] = &[
        "\u{7684}",
        "\u{4e86}",
        "\u{5728}",
        "\u{662f}",
        "\u{6211}",
        "\u{6709}",
        "\u{548c}",
        "\u{5c31}",
        "\u{4e0d}",
        "\u{4eba}",
        "\u{90fd}",
        "\u{4e00}",
        "\u{4e00}\u{500b}",
        "\u{4e0a}",
        "\u{4e5f}",
        "\u{5f88}",
        "\u{5230}",
        "\u{8aaa}",
        "\u{8981}",
        "\u{53bb}",
        "\u{4f60}",
        "\u{6703}",
        "\u{8457}",
        "\u{6c92}\u{6709}",
        "\u{770b}",
        "\u{597d}",
        "\u{81ea}\u{5df1}",
        "\u{9019}",
        "\u{4ed6}",
        "\u{5979}",
        "\u{5b83}",
        "\u{5011}",
        "\u{90a3}",
        "\u{88ab}",
        "\u{5f9e}",
        "\u{628a}",
        "\u{8b93}",
        "\u{7528}",
        "\u{5c0d}",
    ];
    let stop: std::collections::HashSet<&str> =
        EN_STOP.iter().chain(ZH_STOP.iter()).copied().collect();

    let mut freq: HashMap<String, usize> = HashMap::new();

    let puncts = "\u{3002}\u{ff0c}\u{3001}\u{ff1f}\u{ff01}\u{ff1b}\u{ff1a}\u{ff08}\u{ff09}\u{3010}\u{3011}\u{300a}\u{300b}\u{300c}\u{300d}\u{300e}\u{300f}\u{201c}\u{201d}\u{2018}\u{2019}\u{2026}\u{2014}\u{00b7},.!?;:()[]{}";
    for word in
        content.split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || puncts.contains(c))
    {
        let w = word.trim().to_lowercase();
        if w.len() < 2 || w.len() > 20 || stop.contains(w.as_str()) {
            continue;
        }
        if w.chars().all(|c| c.is_ascii_digit()) {
            continue;
        } //
        *freq.entry(w).or_insert(0) += 1;
    }

    let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1));

    let tags: Vec<String> = pairs.into_iter().take(5).map(|(w, _)| w).collect();

    if tags.is_empty() {
        vec!["untagged".to_string()]
    } else {
        tags
    }
}
