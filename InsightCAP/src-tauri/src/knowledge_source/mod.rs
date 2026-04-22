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
