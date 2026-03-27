/// Phase 1 只定義 trait，Phase 2 實現 fastembed.rs
pub mod fastembed;

#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    #[error("Model not loaded")]
    ModelNotLoaded,
    #[error("Embedding failed: {0}")]
    Failed(String),
}

/// Embedder trait 定義
pub trait Embedder: Send + Sync {
    fn embed(
        &self,
        text: &str,
    ) -> impl std::future::Future<Output = Result<Vec<f32>, EmbedError>> + Send;

    fn embed_batch(
        &self,
        texts: &[&str],
    ) -> impl std::future::Future<Output = Result<Vec<Vec<f32>>, EmbedError>> + Send;

    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
}
