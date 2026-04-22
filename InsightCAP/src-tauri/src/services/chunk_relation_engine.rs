use std::sync::Arc;

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::providers::embedding::Embedder;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::vector_store::local::VectorStore;


const RELATION_CANDIDATES: usize = 10;
const RELATION_THRESHOLD: f32 = 0.50;

pub struct ChunkRelationEngine {
    pool: SqlitePool,
    embedder: Arc<dyn Embedder>,
    vector_store: VectorStore,
}

impl ChunkRelationEngine {
    pub fn new(pool: SqlitePool, embedder: Arc<dyn Embedder>, vector_store: VectorStore) -> Self {
        Self {
            pool,
            embedder,
            vector_store,
        }
    }

    pub async fn analyze_and_link(
        &self,
        chunk_id: &str,
        chunk_type: &str,
        content: &str,
    ) -> Result<usize, String> {
        let vec = self
            .embedder
            .embed(content)
            .await
            .map_err(|e| e.to_string())?;

        let candidates = self.vector_store.search(&vec, RELATION_CANDIDATES).await?;
        let candidates: Vec<(u64, f32)> = candidates
            .into_iter()
            .filter(|(_, s)| *s >= RELATION_THRESHOLD)
            .collect();

        if candidates.is_empty() {
            return Ok(0);
        }

        let mut linked = 0usize;
        let settings = crate::settings::store::get_settings(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let cfg = settings.ai_models.chat_llm;
        let api_key = cfg.api_key.clone().unwrap_or_default();
        let is_ollama = cfg.provider == "ollama";

        if api_key.is_empty() && !is_ollama {
            return Ok(0);
        }

        let llm = OpenAiProvider::new(
            api_key,
            cfg.base_url.clone(),
            cfg.model.clone(),
            cfg.provider.clone(),
        );

        for (vec_id, _score) in &candidates {
            if let Some((cand_id, cand_type, cand_content)) =
                self.find_chunk_by_vector_id(*vec_id).await
            {
                if cand_id == chunk_id {
                    continue;
                }

                let exists: bool = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM chunk_relations WHERE (from_id = ? AND to_id = ?) OR (from_id = ? AND to_id = ?)"
                )
                .bind(chunk_id)
                .bind(&cand_id)
                .bind(&cand_id)
                .bind(chunk_id)
                .fetch_one(&self.pool)
                .await
                .unwrap_or(0i64) > 0;

                if exists {
                    continue;
                }

                if let Some((relation, confidence)) =
                    self.classify_relation(&llm, content, &cand_content).await
                {
                    self.write_relation(
                        chunk_id, chunk_type, &cand_id, &cand_type, &relation, confidence,
                    )
                    .await?;
                    linked += 1;
                }
            }
        }

        Ok(linked)
    }

    async fn find_chunk_by_vector_id(&self, vector_id: u64) -> Option<(String, String, String)> {
        if let Ok(row) = sqlx::query(
            "SELECT id, clean_content FROM captures WHERE vector_id = ? AND status = 'processed'",
        )
        .bind(vector_id as i64)
        .fetch_optional(&self.pool)
        .await
        {
            if let Some(r) = row {
                let id: String = r.try_get("id").ok()?;
                let content: String = r.try_get("clean_content").unwrap_or_default();
                return Some((id, "capture".to_string(), content));
            }
        }

        if let Ok(row) = sqlx::query(
            "SELECT id, content FROM memory_chunks WHERE vector_id = ? AND pending_confirm = 0",
        )
        .bind(vector_id as i64)
        .fetch_optional(&self.pool)
        .await
        {
            if let Some(r) = row {
                let id: String = r.try_get("id").ok()?;
                let content: String = r.try_get("content").unwrap_or_default();
                return Some((id, "memory_chunk".to_string(), content));
            }
        }

        None
    }

    async fn classify_relation(
        &self,
        llm: &OpenAiProvider,
        new_content: &str,
        existing_content: &str,
    ) -> Option<(String, f32)> {
        let prompt = format!(
            "                        \n\
            \n\
                 \n{new}\n\
            \n\
                  \n{existing}\n\
            \n\
                                   \n\
            - references                         \n\
            - extends                         \n\
            - contradicts                         \n\
            \n\
                          :     0.0-1.0 \n\
               references:0.85\n\
                        NONE",
            new = &new_content.chars().take(300).collect::<String>(),
            existing = &existing_content.chars().take(300).collect::<String>(),
        );

        let opts = LLMOptions {
            temperature: 0.1,
            max_tokens: 20,
            stream: false,
            think_mode: None,
        };
        let result = llm.complete(&prompt, opts).await.ok()?;
        let result = result.trim().to_string();

        if result == "NONE" || result.is_empty() {
            return None;
        }

        let parts: Vec<&str> = result.splitn(2, ':').collect();
        if parts.len() != 2 {
            return None;
        }

        let relation = parts[0].trim().to_lowercase();
        if !["references", "extends", "contradicts"].contains(&relation.as_str()) {
            return None;
        }

        let confidence: f32 = parts[1].trim().parse().unwrap_or(0.0);
        if confidence < 0.6 {
            return None;
        }

        Some((relation, confidence))
    }

    async fn write_relation(
        &self,
        from_id: &str,
        from_type: &str,
        to_id: &str,
        to_type: &str,
        relation: &str,
        confidence: f32,
    ) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT OR IGNORE INTO chunk_relations (id, from_id, to_id, from_type, to_type, relation, confidence, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(Uuid::now_v7().to_string())
        .bind(from_id)
        .bind(to_id)
        .bind(from_type)
        .bind(to_type)
        .bind(relation)
        .bind(confidence)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn fetch_linked_chunks(
        &self,
        chunk_ids: &[String],
    ) -> Result<Vec<LinkedChunk>, String> {
        if chunk_ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders = chunk_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let sql = format!(
            "SELECT cr.from_id, cr.to_id, cr.from_type, cr.to_type, cr.relation, cr.confidence \
             FROM chunk_relations cr \
             WHERE cr.from_id IN ({ph}) OR cr.to_id IN ({ph})",
            ph = placeholders
        );

        let mut q = sqlx::query(&sql);
        for id in chunk_ids {
            q = q.bind(id);
        }
        for id in chunk_ids {
            q = q.bind(id);
        } // bind       IN

        let rows = q.fetch_all(&self.pool).await.map_err(|e| e.to_string())?;

        let mut results: Vec<LinkedChunk> = Vec::new();
        for r in rows {
            let from_id: String = r.try_get("from_id").unwrap_or_default();
            let to_id: String = r.try_get("to_id").unwrap_or_default();
            let from_type: String = r.try_get("from_type").unwrap_or_default();
            let to_type: String = r.try_get("to_type").unwrap_or_default();
            let relation: String = r.try_get("relation").unwrap_or_default();
            let confidence: f32 = r.try_get("confidence").unwrap_or(0.0);

            let (linked_id, linked_type) = if chunk_ids.contains(&from_id) {
                (to_id, to_type)
            } else {
                (from_id, from_type)
            };

            if chunk_ids.contains(&linked_id) {
                continue;
            }
            if results
                .iter()
                .any(|c: &LinkedChunk| c.chunk_id == linked_id)
            {
                continue;
            }

            if let Some(content) = self.get_chunk_content(&linked_id, &linked_type).await {
                results.push(LinkedChunk {
                    chunk_id: linked_id,
                    chunk_type: linked_type,
                    content,
                    relation,
                    confidence,
                });
            }
        }

        Ok(results)
    }

    async fn get_chunk_content(&self, chunk_id: &str, chunk_type: &str) -> Option<String> {
        match chunk_type {
            "capture" => sqlx::query_scalar("SELECT clean_content FROM captures WHERE id = ?")
                .bind(chunk_id)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten(),
            "memory_chunk" => sqlx::query_scalar(
                "SELECT content FROM memory_chunks WHERE id = ? AND pending_confirm = 0",
            )
            .bind(chunk_id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten(),
            _ => None,
        }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedChunk {
    pub chunk_id: String,
    pub chunk_type: String,
    pub content: String,
    pub relation: String,
    pub confidence: f32,
}
