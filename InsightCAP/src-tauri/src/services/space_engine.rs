use crate::providers::embedding::Embedder;
use crate::vector_store::local::VectorStore;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::sync::Arc;

const BATCH_SIZE: usize = 50;
const ASSIGN_THRESHOLD: f32 = 0.81;
const MERGE_THRESHOLD: f32 = 0.91;

pub struct SpaceEngine {
    pool: SqlitePool,
    embedder: Arc<dyn Embedder>,
    vector_store: VectorStore,
}

impl SpaceEngine {
    pub fn new(pool: SqlitePool, embedder: Arc<dyn Embedder>, vector_store: VectorStore) -> Self {
        Self {
            pool,
            embedder,
            vector_store,
        }
    }

    pub async fn assign_to_space(
        &self,
        capture_id: &str,
        content: &str,
    ) -> Result<Option<(String, bool)>, String> {
        let settings = crate::settings::store::get_settings(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let output_language = output_language_label(&settings.general.language);
        let cfg = settings.ai_models.content_processor_llm;

        let mut opt_provider: Option<crate::providers::llm::openai::OpenAiProvider> = None;
        let is_ollama = cfg.provider == "ollama";
        let api_key = cfg.api_key.unwrap_or_default();

        if !api_key.is_empty() || is_ollama {
            opt_provider = Some(crate::providers::llm::openai::OpenAiProvider::new(
                api_key,
                cfg.base_url,
                cfg.model,
                cfg.provider.clone(),
            ));
        }

        if let Some(llm) = opt_provider {
            use crate::providers::llm::LLMProvider;

            let existing_names: Vec<String> = sqlx::query_scalar(
                "SELECT name FROM spaces WHERE is_archived = 0 ORDER BY chunk_count DESC LIMIT 30",
            )
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

            let byte_limit = content.len().min(1500);
            let safe_limit = content.floor_char_boundary(byte_limit);
            let sample = &content[..safe_limit];

            let prompt = if existing_names.is_empty() {
                format!(
                    "Create a highly specific, short category or Space name for the following text. \
                     Return ONLY the category name in {output_language}. No punctuation.\n\nText:\n{}",
                    sample
                )
            } else {
                format!(
                    "Existing Space names:\n{}\n\n\
                     Categorize the following text. If it VERY STRICTLY belongs to one of the existing Spaces, return that Space name. \
                     Otherwise, create a NEW, highly specific short Space name in {output_language}. \
                     Do NOT default to an existing space if the topic is even slightly different. \
                     Return ONLY the Space name. No punctuation.\n\nText:\n{}",
                    existing_names.join(", "),
                    sample
                )
            };

            if let Ok(category) = llm
                .complete(&prompt, crate::providers::llm::LLMOptions::default())
                .await
            {
                let clean_category = category.trim().to_string();
                if !clean_category.is_empty() {
                    let space_id: Option<String> =
                        sqlx::query_scalar("SELECT id FROM spaces WHERE name = ?")
                            .bind(&clean_category)
                            .fetch_optional(&self.pool)
                            .await
                            .unwrap_or(None);

                    let (active_space_id, is_new) = if let Some(id) = space_id {
                        (id, false)
                    } else {
                        let new_id = uuid::Uuid::now_v7().to_string();
                        let now = chrono::Utc::now().to_rfc3339();
                        sqlx::query("INSERT INTO spaces (id, name, chunk_count, created_at, updated_at) VALUES (?, ?, 0, ?, ?)")
                            .bind(&new_id)
                            .bind(&clean_category)
                            .bind(&now)
                            .bind(&now)
                            .execute(&self.pool)
                            .await
                            .map_err(|e| e.to_string())?;
                        (new_id, true)
                    };

                    sqlx::query("UPDATE captures SET space_id = ? WHERE id = ?")
                        .bind(&active_space_id)
                        .bind(capture_id)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| e.to_string())?;

                    sqlx::query("UPDATE spaces SET chunk_count = chunk_count + 1 WHERE id = ?")
                        .bind(&active_space_id)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| e.to_string())?;

                    return Ok(Some((active_space_id, is_new)));
                }
            }
        }

        println!(
            "[SpaceEngine]     capture {}    Space      inbox   ",
            capture_id
        );
        Ok(None)
    }

    pub async fn recluster_all(&self) -> Result<usize, String> {
        let space_rows = sqlx::query("SELECT id, name FROM spaces WHERE is_archived = 0")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        if space_rows.is_empty() {
            return Ok(0);
        }

        let mut space_centers = self.compute_space_centers(&space_rows).await?;
        if space_centers.is_empty() {
            return Ok(0);
        }

        for (space_id, center) in &space_centers {
            let blob = vec_to_blob(center);
            let _ = sqlx::query("UPDATE spaces SET embedding_center = ? WHERE id = ?")
                .bind(&blob)
                .bind(space_id)
                .execute(&self.pool)
                .await;
        }

        let merged = self.merge_similar_spaces(&space_centers).await?;
        if !merged.is_empty() {
            println!("[SpaceEngine]     {}   Space", merged.len());
            for (absorbed_id, _) in &merged {
                space_centers.remove(absorbed_id);
            }
        }

        let cap_count = self.recluster_captures(&space_centers).await?;
        let mc_count = self.recluster_memory_chunks(&space_centers).await?;

        self.refresh_chunk_counts().await?;

        let total = cap_count + mc_count;
        println!(
            "[SpaceEngine]       captures={} memory_chunks={}",
            cap_count, mc_count
        );
        Ok(total)
    }

    async fn merge_similar_spaces(
        &self,
        space_centers: &HashMap<String, Vec<f32>>,
    ) -> Result<Vec<(String, String)>, String> {
        let count_rows = sqlx::query("SELECT id, chunk_count, is_user_managed FROM spaces WHERE is_archived = 0")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let space_info: HashMap<String, (i64, bool)> = count_rows
            .iter()
            .map(|r| {
                (
                    r.get::<String, _>("id"),
                    (
                        r.try_get::<i64, _>("chunk_count").unwrap_or(0),
                        r.try_get::<i32, _>("is_user_managed").map(|v| v == 1).unwrap_or(false)
                    )
                )
            })
            .collect();

        let mut space_ids: Vec<&String> = space_centers.keys().collect();
        space_ids.sort();

        let mut merge_pairs: Vec<(String, String, f32)> = Vec::new();
        for i in 0..space_ids.len() {
            for j in (i + 1)..space_ids.len() {
                let sim =
                    cosine_similarity(&space_centers[space_ids[i]], &space_centers[space_ids[j]]);
                if sim >= MERGE_THRESHOLD {
                    merge_pairs.push((space_ids[i].clone(), space_ids[j].clone(), sim));
                }
            }
        }

        merge_pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        let mut absorbed: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut result: Vec<(String, String)> = Vec::new();

        for (id_a, id_b, sim) in &merge_pairs {
            if absorbed.contains(id_a) || absorbed.contains(id_b) {
                continue;
            }

            let (count_a, is_user_a) = space_info.get(id_a).copied().unwrap_or((0, false));
            let (count_b, is_user_b) = space_info.get(id_b).copied().unwrap_or((0, false));
            
            let (survivor, absorbed_id) = if is_user_a && !is_user_b {
                (id_a.clone(), id_b.clone())
            } else if !is_user_a && is_user_b {
                (id_b.clone(), id_a.clone())
            } else if count_a >= count_b {
                (id_a.clone(), id_b.clone())
            } else {
                (id_b.clone(), id_a.clone())
            };

            println!(
                "[SpaceEngine]    Space: {} -> {} (similarity={:.3})",
                absorbed_id, survivor, sim
            );

            self.execute_merge(&survivor, &absorbed_id).await?;
            absorbed.insert(absorbed_id.clone());
            result.push((absorbed_id, survivor));
        }

        Ok(result)
    }

    async fn execute_merge(&self, survivor_id: &str, absorbed_id: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query("UPDATE captures SET space_id = ? WHERE space_id = ?")
            .bind(survivor_id)
            .bind(absorbed_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query("UPDATE memory_chunks SET space_id = ? WHERE space_id = ?")
            .bind(survivor_id)
            .bind(absorbed_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let absorbed_guide: String =
            sqlx::query_scalar("SELECT knowledge_guide_content FROM spaces WHERE id = ?")
                .bind(absorbed_id)
                .fetch_optional(&self.pool)
                .await
                .unwrap_or(None)
                .unwrap_or_default();

        if !absorbed_guide.trim().is_empty() {
            let survivor_guide: String =
                sqlx::query_scalar("SELECT knowledge_guide_content FROM spaces WHERE id = ?")
                    .bind(survivor_id)
                    .fetch_optional(&self.pool)
                    .await
                    .unwrap_or(None)
                    .unwrap_or_default();

            let merged_guide = if survivor_guide.trim().is_empty() {
                absorbed_guide
            } else {
                format!("{}\n\n---\n{}", survivor_guide, absorbed_guide)
            };

            sqlx::query("UPDATE spaces SET knowledge_guide_content = ?, knowledge_guide_updated_at = ? WHERE id = ?")
                .bind(&merged_guide)
                .bind(&now)
                .bind(survivor_id)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        }

        sqlx::query(
            "UPDATE spaces SET is_archived = 1, chunk_count = 0, updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(absorbed_id)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn assign_memory_chunk_to_space(
        &self,
        chunk_id: &str,
        content: &str,
    ) -> Result<Option<String>, String> {
        let vec = self
            .embedder
            .embed(content)
            .await
            .map_err(|e| e.to_string())?;

        let space_rows = sqlx::query(
            "SELECT id, embedding_center FROM spaces WHERE is_archived = 0 AND embedding_center IS NOT NULL"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut centers: HashMap<String, Vec<f32>> = HashMap::new();
        for r in &space_rows {
            let sid: String = r.get("id");
            let blob: Vec<u8> = r.try_get("embedding_center").unwrap_or_default();
            if !blob.is_empty() {
                centers.insert(sid, blob_to_vec(&blob));
            }
        }

        if centers.is_empty() {
            return Ok(None);
        }

        if let Some((best_space_id, best_sim)) = best_matching_space(&centers, &vec) {
            if best_sim >= ASSIGN_THRESHOLD {
                sqlx::query("UPDATE memory_chunks SET space_id = ? WHERE id = ?")
                    .bind(&best_space_id)
                    .bind(chunk_id)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(Some(best_space_id));
            }
        }

        Ok(None)
    }

    async fn compute_space_centers(
        &self,
        space_rows: &[sqlx::sqlite::SqliteRow],
    ) -> Result<HashMap<String, Vec<f32>>, String> {
        let mut centers: HashMap<String, Vec<f32>> = HashMap::new();

        for space_row in space_rows {
            let space_id: String = space_row.get("id");
            let mut vecs: Vec<Vec<f32>> = Vec::new();

            let cap_rows = sqlx::query(
                "SELECT vector_id, clean_content FROM captures WHERE space_id = ? AND vector_id IS NOT NULL LIMIT 100"
            )
            .bind(&space_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

            for r in &cap_rows {
                let vid: i64 = r.try_get("vector_id").unwrap_or(0);
                if vid > 0 {
                    if let Some(v) = self.vector_store.get_vector(vid as u64).await {
                        vecs.push(v);
                    }
                }
            }

            let mc_rows = sqlx::query(
                "SELECT vector_id FROM memory_chunks WHERE space_id = ? AND vector_id IS NOT NULL LIMIT 100"
            )
            .bind(&space_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

            for r in &mc_rows {
                let vid: i64 = r.try_get("vector_id").unwrap_or(0);
                if vid > 0 {
                    if let Some(v) = self.vector_store.get_vector(vid as u64).await {
                        vecs.push(v);
                    }
                }
            }

            if vecs.is_empty() {
                let space_name: String = space_row.get("name");
                if let Ok(v) = self.embedder.embed(&space_name).await {
                    if v.iter().any(|&x| x != 0.0) {
                        vecs.push(v);
                    }
                }
            }

            if !vecs.is_empty() {
                let center = average_vectors(&vecs);
                centers.insert(space_id, center);
            }
        }

        Ok(centers)
    }

    async fn recluster_captures(
        &self,
        space_centers: &HashMap<String, Vec<f32>>,
    ) -> Result<usize, String> {
        let mut offset: u64 = 0;
        let mut updated_count = 0;

        loop {
            let rows = sqlx::query(
                "SELECT id, vector_id, clean_content FROM captures \
                 WHERE status = 'processed' \
                 ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(BATCH_SIZE as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

            if rows.is_empty() {
                break;
            }
            let batch_len = rows.len();

            for r in &rows {
                let cap_id: String = r.get("id");
                let vid: Option<i64> = r.try_get("vector_id").ok().flatten();
                let content: String = r.try_get("clean_content").unwrap_or_default();

                let vec = if let Some(v_id) = vid {
                    self.vector_store.get_vector(v_id as u64).await
                } else {
                    None
                };

                let vec = match vec {
                    Some(v) => v,
                    None => match self.embedder.embed(&content).await {
                        Ok(v) => v,
                        Err(_) => continue,
                    },
                };

                if let Some((best_space_id, best_sim)) = best_matching_space(space_centers, &vec) {
                    if best_sim >= ASSIGN_THRESHOLD {
                        let _ = sqlx::query("UPDATE captures SET space_id = ? WHERE id = ?")
                            .bind(&best_space_id)
                            .bind(&cap_id)
                            .execute(&self.pool)
                            .await;
                        updated_count += 1;
                    }
                }
            }

            offset += batch_len as u64;
            if batch_len < BATCH_SIZE {
                break;
            }
        }

        Ok(updated_count)
    }

    async fn recluster_memory_chunks(
        &self,
        space_centers: &HashMap<String, Vec<f32>>,
    ) -> Result<usize, String> {
        let mut offset: u64 = 0;
        let mut updated_count = 0;

        loop {
            let rows = sqlx::query(
                "SELECT id, vector_id, content FROM memory_chunks \
                 WHERE pending_confirm = 0 \
                 ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(BATCH_SIZE as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

            if rows.is_empty() {
                break;
            }
            let batch_len = rows.len();

            for r in &rows {
                let mc_id: String = r.get("id");
                let vid: Option<i64> = r.try_get("vector_id").ok().flatten();
                let content: String = r.try_get("content").unwrap_or_default();

                let vec = if let Some(v_id) = vid {
                    self.vector_store.get_vector(v_id as u64).await
                } else {
                    None
                };

                let vec = match vec {
                    Some(v) => v,
                    None => match self.embedder.embed(&content).await {
                        Ok(v) => v,
                        Err(_) => continue,
                    },
                };

                if let Some((best_space_id, best_sim)) = best_matching_space(space_centers, &vec) {
                    if best_sim >= ASSIGN_THRESHOLD {
                        let _ = sqlx::query("UPDATE memory_chunks SET space_id = ? WHERE id = ?")
                            .bind(&best_space_id)
                            .bind(&mc_id)
                            .execute(&self.pool)
                            .await;
                        updated_count += 1;
                    }
                }
            }

            offset += batch_len as u64;
            if batch_len < BATCH_SIZE {
                break;
            }
        }

        Ok(updated_count)
    }

    async fn refresh_chunk_counts(&self) -> Result<(), String> {
        let space_ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM spaces WHERE is_archived = 0")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| e.to_string())?;

        for space_id in &space_ids {
            let cap_cnt: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM captures WHERE space_id = ?")
                    .bind(space_id)
                    .fetch_one(&self.pool)
                    .await
                    .unwrap_or(0);

            let mc_cnt: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM memory_chunks WHERE space_id = ?")
                    .bind(space_id)
                    .fetch_one(&self.pool)
                    .await
                    .unwrap_or(0);

            let _ = sqlx::query("UPDATE spaces SET chunk_count = ?, updated_at = ? WHERE id = ?")
                .bind(cap_cnt + mc_cnt)
                .bind(chrono::Utc::now().to_rfc3339())
                .bind(space_id)
                .execute(&self.pool)
                .await;
        }

        let _ = sqlx::query(
            "UPDATE spaces SET is_archived = 1, updated_at = ? WHERE is_archived = 0 AND chunk_count = 0 AND is_user_managed = 0"
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await;

        Ok(())
    }
}

fn output_language_label(language: &str) -> &'static str {
    match language {
        "zh-CN" => "Simplified Chinese",
        "en" => "English",
        _ => "Traditional Chinese",
    }
}


fn best_matching_space(
    centers: &HashMap<String, Vec<f32>>,
    query: &[f32],
) -> Option<(String, f32)> {
    let mut best: Option<(String, f32)> = None;
    for (space_id, center) in centers {
        let sim = cosine_similarity(query, center);
        if best.as_ref().map_or(true, |(_, s)| sim > *s) {
            best = Some((space_id.clone(), sim));
        }
    }
    best
}

fn average_vectors(vecs: &[Vec<f32>]) -> Vec<f32> {
    if vecs.is_empty() {
        return vec![];
    }
    let dim = vecs[0].len();
    let mut sum = vec![0.0f32; dim];
    for v in vecs {
        for (i, x) in v.iter().enumerate() {
            if i < dim {
                sum[i] += x;
            }
        }
    }
    let n = vecs.len() as f32;
    sum.iter_mut().for_each(|x| *x /= n);
    sum
}

fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn blob_to_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
