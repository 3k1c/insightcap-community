use crate::prompts::SPACE_KNOWLEDGE_GUIDE_PROMPT;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::settings::store::get_settings;
use chrono::Utc;
use sqlx::{Row, SqlitePool};

const SPACE_KNOWLEDGE_GUIDE_SYSTEM: &str = "你是一個資深知識管理專家。你的任務是分析提供的片段內容，為該空間（Space）總結出一份結構化的知識導引。這份導引應包含：核心概念、關鍵字、常用模式。如果內容不足以產出有意義的導引，請回傳 INSUFFICIENT。";

pub struct SpaceKnowledgeGuideEngine {
    pool: SqlitePool,
}

impl SpaceKnowledgeGuideEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 為指定 Space 生成或更新知識導引
    pub async fn generate_guide(&self, space_id: &str) -> Result<String, String> {
        // 1. 取得設定
        let settings = get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let cfg = settings.ai_models.content_processor_llm;

        let api_key = cfg.api_key.unwrap_or_default();
        if api_key.is_empty() && cfg.provider != "ollama" {
            return Err("未設定 AI API Key".to_string());
        }

        // 2. 取得 Space 中的內容片段（Top 50 按使用頻率或權重）
        let rows = sqlx::query(
            "SELECT content FROM memory_chunks WHERE space_id = ? AND status = 'active' ORDER BY weight DESC LIMIT 50"
        )
        .bind(space_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        if rows.is_empty() {
            return Ok(String::new());
        }

        let mut all_content = String::new();
        for r in rows {
            let content: String = r.get("content");
            all_content.push_str(&content);
            all_content.push_str("\n---\n");
        }

        // 3. 準備 Prompt
        let user_prompt = format!(
            "{}\n\n待分析內容：\n{}",
            SPACE_KNOWLEDGE_GUIDE_PROMPT, all_content
        );

        // 4. 初始化 LLM
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

        // 5. 呼叫 LLM
        let full_prompt = format!("{}\n\n{}", SPACE_KNOWLEDGE_GUIDE_SYSTEM, user_prompt);
        let result = llm
            .complete(&full_prompt, opts)
            .await
            .map_err(|e| e.to_string())?;

        let result = result.trim().to_string();
        if result == "INSUFFICIENT" || result.is_empty() {
            return Ok(String::new());
        }

        // 6. 寫回 DB
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE spaces SET knowledge_guide_content = ?, knowledge_guide_updated_at = ? WHERE id = ?"
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
