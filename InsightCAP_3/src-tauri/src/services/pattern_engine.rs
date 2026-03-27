use sqlx::{SqlitePool, Row};
use uuid::Uuid;
use chrono::Utc;
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
            SELECT id, session_id, content, importance 
            FROM memory_chunks 
            WHERE type = 'pattern' AND promoted_capture_id IS NULL
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
            let session_id: String = r.get("session_id");
            let content: String = r.get("content");
            combined_text.push_str(&format!("- 對話 {}: {}\n", session_id, content));
        }

        let settings = get_settings(db).await.map_err(|e| e.to_string())?;
        let llm_cfg = settings.ai_models.content_processor_llm;
        let api_key = llm_cfg.api_key.unwrap_or_default();
        if api_key.is_empty() { return Ok(0); }

        let provider = OpenAiProvider::new(api_key, llm_cfg.base_url, llm_cfg.model);
        let system_prompt = "你是一個跨對話記憶分析引擎。請檢視以下來自多個對話的重點模式(Patterns)。如果發現有多個對話中重複出現的強烈共同概念、問題或需求，請輸出一個統一的總結(Pattern)。這將被用來升格為正式知識點。若沒有明顯交集，不要硬湊，請嚴格輸出 'NONE'。";
        
        // 此處我們使用使用者的設定
        let prompt_input = format!("{sys}\n\n[多對話紀錄]\n{data}", sys=system_prompt, data=combined_text);
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
