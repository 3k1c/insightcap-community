
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::vector_store::local::VectorStore;

pub struct MultiIndexManager {
    indices: Arc<RwLock<HashMap<String, VectorStore>>>,
    external_dir: PathBuf,
}

impl MultiIndexManager {
    pub fn new(kb_path: &Path) -> Self {
        let external_dir = kb_path.join(".insightcap").join("vectors").join("external");
        let _ = std::fs::create_dir_all(&external_dir);
        Self {
            indices: Arc::new(RwLock::new(HashMap::new())),
            external_dir,
        }
    }

    pub async fn load_or_create_external(&self, ekb_id: &str, dimensions: usize) -> VectorStore {
        let cache_path = self.external_dir.join(format!("{}.bin", ekb_id));
        VectorStore::load_or_create_at_path(&cache_path, dimensions)
    }

    pub async fn get(&self, ekb_id: &str) -> Option<VectorStore> {
        let indices = self.indices.read().await;
        indices.get(ekb_id).cloned()
    }

    pub async fn insert(&self, ekb_id: String, store: VectorStore) {
        let mut indices = self.indices.write().await;
        indices.insert(ekb_id, store);
    }

    pub async fn remove(&self, ekb_id: &str) {
        let mut indices = self.indices.write().await;
        indices.remove(ekb_id);
        let cache_path = self.external_dir.join(format!("{}.bin", ekb_id));
        let _ = std::fs::remove_file(&cache_path);
    }

    pub async fn search_all(&self, query: &[f32], top_k: usize) -> Vec<(String, u64, f32)> {
        let indices = self.indices.read().await;
        let mut all: Vec<(String, u64, f32)> = Vec::new();

        for (ekb_id, store) in indices.iter() {
            if let Ok(results) = store.search(query, top_k).await {
                for (vid, score) in results {
                    all.push((ekb_id.clone(), vid, score));
                }
            }
        }

        all.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        all.truncate(top_k);
        all
    }

    pub fn external_dir(&self) -> &Path {
        &self.external_dir
    }
}
