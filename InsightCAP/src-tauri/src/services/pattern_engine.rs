use crate::prompts;
use crate::providers::embedding::Embedder;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::settings::store::get_settings;
use chrono::Utc;
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

const SIMILARITY_THRESHOLD: f32 = 0.65;
const MIN_CONVERSATIONS: usize = 3;
const MIN_TAG_OVERLAP: usize = 2;

pub struct PatternEngine {
    pool: SqlitePool,
    embedder: Arc<dyn Embedder>,
}

struct Candidate {
    id: String,
    conversation_id: String,
    content: String,
    tags: HashSet<String>,
    embedding: Option<Vec<f32>>,
}

struct PromotionGroup {
    candidates: Vec<Candidate>,
    conversation_count: usize,
}

impl PatternEngine {
    pub fn new(pool: SqlitePool, embedder: Arc<dyn Embedder>) -> Self {
        Self { pool, embedder }
    }

    pub async fn detect_and_promote_patterns(&self) -> Result<usize, String> {
        let db = &self.pool;

        let rows = sqlx::query(
            r#"
            SELECT id, conversation_id, content, tags
            FROM memory_chunks
            WHERE knowledge_type = 'data'
              AND pending_confirm = 0
              AND promoted_capture_id IS NULL
              AND conversation_id IS NOT NULL
            ORDER BY created_at DESC LIMIT 100
            "#,
        )
        .fetch_all(db)
        .await
        .map_err(|e| e.to_string())?;

        if rows.len() < MIN_CONVERSATIONS {
            return Ok(0);
        }

        let mut candidates: Vec<Candidate> = Vec::new();
        for r in &rows {
            let id: String = r.get("id");
            let conv_id: String = r.try_get("conversation_id").unwrap_or_default();
            let content: String = r.get("content");
            let tags_json: String = r.try_get("tags").unwrap_or_else(|_| "[]".to_string());
            let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            let embedding = self.embedder.embed(&content).await.ok();

            candidates.push(Candidate {
                id,
                conversation_id: conv_id,
                content,
                tags,
                embedding,
            });
        }

        let groups = self.find_promotion_groups(&candidates);
        if groups.is_empty() {
            return Ok(0);
        }

        let settings = get_settings(db).await.map_err(|e| e.to_string())?;
        let llm_cfg = settings.ai_models.content_processor_llm;
        let api_key = llm_cfg.api_key.unwrap_or_default();
        if api_key.is_empty() {
            return Ok(0);
        }

        let provider = OpenAiProvider::new(
            api_key,
            llm_cfg.base_url,
            llm_cfg.model,
            llm_cfg.provider.clone(),
        );

        let mut promoted_count = 0;
        for group in &groups {
            match self.promote_group(db, &provider, group).await {
                Ok(true) => promoted_count += 1,
                Ok(false) => {}
                Err(e) => eprintln!("[PATTERN-ENGINE] Promote group failed: {}", e),
            }
        }

        if promoted_count > 0 {
            let _ = self.expand_log_trigger_contexts(db).await;
        }

        Ok(promoted_count)
    }

    fn find_promotion_groups(&self, candidates: &[Candidate]) -> Vec<PromotionGroup> {
        let mut groups: Vec<PromotionGroup> = Vec::new();
        let mut used_ids: HashSet<String> = HashSet::new();

        for (i, anchor) in candidates.iter().enumerate() {
            if used_ids.contains(&anchor.id) {
                continue;
            }

            let mut group_candidates = vec![i];
            let mut group_conversations: HashSet<&str> = HashSet::new();
            group_conversations.insert(&anchor.conversation_id);

            for (j, other) in candidates.iter().enumerate() {
                if i == j || used_ids.contains(&other.id) {
                    continue;
                }

                let tag_overlap = anchor.tags.intersection(&other.tags).count();
                if tag_overlap < MIN_TAG_OVERLAP {
                    continue;
                }

                if let (Some(v1), Some(v2)) = (&anchor.embedding, &other.embedding) {
                    let sim = cosine_similarity(v1, v2);
                    if sim < SIMILARITY_THRESHOLD {
                        continue;
                    }
                } else {
                    continue;
                }

                group_candidates.push(j);
                group_conversations.insert(&other.conversation_id);
            }

            if group_conversations.len() >= MIN_CONVERSATIONS
                && group_candidates.len() >= MIN_CONVERSATIONS
            {
                for &idx in &group_candidates {
                    used_ids.insert(candidates[idx].id.clone());
                }

                groups.push(PromotionGroup {
                    candidates: group_candidates
                        .iter()
                        .map(|&idx| {
                            let c = &candidates[idx];
                            Candidate {
                                id: c.id.clone(),
                                conversation_id: c.conversation_id.clone(),
                                content: c.content.clone(),
                                tags: c.tags.clone(),
                                embedding: None,
                            }
                        })
                        .collect(),
                    conversation_count: group_conversations.len(),
                });
            }
        }

        groups
    }

    async fn promote_group(
        &self,
        db: &SqlitePool,
        provider: &OpenAiProvider,
        group: &PromotionGroup,
    ) -> Result<bool, String> {
        let mut combined_text = String::new();
        let mut all_tags: HashMap<String, usize> = HashMap::new();

        for c in &group.candidates {
            combined_text.push_str(&format!(
                "- Conversation {}: {}\n",
                c.conversation_id, c.content
            ));
            for tag in &c.tags {
                *all_tags.entry(tag.clone()).or_insert(0) += 1;
            }
        }

        let prompt_input = format!(
            "{sys}\n\nAcross {conv_count} conversations, found {chunk_count} similar chunks:\n{data}",
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

        let common_tags: Vec<String> = all_tags
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .map(|(tag, _)| tag)
            .collect();

        let chunk_id = Uuid::now_v7().to_string();
        let now = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(&common_tags).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            "INSERT INTO memory_chunks (id, knowledge_type, content, tags, confidence, pending_confirm, placed_by, created_at, updated_at) \
             VALUES (?, 'pattern', ?, ?, 0.70, 1, 'ai', ?, ?)",
        )
        .bind(&chunk_id)
        .bind(&response)
        .bind(&tags_json)
        .bind(&now)
        .bind(&now)
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;

        for c in &group.candidates {
            let _ = sqlx::query("UPDATE memory_chunks SET promoted_capture_id = ? WHERE id = ?")
                .bind(&chunk_id)
                .bind(&c.id)
                .execute(db)
                .await;
        }

        Ok(true)
    }

    async fn expand_log_trigger_contexts(&self, db: &SqlitePool) -> Result<(), String> {
        let logs = sqlx::query(
            "SELECT id, content, trigger_context FROM memory_chunks \
             WHERE knowledge_type = 'log' ORDER BY created_at DESC LIMIT 20",
        )
        .fetch_all(db)
        .await
        .map_err(|e| e.to_string())?;

        for log_row in &logs {
            let log_id: String = log_row.get("id");
            let content: String = log_row.get("content");
            let existing_ctx: String = log_row.try_get("trigger_context").unwrap_or_default();

            let log_vec = match self.embedder.embed(&content).await {
                Ok(v) => v,
                Err(_) => continue,
            };

            let similar_data = sqlx::query(
                "SELECT content FROM memory_chunks \
                 WHERE knowledge_type = 'data' AND id != ? \
                 ORDER BY created_at DESC LIMIT 20",
            )
            .bind(&log_id)
            .fetch_all(db)
            .await
            .map_err(|e| e.to_string())?;

            let mut new_contexts: Vec<String> = Vec::new();
            let existing_set: HashSet<&str> =
                existing_ctx.split('|').filter(|s| !s.is_empty()).collect();

            for data_row in &similar_data {
                let data_content: String = data_row.get("content");
                let data_vec = match self.embedder.embed(&data_content).await {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let sim = cosine_similarity(&log_vec, &data_vec);
                if sim >= 0.60 {
                    let keyword = data_content.chars().take(30).collect::<String>();
                    if !existing_set.contains(keyword.as_str()) && !new_contexts.contains(&keyword)
                    {
                        new_contexts.push(keyword);
                    }
                }

                if new_contexts.len() >= 3 {
                    break;
                }
            }

            if !new_contexts.is_empty() {
                let updated_ctx = if existing_ctx.is_empty() {
                    new_contexts.join("|")
                } else {
                    format!("{}|{}", existing_ctx, new_contexts.join("|"))
                };

                let _ = sqlx::query(
                    "UPDATE memory_chunks SET trigger_context = ?, updated_at = ? WHERE id = ?",
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

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
