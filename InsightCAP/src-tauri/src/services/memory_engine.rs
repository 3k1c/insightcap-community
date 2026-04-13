use std::sync::Arc;

use serde::Deserialize;
use sqlx::SqlitePool;
use uuid::Uuid;
use chrono::Utc;

use crate::providers::embedding::Embedder;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::settings::store::get_settings;
use crate::vector_store::local::VectorStore;

// ─── Tagger LLM 回傳結構 ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TaggerResult {
    tags: Vec<String>,
    knowledge_type: String,
}

pub struct MemoryEngine {
    pool: SqlitePool,
    vector_store: VectorStore,
    embedder: Arc<dyn Embedder>,
}

impl MemoryEngine {
    pub fn new(pool: SqlitePool, vector_store: VectorStore, embedder: Arc<dyn Embedder>) -> Self {
        Self { pool, vector_store, embedder }
    }

    // ─── 路徑 A：擷取入庫標籤提取（captures 固定 data）──────────────────────

    /// 對新 capture 執行輕量 tagger，只提取標籤，knowledge_type 固定 data
    pub async fn tag_capture(&self, capture_id: &str, content: &str) -> Result<Vec<String>, String> {
        let settings = get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let cfg = settings.ai_models.content_processor_llm;
        let api_key = cfg.api_key.clone().unwrap_or_default();

        // 若無 LLM 設定，降級為 untagged
        if api_key.is_empty() && cfg.provider != "ollama" {
            let fallback = vec!["untagged".to_string()];
            self.write_tags_to_capture(capture_id, &fallback).await?;
            return Ok(fallback);
        }

        let prompt = format!(
            "Analyze the following text and perform two tasks:\n\
             1. Extract 1 to 5 relevant tags (short keywords).\n\
             2. Classify the knowledge type into one of: 'data', 'pattern', 'log'.\n\
             Output ONLY a valid JSON object with keys \"tags\" and \"knowledge_type\".\n\
             Example: {{\"tags\": [\"rust\", \"memory\"], \"knowledge_type\": \"data\"}}\n\
             Do not output any other text or markdown.\n\n\
             Text:\n{content}",
            content = content
        );

        let provider = OpenAiProvider::new(api_key, cfg.base_url, cfg.model.clone(), cfg.provider.clone());
        let opts = LLMOptions { temperature: 0.1, max_tokens: 200, stream: false, think_mode: None };

        // 15 秒超時
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            provider.complete_json(&prompt, opts),
        )
        .await;

        let tags = match result {
            Ok(Ok(json)) => parse_tags_from_json(&json),
            _ => {
                eprintln!("[MemoryEngine] tag_capture timeout/error for {}", capture_id);
                vec!["untagged".to_string()]
            }
        };

        self.write_tags_to_capture(capture_id, &tags).await?;
        Ok(tags)
    }

    // ─── 路徑 B：對話總結深度推斷，寫入 memory_chunks ───────────────────────

    /// 對話總結後，深度推斷 knowledge_type，寫入 memory_chunks
    /// - confidence >= 0.75 → 直接寫入
    /// - confidence < 0.75  → 寫入但 pending_confirm = 1
    pub async fn process_conversation_summary(
        &self,
        conversation_id: &str,
        summary_text: &str,
        project_id: Option<&str>,
    ) -> Result<String, String> {
        let settings = get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let cfg = settings.ai_models.content_processor_llm;
        let api_key = cfg.api_key.clone().unwrap_or_default();

        // 若無 LLM，降級為 data + pending_confirm
        let (knowledge_type, tags, trigger_context, confidence) =
            if api_key.is_empty() && cfg.provider != "ollama" {
                ("data".to_string(), vec!["untagged".to_string()], String::new(), 0.5_f32)
            } else {
                self.deep_infer_knowledge_type(summary_text, &api_key, cfg.base_url.unwrap_or_default(), cfg.model).await
            };

        let pending_confirm = if confidence < 0.75 { 1_i32 } else { 0_i32 };

        // 向量化
        let vector_id = match self.embedder.embed(summary_text).await {
            Ok(vec) => {
                let chunk_id_hash = str_to_u64(conversation_id);
                if let Ok(()) = self.vector_store.add_vector(chunk_id_hash, &vec).await {
                    let vs = self.vector_store.clone();
                    tokio::spawn(async move { let _ = vs.save().await; });
                    Some(chunk_id_hash as i64)
                } else {
                    None
                }
            }
            Err(e) => {
                eprintln!("[MemoryEngine] embed failed: {}", e);
                None
            }
        };

        // 寫入 memory_chunks（UPSERT：同一 conversation_id 只保留最新一筆）
        let now = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());

        // 驗證 project_id 是否存在（sqlx 0.8 預設啟用 FK，不存在的 project_id 會觸發 constraint）
        let valid_project_id: Option<&str> = if let Some(pid) = project_id {
            let exists: bool = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM projects WHERE id = ?"
            )
            .bind(pid)
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0) > 0;
            if exists { Some(pid) } else { None }
        } else {
            None
        };

        // 查找是否已有此對話的 chunk
        let existing_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM memory_chunks WHERE conversation_id = ? LIMIT 1"
        )
        .bind(conversation_id)
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None);

        let chunk_id = if let Some(eid) = existing_id {
            // 更新既有記錄
            sqlx::query(
                "UPDATE memory_chunks SET \
                 project_id = ?, knowledge_type = ?, content = ?, tags = ?, \
                 trigger_context = ?, confidence = ?, pending_confirm = ?, \
                 vector_id = ?, updated_at = ? \
                 WHERE id = ?"
            )
            .bind(valid_project_id)
            .bind(&knowledge_type)
            .bind(summary_text)
            .bind(&tags_json)
            .bind(&trigger_context)
            .bind(confidence)
            .bind(pending_confirm)
            .bind(vector_id)
            .bind(&now)
            .bind(&eid)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
            eid
        } else {
            // 首次插入
            let chunk_id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO memory_chunks \
                 (id, conversation_id, project_id, knowledge_type, content, tags, \
                  trigger_context, confidence, pending_confirm, vector_id, placed_by, \
                  created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'ai', ?, ?)"
            )
            .bind(&chunk_id)
            .bind(conversation_id)
            .bind(valid_project_id)
            .bind(&knowledge_type)
            .bind(summary_text)
            .bind(&tags_json)
            .bind(&trigger_context)
            .bind(confidence)
            .bind(pending_confirm)
            .bind(vector_id)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
            chunk_id
        };

        println!(
            "[MemoryEngine] memory_chunk {} 寫入完成：type={}, confidence={:.2}, pending={}",
            chunk_id, knowledge_type, confidence, pending_confirm
        );
        Ok(chunk_id)
    }

    /// 用戶確認 pending_confirm 的 memory_chunk
    pub async fn confirm_memory_chunk(&self, chunk_id: &str, accept: bool) -> Result<(), String> {
        if accept {
            sqlx::query(
                "UPDATE memory_chunks SET pending_confirm = 0, updated_at = ? WHERE id = ?"
            )
            .bind(Utc::now().to_rfc3339())
            .bind(chunk_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        } else {
            // 拒絕 → 降為 data 並清除 pending
            sqlx::query(
                "UPDATE memory_chunks SET knowledge_type = 'data', pending_confirm = 0, updated_at = ? WHERE id = ?"
            )
            .bind(Utc::now().to_rfc3339())
            .bind(chunk_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // ─── 內部：深度推斷 knowledge_type ──────────────────────────────────────

    async fn deep_infer_knowledge_type(
        &self,
        content: &str,
        api_key: &str,
        base_url: String,
        model: String,
    ) -> (String, Vec<String>, String, f32) {
        let prompt = format!(
            "分析以下對話摘要，執行以下任務：\n\n\
             1. 提取 1 到 5 個相關標籤（簡短關鍵詞，可中英文）。\n\
             2. 將知識類型分類，嚴格標準如下：\n\n\
             - \"pattern\"：僅限對話中包含一個**已確認有效**的可複用方法、工作流程、SOP 或決策框架。\
               信號詞：「這樣做有效」、「決定用」、「確認方案」、「下次應該」、「最佳做法是」。\n\
             - \"log\"：僅限對話記錄了一個具體的錯誤、失敗方向、踩坑或教訓。\
               信號詞：「這樣不行」、「失敗了」、「錯誤是」、「不應該」、「問題出在」。\n\
             - \"data\"：其他所有情況。不確定時一律歸為 \"data\"。\n\n\
             3. 若為 \"log\" 類型，在 \"trigger_context\" 填入 pipe 分隔的場景關鍵詞\
               （例：\"競品分析 | 爬蟲\"）；其他類型留空。\n\
             4. 在 \"confidence\" 填入你對分類結果的信心分數（0.0 到 1.0 的浮點數）。\
               - \"data\" 且無明顯特徵：0.85-0.95\n\
               - \"pattern\" 或 \"log\" 有清晰信號：0.80-0.95\n\
               - \"pattern\" 或 \"log\" 信號模糊：0.50-0.75\n\n\
             只輸出合法 JSON 物件，包含鍵：\"tags\"、\"knowledge_type\"、\"trigger_context\"、\"confidence\"。\n\
             範例：{{\"tags\": [\"rust\", \"async\"], \"knowledge_type\": \"pattern\", \"trigger_context\": \"\", \"confidence\": 0.88}}\n\
             不要輸出任何其他文字或 markdown。\n\n\
             對話摘要：\n{content}",
            content = content
        );

        let provider = OpenAiProvider::new(api_key.to_string(), Some(base_url), model, String::new());
        let opts = LLMOptions { temperature: 0.1, max_tokens: 300, stream: false, think_mode: None };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            provider.complete_json(&prompt, opts),
        )
        .await;

        match result {
            Ok(Ok(json)) => {
                let knowledge_type = json["knowledge_type"]
                    .as_str()
                    .filter(|s| matches!(*s, "data" | "pattern" | "log"))
                    .unwrap_or("data")
                    .to_string();

                let tags = parse_tags_from_json(&json);
                let trigger_context = json["trigger_context"].as_str().unwrap_or("").to_string();

                // 直接使用 LLM 輸出的信心度，合法範圍 0.0-1.0
                let confidence = json["confidence"]
                    .as_f64()
                    .map(|f| f.clamp(0.0, 1.0) as f32)
                    .unwrap_or(if knowledge_type == "data" { 0.9 } else { 0.7 });

                (knowledge_type, tags, trigger_context, confidence)
            }
            _ => {
                eprintln!("[MemoryEngine] deep_infer timeout/error，降級為 data");
                ("data".to_string(), vec!["untagged".to_string()], String::new(), 0.5)
            }
        }
    }

    // ─── 內部：寫 tags 回 captures 表 ───────────────────────────────────────

    async fn write_tags_to_capture(&self, capture_id: &str, tags: &[String]) -> Result<(), String> {
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
        sqlx::query("UPDATE captures SET tags = ?, updated_at = ? WHERE id = ?")
            .bind(&tags_json)
            .bind(Utc::now().to_rfc3339())
            .bind(capture_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

// ─── 輔助函式 ────────────────────────────────────────────────────────────────

fn parse_tags_from_json(json: &serde_json::Value) -> Vec<String> {
    let raw = json["tags"].as_array();
    match raw {
        Some(arr) => {
            let mut tags: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if tags.is_empty() {
                tags.push("untagged".to_string());
            }
            tags
        }
        None => vec!["untagged".to_string()],
    }
}

/// 將 UUID v7 字串轉為 u64（取高 64 位），保證同輸入同輸出且碰撞極低
/// UUID v7 高 64 位包含時間戳 + 隨機位，比 DefaultHasher 更可靠
fn str_to_u64(s: &str) -> u64 {
    // 嘗試解析為 UUID，取高 64 位 bytes
    if let Ok(uuid) = s.parse::<uuid::Uuid>() {
        let bytes = uuid.as_bytes();
        u64::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]])
    } else {
        // 非 UUID 格式：用 FNV-1a 64-bit（比 DefaultHasher 穩定）
        let mut hash: u64 = 14695981039346656037;
        for byte in s.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
        hash
    }
}
