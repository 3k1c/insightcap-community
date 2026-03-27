use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

// ─── LRU Search Cache ─────────────────────────────────────────────────────────

struct SearchCache {
    map: HashMap<u64, Vec<(u64, f32)>>,
    order: VecDeque<u64>,
    capacity: usize,
}

impl SearchCache {
    fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn get(&mut self, key: u64) -> Option<Vec<(u64, f32)>> {
        if self.map.contains_key(&key) {
            self.order.retain(|&k| k != key);
            self.order.push_front(key);
            self.map.get(&key).cloned()
        } else {
            None
        }
    }

    fn insert(&mut self, key: u64, value: Vec<(u64, f32)>) {
        if self.map.contains_key(&key) {
            self.order.retain(|&k| k != key);
        } else if self.map.len() >= self.capacity {
            if let Some(old_key) = self.order.pop_back() {
                self.map.remove(&old_key);
            }
        }
        self.order.push_front(key);
        self.map.insert(key, value);
    }

    fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

// ─── VectorStore ──────────────────────────────────────────────────────────────

/// A thread-safe, pure-Rust MVP Vector Database.
/// Uses a simple HashMap for in-memory storage and naive exact K-NN search via Cosine Similarity.
/// Good enough for MVP (< 100k chunks) and compiles flawlessly on all platforms.
#[derive(Clone)]
pub struct VectorStore {
    state: Arc<RwLock<VectorState>>,
    search_cache: Arc<Mutex<SearchCache>>,
    path: PathBuf,
}

#[derive(Serialize, Deserialize, Default)]
struct VectorState {
    dimensions: usize,
    vectors: HashMap<u64, Vec<f32>>,
}

impl VectorStore {
    /// Loads an existing index from disk or creates a new one.
    pub fn load_or_create(db_dir: &Path, dimensions: usize) -> Result<Self, String> {
        let path = db_dir.join(".insightcap").join("vectors").join("default.bin");

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut state = VectorState {
            dimensions,
            vectors: HashMap::new(),
        };

        if path.exists() {
            match File::open(&path) {
                Ok(mut file) => {
                    let mut buffer = Vec::new();
                    if file.read_to_end(&mut buffer).is_ok() {
                        if let Ok(loaded_state) = bincode::deserialize::<VectorState>(&buffer) {
                            if loaded_state.dimensions != dimensions {
                                eprintln!("Vector store dimension mismatch! Expected {}, found {}. Resetting vectors...", dimensions, loaded_state.dimensions);
                                state.dimensions = dimensions;
                                state.vectors = HashMap::new();
                            } else {
                                state = loaded_state;
                                println!(
                                    "Loaded existing vector store from {:?} with {} vectors.",
                                    path,
                                    state.vectors.len()
                                );
                            }
                        } else {
                            eprintln!("Failed to deserialize vector store. Starting fresh.");
                        }
                    }
                }
                Err(e) => eprintln!("Failed to open vector store file: {}", e),
            }
        } else {
            println!("Created new vector store at {:?}", path);
        }

        Ok(Self {
            state: Arc::new(RwLock::new(state)),
            search_cache: Arc::new(Mutex::new(SearchCache::new(50))),
            path,
        })
    }

    /// Loads an existing index from the exact file path, or creates a new one there.
    pub fn load_or_create_at_path(path: &Path, dimensions: usize) -> Self {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut state = VectorState {
            dimensions,
            vectors: HashMap::new(),
        };

        if path.exists() {
            match File::open(path) {
                Ok(mut file) => {
                    let mut buffer = Vec::new();
                    if file.read_to_end(&mut buffer).is_ok() {
                        if let Ok(loaded_state) = bincode::deserialize::<VectorState>(&buffer) {
                            if loaded_state.dimensions != dimensions {
                                eprintln!("Vector store dimension mismatch at {:?}! Expected {}, found {}. Resetting...", path, dimensions, loaded_state.dimensions);
                            } else {
                                state = loaded_state;
                                println!("Loaded external vector store from {:?} with {} vectors.", path, state.vectors.len());
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Failed to open external vector store: {}", e),
            }
        }

        Self {
            state: Arc::new(RwLock::new(state)),
            search_cache: Arc::new(Mutex::new(SearchCache::new(50))),
            path: path.to_path_buf(),
        }
    }

    /// Adds a single vector to the store with a corresponding integer ID.
    pub async fn add_vector(&self, id: u64, vector: &[f32]) -> Result<(), String> {
        let mut state = self.state.write().await;

        if vector.len() != state.dimensions {
            return Err(format!(
                "Vector dimension mismatch. Expected {}, got {}",
                state.dimensions,
                vector.len()
            ));
        }

        state.vectors.insert(id, vector.to_vec());
        drop(state);
        // 新增向量後舊的搜尋結果已過期
        self.search_cache.lock().unwrap().clear();
        Ok(())
    }

    pub async fn contains_id(&self, id: u64) -> bool {
        let state = self.state.read().await;
        state.vectors.contains_key(&id)
    }

    pub async fn get_vector(&self, id: u64) -> Option<Vec<f32>> {
        let state = self.state.read().await;
        state.vectors.get(&id).cloned()
    }

    /// Searches for `count` nearest neighbors to the `query` vector using Cosine Similarity.
    /// Results are LRU-cached (capacity 50) to avoid redundant full scans for repeated queries.
    /// Returns a list of (id, similarity_score) tuples sorted by highest similarity.
    pub async fn search(&self, query: &[f32], count: usize) -> Result<Vec<(u64, f32)>, String> {
        // ── Cache lookup ──
        let cache_key = Self::compute_cache_key(query, count);
        if let Some(cached) = self.search_cache.lock().unwrap().get(cache_key) {
            return Ok(cached);
        }

        let state = self.state.read().await;

        if query.len() != state.dimensions {
            return Err(format!(
                "Query dimension mismatch. Expected {}, got {}",
                state.dimensions,
                query.len()
            ));
        }

        // Cosine Similarity: dot_product(A, B) / (norm(A) * norm(B))
        let query_norm = Self::norm(query);
        if query_norm == 0.0 {
            return Err("Query vector has zero norm".to_string());
        }

        let mut results: Vec<(u64, f32)> = state
            .vectors
            .iter()
            .map(|(&id, vec)| {
                let dot = Self::dot_product(query, vec);
                let vec_norm = Self::norm(vec);

                let similarity = if vec_norm == 0.0 {
                    0.0
                } else {
                    dot / (query_norm * vec_norm)
                };

                (id, similarity)
            })
            .collect();

        // Sort descending by similarity
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top `count`
        results.truncate(count);

        // ── Store in cache ──
        drop(state);
        self.search_cache.lock().unwrap().insert(cache_key, results.clone());

        Ok(results)
    }

    /// Computes a stable u64 hash for a query vector + count pair.
    fn compute_cache_key(query: &[f32], count: usize) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        count.hash(&mut hasher);
        for &f in query {
            f.to_bits().hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Removes a vector from the store by its ID.
    pub async fn remove_vector(&self, id: u64) -> Result<(), String> {
        let mut state = self.state.write().await;
        state.vectors.remove(&id);
        drop(state);
        self.search_cache.lock().unwrap().clear();
        Ok(())
    }

    /// Clears all vectors from the store and saves the empty state.
    pub async fn clear(&self) -> Result<(), String> {
        let mut state = self.state.write().await;
        state.vectors.clear();
        drop(state);
        self.search_cache.lock().unwrap().clear();
        self.save().await
    }

    /// Saves the vector state to disk using bincode.
    pub async fn save(&self) -> Result<(), String> {
        let state = self.state.read().await;

        let encoded =
            bincode::serialize(&*state).map_err(|e| format!("Serialization failed: {}", e))?;

        let mut file =
            File::create(&self.path).map_err(|e| format!("Failed to create file: {}", e))?;

        file.write_all(&encoded)
            .map_err(|e| format!("Failed to write to file: {}", e))?;

        Ok(())
    }

    fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    fn norm(a: &[f32]) -> f32 {
        a.iter().map(|x| x * x).sum::<f32>().sqrt()
    }
}
