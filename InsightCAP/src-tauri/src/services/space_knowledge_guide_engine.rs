use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::prompts::{SPACE_KNOWLEDGE_GUIDE_PROMPT, SPACE_KNOWLEDGE_GUIDE_SYSTEM};
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::settings::store::get_settings;

const MAX_CHUNKS_PER_UPDATE: i64 = 30;

pub struct SpaceKnowledgeGuideEngine {
    pool: SqlitePool,
}

impl SpaceKnowledgeGuideEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn generate_guide(&self, space_id: &str) -> Result<String, String> {
        let settings = get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let output_language = output_language_label(&settings.general.language);

        let space_row = sqlx::query(
            "SELECT name, knowledge_guide_content FROM spaces WHERE id = ? AND is_archived = 0",
        )
        .bind(space_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Space {} not found", space_id))?;

        let space_name: String = space_row.try_get("name").unwrap_or_default();
        let existing_guide: String = space_row
            .try_get("knowledge_guide_content")
            .unwrap_or_default();

        let chunk_rows = sqlx::query(
            "SELECT content, knowledge_type FROM memory_chunks \
             WHERE space_id = ? AND status = 'active' AND pending_confirm = 0 \
             ORDER BY CASE knowledge_type WHEN 'pattern' THEN 0 WHEN 'log' THEN 1 ELSE 2 END, weight DESC, created_at DESC \
             LIMIT ?",
        )
        .bind(space_id)
        .bind(MAX_CHUNKS_PER_UPDATE)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        if chunk_rows.is_empty() {
            return Ok(String::new());
        }

        let chunks_text = chunk_rows
            .iter()
            .map(|row| {
                let knowledge_type: String = row
                    .try_get("knowledge_type")
                    .unwrap_or_else(|_| "data".to_string());
                let content: String = row.try_get("content").unwrap_or_default();
                let label = match knowledge_type.as_str() {
                    "pattern" => "[Pattern]",
                    "log" => "[Log]",
                    _ => "[Data]",
                };

                format!(
                    "{} {}",
                    label,
                    content.chars().take(200).collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let mut user_prompt = format!(
            "{}\n\nOutput language: {}\n\nSpace name: {}\n\nMemory chunks:\n{}\n",
            SPACE_KNOWLEDGE_GUIDE_PROMPT, output_language, space_name, chunks_text
        );

        if !existing_guide.trim().is_empty() {
            user_prompt.push_str(&format!(
                "\nExisting Knowledge Guide. Update it incrementally based on the new chunks:\n{}\n",
                existing_guide
            ));
        }

        let cfg = settings.ai_models.content_processor_llm;
        let api_key = cfg.api_key.clone().unwrap_or_default();
        let is_ollama = cfg.provider == "ollama";

        if api_key.is_empty() && !is_ollama {
            return Ok(String::new());
        }

        let llm = OpenAiProvider::new(
            api_key,
            cfg.base_url.clone(),
            cfg.model.clone(),
            cfg.provider.clone(),
        );
        let opts = LLMOptions {
            temperature: 0.3,
            max_tokens: 1500,
            stream: false,
            think_mode: None,
        };

        let full_prompt = format!("{}\n\n{}", SPACE_KNOWLEDGE_GUIDE_SYSTEM, user_prompt);
        let result = llm
            .complete(&full_prompt, opts)
            .await
            .map_err(|e| e.to_string())?;

        let result = result.trim().to_string();
        if result == "INSUFFICIENT" || result.is_empty() {
            return Ok(String::new());
        }

        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE spaces SET knowledge_guide_content = ?, knowledge_guide_updated_at = ? WHERE id = ?",
        )
        .bind(&result)
        .bind(&now)
        .bind(space_id)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(result)
    }
}

fn output_language_label(language: &str) -> &'static str {
    match language {
        "zh-CN" => "Simplified Chinese",
        "en" => "English",
        _ => "Traditional Chinese",
    }
}
