use async_trait::async_trait;
use fastembed::{InitOptions, TextEmbedding};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::providers::embedding::Embedder;

pub struct FastEmbedder {
    pub active_model_name: String,
    model: Arc<Mutex<TextEmbedding>>,
}

impl FastEmbedder {
    pub fn new(model_name: &str) -> Result<Self, String> {
        let embedding_model = resolve_embedding_model(model_name);
        println!(
            "[Embedder] Initializing embedding model: {:?}",
            embedding_model
        );

        let mut options = InitOptions::new(embedding_model);
        options.show_download_progress = true;

        let model =
            TextEmbedding::try_new(options).map_err(|e| format!("FastEmbed init failed: {}", e))?;

        let canonical = canonical_embedding_model_name(model_name).to_string();
        println!("[Embedder] Active embedding model: {}", canonical);

        Ok(Self {
            active_model_name: canonical,
            model: Arc::new(Mutex::new(model)),
        })
    }
}

#[async_trait]
impl Embedder for FastEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, crate::providers::embedding::EmbedError> {
        let mut model = self.model.lock().await;
        let mut res = model
            .embed(vec![text], None)
            .map_err(|e| crate::providers::embedding::EmbedError::Failed(e.to_string()))?;
        Ok(res.remove(0))
    }

    async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Vec<f32>>, crate::providers::embedding::EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let mut model = self.model.lock().await;
        model
            .embed(texts.to_vec(), None)
            .map_err(|e| crate::providers::embedding::EmbedError::Failed(e.to_string()))
    }

    fn dimension(&self) -> usize {
        384
    }

    fn model_name(&self) -> &str {
        &self.active_model_name
    }
}

fn resolve_embedding_model(name: &str) -> fastembed::EmbeddingModel {
    match name.trim().to_lowercase().as_str() {
        "bge-small-en-v1.5" | "bge_small_en" => fastembed::EmbeddingModel::BGESmallENV15,
        "all-minilm-l6-v2" | "all_minilm_l6_v2" => fastembed::EmbeddingModel::AllMiniLML6V2,
        _ => fastembed::EmbeddingModel::MultilingualE5Small, // safe multilingual default
    }
}

fn canonical_embedding_model_name(name: &str) -> &str {
    match name.trim().to_lowercase().as_str() {
        "bge-small-en-v1.5" | "bge_small_en" => "bge-small-en-v1.5",
        "all-minilm-l6-v2" | "all_minilm_l6_v2" => "all-minilm-l6-v2",
        _ => "multilingual-e5-small",
    }
}
