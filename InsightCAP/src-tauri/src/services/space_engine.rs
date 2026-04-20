use crate::providers::embedding::Embedder;
use crate::vector_store::local::VectorStore;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::sync::Arc;

/// 重聚類一批的大小
const BATCH_SIZE: usize = 50;
/// 分配給 Space 所需的最低向量相似度
const ASSIGN_THRESHOLD: f32 = 0.45;
/// Space 合併所需的中心向量相似度閾值
const MERGE_THRESHOLD: f32 = 0.82;

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

    /// 將給定的 Capture 分配到現有的 Space，或者建立新的 Space。
    /// 回傳 (space_id, is_new_space)；is_new_space=true 表示新建了 Space。
    pub async fn assign_to_space(
        &self,
        capture_id: &str,
        content: &str,
    ) -> Result<Option<(String, bool)>, String> {
        let settings = crate::settings::store::get_settings(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
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

            // 查出現有 Space 名稱清單，讓 LLM 優先重用
            let existing_names: Vec<String> = sqlx::query_scalar(
                "SELECT name FROM spaces WHERE is_archived = 0 ORDER BY chunk_count DESC LIMIT 30",
            )
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

            let prompt = if existing_names.is_empty() {
                format!(
                    "Categorize the following text into one short category/space name (e.g. 程式開發, 數位行銷, 學習筆記). \
                     Return ONLY the category name in Traditional Chinese. NO punctuation.\n\nText:\n{}",
                    &content[..content.len().min(1500)]
                )
            } else {
                format!(
                    "你是知識庫分類助手。\n\
                     現有 Space 清單：{}\n\n\
                     請將以下文字分配到最合適的 Space。\
                     規則：1) 如果現有清單中有語意相符的，直接回傳那個名稱（完全一致）；\
                     2) 只有在清單中完全沒有合適選項時，才回傳一個新的繁體中文短名稱（4字以內）。\
                     只回傳名稱，不含標點或其他文字。\n\nText:\n{}",
                    existing_names.join("、"),
                    &content[..content.len().min(1500)]
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
            "[SpaceEngine] 無法為 capture {} 分配 Space，保留預設 inbox 狀態",
            capture_id
        );
        Ok(None)
    }

    /// 全量重聚類：
    /// 1. 計算各 Space 的 embedding_center（已分配 chunks 向量平均值）
    /// 2. 對所有 captures + memory_chunks 重新計算與各 Space center 的相似度
    /// 3. 批次更新 space_id，閾值 >= 0.45
    /// 4. 更新 spaces.chunk_count
    pub async fn recluster_all(&self) -> Result<usize, String> {
        // Step 1: 取得所有非歸檔 Space
        let space_rows = sqlx::query("SELECT id, name FROM spaces WHERE is_archived = 0")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        if space_rows.is_empty() {
            return Ok(0);
        }

        // Step 2: 計算各 Space 的 embedding_center
        let mut space_centers = self.compute_space_centers(&space_rows).await?;
        if space_centers.is_empty() {
            return Ok(0);
        }

        // Step 3: 將 center 存回 spaces.embedding_center（BLOB）
        for (space_id, center) in &space_centers {
            let blob = vec_to_blob(center);
            let _ = sqlx::query("UPDATE spaces SET embedding_center = ? WHERE id = ?")
                .bind(&blob)
                .bind(space_id)
                .execute(&self.pool)
                .await;
        }

        // Step 3.5: 合併相似 Space
        let merged = self.merge_similar_spaces(&space_centers).await?;
        if !merged.is_empty() {
            println!("[SpaceEngine] 合併了 {} 對 Space", merged.len());
            for (absorbed_id, _) in &merged {
                space_centers.remove(absorbed_id);
            }
        }

        // Step 4: 批次處理 captures
        let cap_count = self.recluster_captures(&space_centers).await?;
        // Step 5: 批次處理 memory_chunks
        let mc_count = self.recluster_memory_chunks(&space_centers).await?;

        // Step 6: 重新計算 spaces.chunk_count
        self.refresh_chunk_counts().await?;

        let total = cap_count + mc_count;
        println!(
            "[SpaceEngine] 重聚類完成：captures={} memory_chunks={}",
            cap_count, mc_count
        );
        Ok(total)
    }

    /// 合併中心向量相似度 >= MERGE_THRESHOLD 的 Space 對。
    /// 較小的 Space（by chunk_count）被吸收進較大的，回傳 (absorbed_id, survivor_id) 清單。
    async fn merge_similar_spaces(
        &self,
        space_centers: &HashMap<String, Vec<f32>>,
    ) -> Result<Vec<(String, String)>, String> {
        // 1. 讀取 chunk_count
        let count_rows = sqlx::query("SELECT id, chunk_count FROM spaces WHERE is_archived = 0")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let chunk_counts: HashMap<String, i64> = count_rows
            .iter()
            .map(|r| {
                (
                    r.get::<String, _>("id"),
                    r.try_get::<i64, _>("chunk_count").unwrap_or(0),
                )
            })
            .collect();

        // 2. 排序 space_id 確保配對順序確定
        let mut space_ids: Vec<&String> = space_centers.keys().collect();
        space_ids.sort();

        // 3. 找出相似度 >= MERGE_THRESHOLD 的配對
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

        // 4. 按相似度降序（最相似的先合併）
        merge_pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        // 5. 逐對合併，已被吸收的不再參與
        let mut absorbed: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut result: Vec<(String, String)> = Vec::new();

        for (id_a, id_b, sim) in &merge_pairs {
            if absorbed.contains(id_a) || absorbed.contains(id_b) {
                continue;
            }

            let count_a = chunk_counts.get(id_a).copied().unwrap_or(0);
            let count_b = chunk_counts.get(id_b).copied().unwrap_or(0);
            let (survivor, absorbed_id) = if count_a >= count_b {
                (id_a.clone(), id_b.clone())
            } else {
                (id_b.clone(), id_a.clone())
            };

            println!(
                "[SpaceEngine] 合併 Space: {} -> {} (similarity={:.3})",
                absorbed_id, survivor, sim
            );

            self.execute_merge(&survivor, &absorbed_id).await?;
            absorbed.insert(absorbed_id.clone());
            result.push((absorbed_id, survivor));
        }

        Ok(result)
    }

    /// 執行合併：將 absorbed 的所有 chunks 移給 survivor，合併 wiki，歸檔 absorbed
    async fn execute_merge(&self, survivor_id: &str, absorbed_id: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();

        // 1. 移轉 captures
        sqlx::query("UPDATE captures SET space_id = ? WHERE space_id = ?")
            .bind(survivor_id)
            .bind(absorbed_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        // 2. 移轉 memory_chunks
        sqlx::query("UPDATE memory_chunks SET space_id = ? WHERE space_id = ?")
            .bind(survivor_id)
            .bind(absorbed_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        // 3. 合併 wiki_content
        let absorbed_wiki: String =
            sqlx::query_scalar("SELECT wiki_content FROM spaces WHERE id = ?")
                .bind(absorbed_id)
                .fetch_optional(&self.pool)
                .await
                .unwrap_or(None)
                .unwrap_or_default();

        if !absorbed_wiki.trim().is_empty() {
            let survivor_wiki: String =
                sqlx::query_scalar("SELECT wiki_content FROM spaces WHERE id = ?")
                    .bind(survivor_id)
                    .fetch_optional(&self.pool)
                    .await
                    .unwrap_or(None)
                    .unwrap_or_default();

            let merged_wiki = if survivor_wiki.trim().is_empty() {
                absorbed_wiki
            } else {
                format!("{}\n\n---\n{}", survivor_wiki, absorbed_wiki)
            };

            sqlx::query("UPDATE spaces SET wiki_content = ?, wiki_updated_at = ? WHERE id = ?")
                .bind(&merged_wiki)
                .bind(&now)
                .bind(survivor_id)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        }

        // 4. 歸檔被吸收的 Space
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

    /// 用向量相似度為 memory_chunk 分配 Space（不呼叫 LLM）
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

        // 讀取所有非歸檔 space 的 embedding_center
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

    /// 計算各 Space 的向量中心（已分配 chunks 的向量平均）
    async fn compute_space_centers(
        &self,
        space_rows: &[sqlx::sqlite::SqliteRow],
    ) -> Result<HashMap<String, Vec<f32>>, String> {
        let mut centers: HashMap<String, Vec<f32>> = HashMap::new();

        for space_row in space_rows {
            let space_id: String = space_row.get("id");
            let mut vecs: Vec<Vec<f32>> = Vec::new();

            // 取得此 Space 的 captures 向量
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

            // 取得此 Space 的 memory_chunks 向量
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

            // 沒有已分配向量 → 用 space name 計算 embedding 作為 center
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

    /// 重聚類 captures：批次讀取 → 計算最近 Space → 更新 space_id
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

                // 取得向量：優先從 VectorStore，沒有則重新 embed
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

    /// 重聚類 memory_chunks：批次讀取 → 計算最近 Space → 更新 space_id
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

    /// 重新計算各 Space 的 chunk_count
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

        // 自動歸檔 chunk_count = 0 的 Space
        let _ = sqlx::query(
            "UPDATE spaces SET is_archived = 1, updated_at = ? WHERE is_archived = 0 AND chunk_count = 0"
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await;

        Ok(())
    }
}

// ─── 輔助函式 ─────────────────────────────────────────────────────────────────

/// 找出與 query 最相似的 Space（回傳 space_id + 相似度）
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

/// 計算多個向量的平均值
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

/// f32 向量 → BLOB（little-endian bytes）
fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// BLOB（little-endian bytes）→ f32 向量
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
