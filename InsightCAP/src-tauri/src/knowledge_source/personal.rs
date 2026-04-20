use crate::knowledge_source::{
    KnowledgeError, KnowledgeSource, KnowledgeSourceType, QueryScope, ScoredChunk,
};
use sqlx::{Row, SqlitePool};

pub struct PersonalKnowledgeSource {
    pool: SqlitePool,
}

impl PersonalKnowledgeSource {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl KnowledgeSource for PersonalKnowledgeSource {
    fn source_type(&self) -> KnowledgeSourceType {
        KnowledgeSourceType::Personal
    }

    async fn semantic_search(
        &self,
        _query_embedding: &[f32],
        _scope: &QueryScope,
        limit: usize,
    ) -> Result<Vec<ScoredChunk>, KnowledgeError> {
        let rows = sqlx::query(
            "SELECT id, clean_content, tags FROM captures ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KnowledgeError::Database(e.to_string()))?;

        let mut chunks = Vec::new();
        for r in rows {
            let id: String = r.get("id");
            let content: String = r.get("clean_content");
            chunks.push(ScoredChunk {
                id,
                content,
                score: 0.8,
                knowledge_type: "data".to_string(),
                source_title: None,
                tags: vec![],
            });
        }
        Ok(chunks)
    }

    async fn keyword_trigger(
        &self,
        query: &str,
        _scope: &QueryScope,
    ) -> Result<Vec<ScoredChunk>, KnowledgeError> {
        let query_like = format!("%{}%", query);
        let rows = sqlx::query(
            "SELECT id, clean_content, tags FROM captures WHERE clean_content LIKE ? ORDER BY created_at DESC LIMIT 5"
        )
        .bind(&query_like)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KnowledgeError::Database(e.to_string()))?;

        let mut chunks = Vec::new();
        for r in rows {
            let id: String = r.get("id");
            let content: String = r.get("clean_content");
            chunks.push(ScoredChunk {
                id,
                content,
                score: 0.9,
                knowledge_type: "data".to_string(),
                source_title: None,
                tags: vec![],
            });
        }
        Ok(chunks)
    }
}
