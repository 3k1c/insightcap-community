pub mod fastembed;

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    #[error("Model not loaded")]
    ModelNotLoaded,
    #[error("Embedding failed: {0}")]
    Failed(String),
}

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError>;
    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
}

pub struct NoopEmbedder;

#[async_trait]
impl Embedder for NoopEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
        Ok(vec![0.0; 384])
    }
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Ok(texts.iter().map(|_| vec![0.0; 384]).collect())
    }
    fn dimension(&self) -> usize {
        384
    }
    fn model_name(&self) -> &str {
        "noop"
    }
}
