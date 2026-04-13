use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;
use chrono::Utc;
use crate::prompts;
use crate::providers::llm::LLMProvider;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::LLMOptions;
use crate::providers::embedding::Embedder;
use crate::settings::store::get_settings;

/// 向量相似度門檻
const SIMILARITY_THRESHOLD: f32 = 0.65;
/// 最少需要出現在幾個不同對話
const MIN_CONVERSATIONS: usize = 3;
/// 最少需要幾個共同標籤
const MIN_TAG_OVERLAP: usize = 2;

pub struct PatternEngine {
    pool: SqlitePool,
    embedder: Arc<dyn Embedder>,
}

/// 候選 chunk 結構
struct Candidate {
    id: String,
    conversation_id: String,
    content: String,
    tags: HashSet<String>,
    embedding: Option<Vec<f32>>,
}

/// 一組可升格的候選群
struct PromotionGroup {
    candidates: Vec<Candidate>,
    conversation_count: usize,
}

impl PatternEngine {
    pub fn new(pool: SqlitePool, embedder: Arc<dyn Embedder>) -> Self {
        Self { pool, embedder }
    }

    /// 路徑 B 核心邏輯：跨對話自動識別重複模式
    /// 條件：標籤重疊 ≥ 2 + 向量相似度 ≥ 0.65 + 出現在 ≥ 3 個不同對話
    pub async fn detect_and_promote_patterns(&self) -> Result<usize, String> {
        let db = &self.pool;

        // Step 1: 取得尚未升格的 data 類型 chunks（最近 100 筆）
        let rows = sqlx::query(
            r#"
            SELECT id, conversation_id, content, tags
            FROM memory_chunks
            WHERE knowledge_type = 'data'
              AND pending_confirm = 0
              AND promoted_capture_id IS NULL
              AND conversation_id IS NOT NULL
            ORDER BY created_at DESC LIMIT 100
            "#
        )
        .fetch_all(db)
        .await
        .map_err(|e| e.to_string())?;

        if rows.len() < MIN_CONVERSATIONS {
            return Ok(0);
        }

        // Step 2: 解析為 Candidate，計算 embedding
        let mut candidates: Vec<Candidate> = Vec::new();
        for r in &rows {
            let id: String = r.get("id");
            let conv_id: String = r.try_get("conversation_id").unwrap_or_default();
            let content: String = r.get("content");
            let tags_json: String = r.try_get("tags").unwrap_or_else(|_| "[]".to_string());
            let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            let embedding = match self.embedder.embed(&content).await {
                Ok(v) => Some(v),
                Err(_) => None,
            };

            candidates.push(Candidate { id, conversation_id: conv_id, content, tags, embedding });
        }

        // Step 3: 找出符合三個條件的候選群
        let groups = self.find_promotion_groups(&candidates);
        if groups.is_empty() {
            return Ok(0);
        }

        // Step 4: 對每個候選群呼叫 LLM 分析
        let settings = get_settings(db).await.map_err(|e| e.to_string())?;
        let llm_cfg = settings.ai_models.content_processor_llm;
        let api_key = llm_cfg.api_key.unwrap_or_default();
        if api_key.is_empty() { return Ok(0); }

        let provider = OpenAiProvider::new(api_key, llm_cfg.base_url, llm_cfg.model, llm_cfg.provider.clone());
        let mut promoted_count = 0;

        for group in &groups {
            match self.promote_group(db, &provider, group).await {
                Ok(true) => promoted_count += 1,
                Ok(false) => {}
                Err(e) => eprintln!("[PATTERN-ENGINE] 群組升格失敗: {}", e),
            }
        }

        // Step 5: 擴展 Log 的 trigger_context
        if promoted_count > 0 {
            let _ = self.expand_log_trigger_contexts(db).await;
        }

        Ok(promoted_count)
    }

    /// 找出符合條件的候選群：標籤重疊 ≥ 2、向量相似度 ≥ 0.65、≥ 3 個不同對話
    fn find_promotion_groups(&self, candidates: &[Candidate]) -> Vec<PromotionGroup> {
        let mut groups: Vec<PromotionGroup> = Vec::new();
        let mut used_ids: HashSet<String> = HashSet::new();

        for (i, anchor) in candidates.iter().enumerate() {
            if used_ids.contains(&anchor.id) { continue; }

            let mut group_candidates = vec![i];
            let mut group_conversations: HashSet<&str> = HashSet::new();
            group_conversations.insert(&anchor.conversation_id);

            for (j, other) in candidates.iter().enumerate() {
                if i == j || used_ids.contains(&other.id) { continue; }

                // 條件 1: 標籤重疊 ≥ 2
                let tag_overlap = anchor.tags.intersection(&other.tags).count();
                if tag_overlap < MIN_TAG_OVERLAP { continue; }

                // 條件 2: 向量相似度 ≥ 0.65
                if let (Some(ref v1), Some(ref v2)) = (&anchor.embedding, &other.embedding) {
                    let sim = cosine_similarity(v1, v2);
                    if sim < SIMILARITY_THRESHOLD { continue; }
                } else {
                    continue;
                }

                group_candidates.push(j);
                group_conversations.insert(&other.conversation_id);
            }

            // 條件 3: ≥ 3 個不同對話
            if group_conversations.len() >= MIN_CONVERSATIONS && group_candidates.len() >= MIN_CONVERSATIONS {
                for &idx in &group_candidates {
                    used_ids.insert(candidates[idx].id.clone());
                }
                // 從 indices 取出實際 Candidate ref 建立 group
                // 由於 borrow checker，我們只存 ids
                groups.push(PromotionGroup {
                    candidates: group_candidates.iter().map(|&idx| {
                        let c = &candidates[idx];
                        Candidate {
                            id: c.id.clone(),
                            conversation_id: c.conversation_id.clone(),
                            content: c.content.clone(),
                            tags: c.tags.clone(),
                            embedding: None, // 不需要再存 embedding
                        }
                    }).collect(),
                    conversation_count: group_conversations.len(),
                });
            }
        }

        groups
    }

    /// 對一個候選群呼叫 LLM 並寫入 memory_chunks（pending_confirm = 1）
    async fn promote_group(
        &self,
        db: &SqlitePool,
        provider: &OpenAiProvider,
        group: &PromotionGroup,
    ) -> Result<bool, String> {
        let mut combined_text = String::new();
        let mut all_tags: HashMap<String, usize> = HashMap::new();

        for c in &group.candidates {
            combined_text.push_str(&format!("- 對話 {}: {}\n", c.conversation_id, c.content));
            for tag in &c.tags {
                *all_tags.entry(tag.clone()).or_insert(0) += 1;
            }
        }

        let prompt_input = format!(
            "{sys}\n\n來自 {conv_count} 個不同對話的 {chunk_count} 筆資料：\n{data}",
            sys = prompts::PATTERN_ANALYSIS,
            conv_count = group.conversation_count,
            chunk_count = group.candidates.len(),
            data = combined_text
        );

        let response: String = provider
            .complete(&prompt_input, LLMOptions::default())
            .await
            .map_err(|e| e.to_string())?;

        if response.trim().to_uppercase().contains("NONE") {
            return Ok(false);
        }

        // 取出現次數 ≥ 2 的共同標籤
        let common_tags: Vec<String> = all_tags
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .map(|(tag, _)| tag)
            .collect();

        let chunk_id = Uuid::now_v7().to_string();
        let now = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(&common_tags).unwrap_or_else(|_| "[]".to_string());

        // 寫入 memory_chunks，pending_confirm = 1
        sqlx::query(
            "INSERT INTO memory_chunks (id, knowledge_type, content, tags, confidence, pending_confirm, placed_by, created_at, updated_at) \
             VALUES (?, 'pattern', ?, ?, 0.70, 1, 'ai', ?, ?)"
        )
        .bind(&chunk_id)
        .bind(&response)
        .bind(&tags_json)
        .bind(&now)
        .bind(&now)
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;

        // 標記原始 candidates 的 promoted_capture_id
        for c in &group.candidates {
            let _ = sqlx::query("UPDATE memory_chunks SET promoted_capture_id = ? WHERE id = ?")
                .bind(&chunk_id)
                .bind(&c.id)
                .execute(db)
                .await;
        }

        Ok(true)
    }

    /// 擴展 Log 的 trigger_context：掃描 log 類型 chunks，
    /// 將近期相似對話的關鍵字追加到 trigger_context
    async fn expand_log_trigger_contexts(&self, db: &SqlitePool) -> Result<(), String> {
        let logs = sqlx::query(
            "SELECT id, content, trigger_context FROM memory_chunks \
             WHERE knowledge_type = 'log' ORDER BY created_at DESC LIMIT 20"
        )
        .fetch_all(db)
        .await
        .map_err(|e| e.to_string())?;

        for log_row in &logs {
            let log_id: String = log_row.get("id");
            let content: String = log_row.get("content");
            let existing_ctx: String = log_row.try_get("trigger_context").unwrap_or_default();

            // 用 embedding 找近似的 data chunks
            let log_vec = match self.embedder.embed(&content).await {
                Ok(v) => v,
                Err(_) => continue,
            };

            let similar_data = sqlx::query(
                "SELECT content FROM memory_chunks \
                 WHERE knowledge_type = 'data' AND id != ? \
                 ORDER BY created_at DESC LIMIT 20"
            )
            .bind(&log_id)
            .fetch_all(db)
            .await
            .map_err(|e| e.to_string())?;

            let mut new_contexts: Vec<String> = Vec::new();
            let existing_set: HashSet<&str> = existing_ctx.split('|').filter(|s| !s.is_empty()).collect();

            for data_row in &similar_data {
                let data_content: String = data_row.get("content");
                let data_vec = match self.embedder.embed(&data_content).await {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let sim = cosine_similarity(&log_vec, &data_vec);
                if sim >= 0.60 {
                    // 取前 30 字元作為 trigger keyword
                    let keyword = data_content.chars().take(30).collect::<String>();
                    if !existing_set.contains(keyword.as_str()) && !new_contexts.contains(&keyword) {
                        new_contexts.push(keyword);
                    }
                }

                // 最多擴展 3 個新 context
                if new_contexts.len() >= 3 { break; }
            }

            if !new_contexts.is_empty() {
                let updated_ctx = if existing_ctx.is_empty() {
                    new_contexts.join("|")
                } else {
                    format!("{}|{}", existing_ctx, new_contexts.join("|"))
                };

                let _ = sqlx::query(
                    "UPDATE memory_chunks SET trigger_context = ?, updated_at = ? WHERE id = ?"
                )
                .bind(&updated_ctx)
                .bind(Utc::now().to_rfc3339())
                .bind(&log_id)
                .execute(db)
                .await;
            }
        }

        Ok(())
    }
}

/// 餘弦相似度
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { return 0.0; }
    dot / (norm_a * norm_b)
}
