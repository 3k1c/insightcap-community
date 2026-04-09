use sqlx::{Row, SqlitePool};
use chrono::Utc;

use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::prompts::SPACE_WIKI_SYSTEM;

/// 每次更新最多取前 N 個 memory_chunks 作為輸入（避免 context 過大）
const MAX_CHUNKS_PER_UPDATE: i64 = 30;

pub struct SpaceWikiEngine {
    pool: SqlitePool,
}

impl SpaceWikiEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 針對指定 space_id，用現有 memory_chunks 增量更新 wiki_content
    /// 回傳更新後的 wiki（空字串代表內容不足，不更新）
    pub async fn update_wiki_for_space(&self, space_id: &str) -> Result<String, String> {
        // 1. 取 Space 基本資訊（name + 現有 wiki）
        let space_row = sqlx::query(
            "SELECT name, wiki_content FROM spaces WHERE id = ? AND is_archived = 0"
        )
        .bind(space_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Space {} not found", space_id))?;

        let space_name: String = space_row.try_get("name").unwrap_or_default();
        let existing_wiki: String = space_row.try_get("wiki_content").unwrap_or_default();

        // 2. 取屬於此 Space 的 memory_chunks（已確認，按 knowledge_type 排序讓 pattern/log 優先）
        let mc_rows = sqlx::query(
            "SELECT content, knowledge_type FROM memory_chunks \
             WHERE space_id = ? AND pending_confirm = 0 \
             ORDER BY CASE knowledge_type WHEN 'pattern' THEN 0 WHEN 'log' THEN 1 ELSE 2 END, created_at DESC \
             LIMIT ?"
        )
        .bind(space_id)
        .bind(MAX_CHUNKS_PER_UPDATE)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        if mc_rows.is_empty() {
            return Ok(String::new());
        }

        // 3. 組裝輸入給 LLM 的 context
        let chunks_text: String = mc_rows.iter().map(|r| {
            let kt: String = r.try_get("knowledge_type").unwrap_or_else(|_| "data".to_string());
            let content: String = r.try_get("content").unwrap_or_default();
            let label = match kt.as_str() {
                "pattern" => "【Pattern】",
                "log"     => "【Log】",
                _         => "【Data】",
            };
            format!("{} {}", label, content.chars().take(200).collect::<String>())
        }).collect::<Vec<_>>().join("\n\n");

        // 4. 組裝 prompt
        let mut user_prompt = format!(
            "Space 名稱：{}\n\n記憶 chunks：\n{}\n",
            space_name, chunks_text
        );
        if !existing_wiki.trim().is_empty() {
            user_prompt.push_str(&format!(
                "\n現有 Wiki（請在此基礎上增量更新）：\n{}\n",
                existing_wiki
            ));
        }

        // 5. 讀取 LLM 設定
        let settings = crate::settings::store::get_settings(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let cfg = settings.ai_models.content_processor_llm;
        let api_key = cfg.api_key.clone().unwrap_or_default();
        let is_ollama = cfg.provider == "ollama";

        if api_key.is_empty() && !is_ollama {
            return Ok(String::new());
        }

        let llm = OpenAiProvider::new(api_key, cfg.base_url.clone(), cfg.model.clone(), cfg.provider.clone());
        let opts = LLMOptions { temperature: 0.3, max_tokens: 1500, stream: false, think_mode: None };

        // 6. 呼叫 LLM（以 SPACE_WIKI_SYSTEM 為 system，user_prompt 為 user）
        let full_prompt = format!("{}\n\n{}", SPACE_WIKI_SYSTEM, user_prompt);
        let result = llm.complete(&full_prompt, opts)
            .await
            .map_err(|e| e.to_string())?;

        let result = result.trim().to_string();
        if result == "INSUFFICIENT" || result.is_empty() {
            return Ok(String::new());
        }

        // 7. 寫回 DB
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE spaces SET wiki_content = ?, wiki_updated_at = ? WHERE id = ?"
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
