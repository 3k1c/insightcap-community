/// KnowledgeSource trait — 抽象知識源接口
/// 按 Architecture-v2.md「抽象層」章節定義
/// Phase 1 只定義 trait，具體實現在 Phase 2 (personal.rs) 和 Phase 5 (enterprise.rs)
pub mod enterprise;
pub mod personal;

#[derive(Debug, Clone)]
pub struct QueryScope {
    pub tags: Vec<String>,
    pub project_id: Option<String>,
    pub include_external: bool,
}

#[derive(Debug, Clone)]
pub struct ScoredChunk {
    pub id: String,
    pub content: String,
    pub score: f32,
    pub knowledge_type: String, // data | pattern | log
    pub source_title: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum KnowledgeSourceType {
    Personal,
    EnterpriseLocal,
    EnterpriseExternal(String),
}

#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Vector search error: {0}")]
    VectorSearch(String),
    #[error("Not initialized")]
    NotInitialized,
}

/// KnowledgeSource trait 定義
/// 個人版和商業版通過此 trait 分離
pub trait KnowledgeSource: Send + Sync {
    fn source_type(&self) -> KnowledgeSourceType;

    fn semantic_search(
        &self,
        query_embedding: &[f32],
        scope: &QueryScope,
        limit: usize,
    ) -> impl std::future::Future<Output = Result<Vec<ScoredChunk>, KnowledgeError>> + Send;

    fn keyword_trigger(
        &self,
        query: &str,
        scope: &QueryScope,
    ) -> impl std::future::Future<Output = Result<Vec<ScoredChunk>, KnowledgeError>> + Send;
}
