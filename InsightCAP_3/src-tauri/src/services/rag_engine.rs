use sqlx::{Row, SqlitePool};
use serde_json::json;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::providers::llm::openai::OpenAiProvider;

pub struct RagEngine {
    pool: SqlitePool,
}

impl RagEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 根據 query 返回最佳的 Context (分層召回：MemoryChunks > Sources > Captures)
    pub async fn retrieve_context(&self, query: &str) -> Result<serde_json::Value, String> {
        // 先嘗試 LIKE 檢索，若無則抓最新 3 筆 context
        let query_like = format!("%{}%", query);
        
        let rows = sqlx::query(
            "SELECT clean_content, source_id FROM captures WHERE clean_content LIKE ? ORDER BY created_at DESC LIMIT 3"
        )
        .bind(&query_like)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut context: Vec<String> = if rows.is_empty() {
            sqlx::query("SELECT clean_content, source_id FROM captures ORDER BY created_at DESC LIMIT 3")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|r| r.get::<String, _>("clean_content"))
                .collect()
        } else {
            rows.into_iter().map(|r| r.get::<String, _>("clean_content")).collect()
        };

        // 新增：嘗試從已連線的外部知識庫中聚合結果 (Phase 5)
        let external_dbs: Vec<String> = sqlx::query_scalar(
            "SELECT uri FROM external_knowledge_bases WHERE status = 'connected'"
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        for db_path in external_dbs {
            let url = format!("sqlite:{}?mode=ro", db_path);
            if let Ok(ext_pool) = sqlx::SqlitePool::connect(&url).await {
                if let Ok(ext_rows) = sqlx::query(
                    "SELECT clean_content FROM captures WHERE clean_content LIKE ? ORDER BY created_at DESC LIMIT 2"
                )
                .bind(&query_like)
                .fetch_all(&ext_pool)
                .await {
                    for r in ext_rows {
                        if let Ok(c) = r.try_get::<String, _>("clean_content") {
                            context.push(format!("[外部知識庫] {}", c));
                        }
                    }
                }
                let _ = ext_pool.close().await;
            }
        }

        Ok(json!({
            "memory_chunks": [],
            "sources": [],
            "captures": context
        }))
    }

    pub async fn generate_answer(&self, query: &str, _project_id: Option<String>) -> Result<serde_json::Value, String> {
        // 1. 取得 LLM 設定
        let settings = crate::settings::store::get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let cfg = settings.ai_models.chat_llm;
        
        let mut opt_provider: Option<OpenAiProvider> = None;
        let is_ollama = cfg.provider == "ollama";
        let api_key = cfg.api_key.unwrap_or_default();
        
        if !api_key.is_empty() || is_ollama {
            opt_provider = Some(OpenAiProvider::new(api_key, cfg.base_url, cfg.model));
        }

        let provider = format!("This is a simulated AI answer based on local constraints since no API key is provided.\nUser Query: {}", query);
        
        // 2. 啟動 RAG 上下文拼裝
        let context_json = self.retrieve_context(query).await?;
        let captures = context_json["captures"].as_array().unwrap();
        let mut context_str = String::new();
        for (i, c) in captures.iter().enumerate() {
            context_str.push_str(&format!("Context {}:\n{}\n\n", i + 1, c.as_str().unwrap_or_default()));
        }

        let is_simulated = opt_provider.is_none();
        
        // 3. 呼叫 LLM 或是回退 simulated response
        let answer = if let Some(llm) = opt_provider {
            let prompt = format!(
                "You are InsightCAP, an AI assistant. Use the following context to answer the user's question.\n\nContext:\n{}\n\nQuestion: {}",
                context_str, query
            );
            llm.complete(&prompt, LLMOptions::default()).await.map_err(|e| e.to_string())?
        } else {
            format!("{}\n\nContext retrieved: {} items.", provider, captures.len())
        };

        Ok(json!({
            "answer": answer,
            "context_used": context_json,
            "is_simulated": is_simulated
        }))
    }
}
