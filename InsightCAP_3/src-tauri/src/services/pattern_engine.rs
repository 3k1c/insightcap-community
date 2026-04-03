use sqlx::{SqlitePool, Row};
use uuid::Uuid;
use chrono::Utc;
use crate::prompts;
use crate::providers::llm::LLMProvider;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::LLMOptions;
use crate::settings::store::get_settings;

pub struct PatternEngine {
    pool: SqlitePool,
}

impl PatternEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 路徑 B 核心邏輯：偵測不同對話中是否有重複出現的模式
    /// 例如：尋找 `memory_chunks` 中 type 為 'pattern' 的記錄，
    /// 若有多個對話提及相同的 tag 或內容高度重疊，則觸發「升格推廣」。
    pub async fn detect_and_promote_patterns(&self) -> Result<usize, String> {
        let db = &self.pool;
        
        // 取得未被升格的 pattern
        let rows = sqlx::query(
            r#"
            SELECT id, conversation_id, content 
            FROM memory_chunks 
            WHERE knowledge_type = 'pattern' AND promoted_capture_id IS NULL
            ORDER BY created_at DESC LIMIT 50
            "#
        )
        .fetch_all(db)
        .await
        .map_err(|e| e.to_string())?;

        if rows.len() < 2 {
            return Ok(0);
        }

        let mut combined_text = String::new();
        for r in rows.iter() {
            let conv_id: String = r.get("conversation_id");
            let content: String = r.get("content");
            combined_text.push_str(&format!("- 對話 {}: {}\n", conv_id, content));
        }

        let settings = get_settings(db).await.map_err(|e| e.to_string())?;
        let llm_cfg = settings.ai_models.content_processor_llm;
        let api_key = llm_cfg.api_key.unwrap_or_default();
        if api_key.is_empty() { return Ok(0); }

        let provider = OpenAiProvider::new(api_key, llm_cfg.base_url, llm_cfg.model);
        // 此處我們使用使用者的設定
        let prompt_input = format!("{sys}\n\n[多對話紀錄]\n{data}", sys=prompts::PATTERN_ANALYSIS, data=combined_text);
        let response: String = provider.complete(&prompt_input, LLMOptions::default()).await.map_err(|e| e.to_string())?;
        if response.trim().to_uppercase().contains("NONE") {
            return Ok(0);
        }

        let capture_id = Uuid::now_v7().to_string();
        let now = Utc::now().to_rfc3339();

        // 存入 pending_confirm 狀態的 capture 供使用者確認
        sqlx::query(
            "INSERT INTO captures (id, source_id, type, raw_content, clean_content, capture_method, chunk_index, status, created_at, updated_at) VALUES (?, NULL, 'text', ?, ?, 'pattern_promotion', 0, 'pending_confirm', ?, ?)"
        )
        .bind(&capture_id)
        .bind(&response)
        .bind(&response)
        .bind(&now)
        .bind(&now)
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;

        for r in rows.iter() {
            let row_id: String = r.get("id");
            let _ = sqlx::query("UPDATE memory_chunks SET promoted_capture_id = ? WHERE id = ?")
                .bind(&capture_id)
                .bind(&row_id)
                .execute(db)
                .await;
        }

        Ok(1)
    }
}
