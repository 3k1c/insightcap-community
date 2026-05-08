use async_trait::async_trait;
use fastembed::{InitOptions, TextEmbedding};
use std::path::{Path, PathBuf};
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

        let mut options = InitOptions::new(embedding_model).with_cache_dir(fastembed_cache_dir());
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

fn fastembed_cache_dir_from_env(local: Option<&Path>, home: Option<&Path>) -> PathBuf {
    if let Some(local) = local {
        return local.join("com.insightcap.app").join(".fastembed_cache");
    }
    if let Some(home) = home {
        return home.join(".insightcap").join(".fastembed_cache");
    }
    std::env::temp_dir()
        .join("InsightCAP")
        .join(".fastembed_cache")
}

fn fastembed_cache_dir() -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let dir = fastembed_cache_dir_from_env(local.as_deref(), home.as_deref());
    let _ = std::fs::create_dir_all(&dir);
    dir
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn stores_fastembed_cache_under_tauri_app_data_dir_on_windows() {
        let local = Path::new(r"C:\Users\dev\AppData\Local");

        assert_eq!(
            fastembed_cache_dir_from_env(Some(local), None),
            local.join("com.insightcap.app").join(".fastembed_cache")
        );
    }
}
