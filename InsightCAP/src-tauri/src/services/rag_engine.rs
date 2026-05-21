use std::sync::Arc;

use std::collections::HashSet;

use serde_json::json;
use sqlx::{Row, SqlitePool};

use crate::prompts;
use crate::providers::embedding::Embedder;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::usage_policy::{apply_llm_usage_policy, LLMTaskKind};
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::vector_store::local::VectorStore;

const CAPTURES_LIMIT: usize = 10;
const CAPTURES_THRESHOLD: f32 = 0.25;
const MEMORY_DATA_LIMIT: usize = 5;
const MEMORY_DATA_THRESHOLD: f32 = 0.25;
const MEMORY_PATTERN_LIMIT: usize = 3;
const MEMORY_PATTERN_THRESHOLD: f32 = 0.20;
const MEMORY_EXTERNAL_LIMIT: usize = 5;
#[allow(dead_code)]
const MEMORY_EXTERNAL_THRESHOLD: f32 = 0.25;

const BONUS_SAME_PROJECT: f32 = 0.06;
const BONUS_PATTERN: f32 = 0.05;
const BONUS_LOG: f32 = 0.08;
const BONUS_USE_FREQUENCY: f32 = 0.02;
const BONUS_USER_PLACED: f32 = 0.05;

pub struct RagEngine {
    pool: SqlitePool,
    vector_store: VectorStore,
    embedder: Arc<dyn Embedder>,
}

struct SourceScopeGuidance {
    scope: &'static str,
    phrase: &'static str,
    guidance: String,
}

fn build_source_scope_guidance(
    selected_source_count: usize,
    temp_attachment_count: usize,
    citation_source_count: usize,
    data_count: usize,
    pattern_count: usize,
    log_count: usize,
    compiled_count: usize,
    external_count: usize,
    source_types: &[String],
) -> SourceScopeGuidance {
    let has_memory = pattern_count > 0 || log_count > 0;
    let has_context = temp_attachment_count
        + selected_source_count
        + citation_source_count
        + data_count
        + pattern_count
        + log_count
        + compiled_count
        + external_count
        > 0;

    let source_kind_phrase = source_scope_kind_phrase(source_types);

    let (scope, phrase) = if temp_attachment_count > 0 {
        ("attached_content", "the material the user just attached")
    } else if selected_source_count == 1 {
        (
            "selected_single_source",
            source_kind_phrase.unwrap_or("the selected source"),
        )
    } else if selected_source_count > 1 {
        ("selected_multiple_sources", "the selected sources")
    } else if external_count > 0 && data_count == 0 && !has_memory && compiled_count == 0 {
        ("external_knowledge", "the external knowledge base")
    } else if compiled_count > 0 && data_count == 0 && !has_memory {
        ("compiled_knowledge", "the compiled knowledge")
    } else if has_memory && data_count == 0 {
        ("memory_only", "the user's previous related records")
    } else if citation_source_count == 1 && data_count > 0 && !has_memory && compiled_count == 0 {
        (
            "single_retrieved_source",
            source_kind_phrase.unwrap_or("this source"),
        )
    } else if citation_source_count > 1 || data_count > 1 {
        (
            "multiple_retrieved_sources",
            "the sources found in the user's data",
        )
    } else if has_context {
        ("mixed_user_data", "the user's available data")
    } else {
        ("insufficient_context", "the available material")
    };

    let guidance = if has_context {
        format!(
            "Knowledge scope: {phrase}. State this boundary naturally when it matters. Do not turn local, benchmark, memory, or synthesized material into universal facts. If evidence is not direct, say it is insufficient."
        )
    } else {
        "No directly relevant local material was retrieved. Do not imply the user's data supports the answer.".to_string()
    };

    SourceScopeGuidance {
        scope,
        phrase,
        guidance,
    }
}

fn source_scope_kind_phrase(source_types: &[String]) -> Option<&'static str> {
    let normalized = source_types
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<Vec<_>>();

    if normalized.iter().any(|s| {
        s.contains("youtube")
            || s.contains("bilibili")
            || s.contains("video")
            || s.contains("subtitle")
            || s.contains("transcript")
    }) {
        return Some("the video");
    }

    if normalized
        .iter()
        .any(|s| s.contains("url") || s.contains("web") || s.contains("html"))
    {
        return Some("the web page");
    }

    if normalized.iter().any(|s| {
        s.contains("pdf")
            || s.contains("doc")
            || s.contains("ppt")
            || s.contains("xls")
            || s.contains("file")
            || s.contains("document")
    }) {
        return Some("the document");
    }

    None
}

fn insert_source_type(source_types: &mut HashSet<String>, value: Option<String>) {
    if let Some(value) = value.map(|s| s.trim().to_string()) {
        if !value.is_empty() {
            source_types.insert(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_scope_guidance_identifies_single_selected_source() {
        let guidance = build_source_scope_guidance(1, 0, 1, 3, 0, 0, 0, 0, &[]);

        assert_eq!(guidance.scope, "selected_single_source");
        assert!(guidance.guidance.contains("the selected source"));
        assert!(guidance.guidance.contains("universal facts"));
    }

    #[test]
    fn source_scope_guidance_identifies_multiple_retrieved_sources() {
        let guidance = build_source_scope_guidance(0, 0, 3, 5, 0, 0, 0, 0, &[]);

        assert_eq!(guidance.scope, "multiple_retrieved_sources");
        assert!(guidance
            .guidance
            .contains("the sources found in the user's data"));
    }

    #[test]
    fn source_scope_guidance_identifies_memory_only_context() {
        let guidance = build_source_scope_guidance(0, 0, 0, 0, 2, 1, 0, 0, &[]);

        assert_eq!(guidance.scope, "memory_only");
        assert!(guidance
            .guidance
            .contains("the user's previous related records"));
    }

    #[test]
    fn source_scope_guidance_requires_insufficient_context_when_empty() {
        let guidance = build_source_scope_guidance(0, 0, 0, 0, 0, 0, 0, 0, &[]);

        assert_eq!(guidance.scope, "insufficient_context");
        assert!(guidance
            .guidance
            .contains("No directly relevant local material"));
    }

    #[test]
    fn source_scope_guidance_uses_video_phrase_for_youtube_context() {
        let source_types = vec!["youtube_subtitle".to_string()];
        let guidance = build_source_scope_guidance(0, 0, 1, 1, 0, 0, 0, 0, &source_types);

        assert_eq!(guidance.scope, "single_retrieved_source");
        assert_eq!(guidance.phrase, "the video");
        assert!(guidance.guidance.contains("the video"));
    }
}

impl RagEngine {
    pub fn new(pool: SqlitePool, vector_store: VectorStore, embedder: Arc<dyn Embedder>) -> Self {
        Self {
            pool,
            vector_store,
            embedder,
        }
    }

    pub async fn retrieve_context(
        &self,
        query: &str,
        project_id: Option<&str>,
        source_ids: Option<&[String]>,
        tag_filter: Option<&[String]>,
        only_memory: bool,
    ) -> Result<serde_json::Value, String> {
        let query_vec = self
            .embedder
            .embed(query)
            .await
            .map_err(|e| e.to_string())?;

        let mut data_context: Vec<String> = Vec::new();
        let mut citation_sources: Vec<String> = Vec::new();
        let mut source_types: HashSet<String> = HashSet::new();
        let mut retrieved_capture_ids: Vec<String> = Vec::new();

        if !only_memory {
            let capture_results = self.vector_store.search(&query_vec, CAPTURES_LIMIT).await?;
            let capture_ids: Vec<(u64, f32)> = capture_results
                .into_iter()
                .filter(|(_, score)| *score >= CAPTURES_THRESHOLD)
                .collect();
            for (vec_id, score) in &capture_ids {
                let mut sql = String::from(
                "SELECT c.id, c.clean_content, c.is_user_edited, c.type AS capture_type, c.content_type, s.title AS source_title, s.type AS source_type, s.url AS source_url, s.use_frequency \
                 FROM captures c LEFT JOIN sources s ON c.source_id = s.id \
                 WHERE c.vector_id = ? AND c.status = 'processed'"
            );
                let mut sid_binds: Vec<&str> = Vec::new();
                let mut filter_conds = Vec::new();
                let mut is_filtered = false;

                if let Some(sids) = source_ids {
                    is_filtered = true;
                    if !sids.is_empty() {
                        let ph = sids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                        filter_conds.push(format!("c.source_id IN ({})", ph));
                        sid_binds.extend(sids.iter().map(|s| s.as_str()));
                    }
                }

                if let Some(tags) = tag_filter {
                    is_filtered = true;
                    if !tags.is_empty() {
                        let tag_conds: Vec<String> = tags
                            .iter()
                            .map(|t| format!("c.tags LIKE '%\"{}%\"'", t.replace('\'', "''")))
                            .collect();
                        filter_conds.push(format!("({})", tag_conds.join(" OR ")));
                    }
                }

                let skip = is_filtered && filter_conds.is_empty();

                if !filter_conds.is_empty() {
                    sql.push_str(&format!(" AND ({})", filter_conds.join(" OR ")));
                }

                let rows = if skip {
                    vec![]
                } else {
                    let mut q = sqlx::query(&sql).bind(*vec_id as i64);
                    for sid in &sid_binds {
                        q = q.bind(*sid);
                    }
                    q.fetch_all(&self.pool).await.map_err(|e| e.to_string())?
                };

                for r in rows {
                    let capture_id: String = r.try_get("id").unwrap_or_default();
                    let content: String = r.try_get("clean_content").unwrap_or_default();
                    let source_title = r.try_get::<String, _>("source_title").ok();
                    insert_source_type(&mut source_types, r.try_get("source_type").ok());
                    insert_source_type(&mut source_types, r.try_get("source_url").ok());
                    insert_source_type(&mut source_types, r.try_get("capture_type").ok());
                    insert_source_type(&mut source_types, r.try_get("content_type").ok());
                    let use_freq: i32 = r.try_get("use_frequency").unwrap_or(0);
                    let is_user_edited: i32 = r.try_get("is_user_edited").unwrap_or(0);

                    let adjusted = score
                        + if use_freq > 0 {
                            BONUS_USE_FREQUENCY
                        } else {
                            0.0
                        }
                        + if is_user_edited == 1 {
                            BONUS_USER_PLACED
                        } else {
                            0.0
                        };

                    if !content.is_empty() && adjusted >= CAPTURES_THRESHOLD {
                        if !capture_id.is_empty() {
                            retrieved_capture_ids.push(capture_id);
                        }
                        if let Some(title) = source_title.as_ref().filter(|t| !t.is_empty()) {
                            data_context.push(format!("[  : {}]\n{}", title, content));
                        } else {
                            data_context.push(content);
                        }
                    }
                    if let Some(title) = source_title {
                        if !title.is_empty() && !citation_sources.iter().any(|s| s == &title) {
                            citation_sources.push(title);
                        }
                    }
                }
            }
        }

        let memory_results = self
            .vector_store
            .search(&query_vec, MEMORY_PATTERN_LIMIT + MEMORY_DATA_LIMIT + 5)
            .await?;

        let mut pattern_context: Vec<String> = Vec::new();
        let mut log_context: Vec<String> = Vec::new();
        let mut pattern_hints: Vec<String> = Vec::new();
        let mut log_hints: Vec<String> = Vec::new();
        let mut retrieved_memory_chunk_ids: Vec<String> = Vec::new();

        for (vec_id, score) in &memory_results {
            let mut msql = String::from(
                "SELECT m.id, m.content, m.knowledge_type, m.project_id, m.trigger_context, m.placed_by, s.title AS source_title, s.type AS source_type, s.url AS source_url \
                 FROM memory_chunks m \
                 LEFT JOIN sources s ON m.source_id = s.id \
                 WHERE m.vector_id = ?"
            );
            let mut msid_binds: Vec<&str> = Vec::new();
            let mut mfilter_conds = Vec::new();
            let mut is_filtered = false;

            if let Some(sids) = source_ids {
                is_filtered = true;
                if !sids.is_empty() {
                    let ph = sids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                    mfilter_conds.push(format!("m.source_id IN ({})", ph));
                    msid_binds.extend(sids.iter().map(|s| s.as_str()));
                }
            }

            if let Some(tags) = tag_filter {
                is_filtered = true;
                if !tags.is_empty() {
                    let tag_conds: Vec<String> = tags
                        .iter()
                        .map(|t| format!("m.tags LIKE '%\"{}%\"'", t.replace('\'', "''")))
                        .collect();
                    mfilter_conds.push(format!("({})", tag_conds.join(" OR ")));
                }
            }

            let mskip = is_filtered && mfilter_conds.is_empty();

            if !mfilter_conds.is_empty() {
                msql.push_str(&format!(" AND ({})", mfilter_conds.join(" OR ")));
            }

            let rows = if mskip {
                vec![]
            } else {
                let mut q = sqlx::query(&msql).bind(*vec_id as i64);
                for sid in &msid_binds {
                    q = q.bind(*sid);
                }
                q.fetch_all(&self.pool).await.map_err(|e| e.to_string())?
            };

            for r in rows {
                let mc_id: String = r.try_get("id").unwrap_or_default();
                let knowledge_type: String = r.try_get("knowledge_type").unwrap_or_default();
                let content: String = r.try_get("content").unwrap_or_default();
                let chunk_project: Option<String> = r.try_get("project_id").unwrap_or(None);
                let source_title: Option<String> = r.try_get("source_title").ok();
                insert_source_type(&mut source_types, r.try_get("source_type").ok());
                insert_source_type(&mut source_types, r.try_get("source_url").ok());
                let placed_by: String = r.try_get("placed_by").unwrap_or_else(|_| "ai".to_string());

                let mut adjusted = *score;
                if knowledge_type == "pattern" {
                    adjusted += BONUS_PATTERN;
                }
                if knowledge_type == "log" {
                    adjusted += BONUS_LOG;
                }
                if placed_by == "user" {
                    adjusted += BONUS_USER_PLACED;
                }
                if let (Some(pid), Some(cpid)) = (project_id, &chunk_project) {
                    if pid == cpid {
                        adjusted += BONUS_SAME_PROJECT;
                    }
                }

                match knowledge_type.as_str() {
                    "pattern"
                        if adjusted >= MEMORY_PATTERN_THRESHOLD
                            && pattern_context.len() < MEMORY_PATTERN_LIMIT =>
                    {
                        let hint = content.chars().take(50).collect::<String>();
                        pattern_hints.push(hint);
                        pattern_context.push(content);
                        if !mc_id.is_empty() {
                            retrieved_memory_chunk_ids.push(mc_id);
                        }
                        if let Some(title) = source_title.as_ref().filter(|t| !t.is_empty()) {
                            if !citation_sources.iter().any(|s| s == title) {
                                citation_sources.push(title.clone());
                            }
                        }
                    }
                    "data"
                        if adjusted >= MEMORY_DATA_THRESHOLD
                            && data_context.len() < MEMORY_DATA_LIMIT =>
                    {
                        if !mc_id.is_empty() {
                            retrieved_memory_chunk_ids.push(mc_id);
                        }
                        if let Some(title) = source_title.as_ref().filter(|t| !t.is_empty()) {
                            data_context.push(format!("[  : {}]\\n{}", title, content));
                            if !citation_sources.iter().any(|s| s == title) {
                                citation_sources.push(title.clone());
                            }
                        } else {
                            data_context.push(content);
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut log_sql = String::from(
            "SELECT m.content, m.trigger_context, s.title AS source_title, s.type AS source_type, s.url AS source_url \
             FROM memory_chunks m \
             LEFT JOIN sources s ON m.source_id = s.id \
             WHERE m.knowledge_type = 'log'",
        );
        let mut log_sid_binds: Vec<&str> = Vec::new();
        let mut log_filter_conds = Vec::new();
        let mut is_filtered = false;

        if let Some(sids) = source_ids {
            is_filtered = true;
            if !sids.is_empty() {
                let ph = sids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                log_filter_conds.push(format!("m.source_id IN ({})", ph));
                log_sid_binds.extend(sids.iter().map(|s| s.as_str()));
            }
        }

        if let Some(tags) = tag_filter {
            is_filtered = true;
            if !tags.is_empty() {
                let tag_conds: Vec<String> = tags
                    .iter()
                    .map(|t| format!("m.tags LIKE '%\"{}%\"'", t.replace('\'', "''")))
                    .collect();
                log_filter_conds.push(format!("({})", tag_conds.join(" OR ")));
            }
        }

        let log_skip = is_filtered && log_filter_conds.is_empty();

        if !log_filter_conds.is_empty() {
            log_sql.push_str(&format!(" AND ({})", log_filter_conds.join(" OR ")));
        }

        let log_rows = if log_skip {
            Vec::new()
        } else {
            let mut q = sqlx::query(&log_sql);
            for sid in &log_sid_binds {
                q = q.bind(*sid);
            }
            q.fetch_all(&self.pool).await.map_err(|e| e.to_string())?
        };

        let query_lower = query.to_lowercase();
        for r in log_rows {
            let trigger: String = r.try_get("trigger_context").unwrap_or_default();
            let triggered = trigger
                .split('|')
                .any(|t| query_lower.contains(&t.trim().to_lowercase()));
            if triggered {
                let content: String = r.try_get("content").unwrap_or_default();
                insert_source_type(&mut source_types, r.try_get("source_type").ok());
                insert_source_type(&mut source_types, r.try_get("source_url").ok());
                let hint = content.chars().take(50).collect::<String>();
                log_hints.push(hint);
                log_context.push(content);
                if let Ok(title) = r.try_get::<String, _>("source_title") {
                    if !title.is_empty() && !citation_sources.iter().any(|s| s == &title) {
                        citation_sources.push(title);
                    }
                }
            }
        }

        let mut external_context: Vec<String> = Vec::new();
        let external_dbs: Vec<String> = sqlx::query_scalar(
            "SELECT db_path FROM external_knowledge_bases WHERE status = 'connected'",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        for db_path in external_dbs {
            let url = format!("sqlite:{}?mode=ro", db_path);
            if let Ok(ext_pool) = sqlx::SqlitePool::connect(&url).await {
                let query_like = format!("%{}%", query);
                if let Ok(ext_rows) = sqlx::query(
                    "SELECT clean_content FROM captures WHERE clean_content LIKE ? ORDER BY created_at DESC LIMIT ?"
                )
                .bind(&query_like)
                .bind(MEMORY_EXTERNAL_LIMIT as i64)
                .fetch_all(&ext_pool)
                .await {
                    for r in ext_rows {
                        if let Ok(c) = r.try_get::<String, _>("clean_content") {
                            external_context.push(c);
                        }
                    }
                }
                let _ = ext_pool.close().await;
            }
        }

        let mut all_retrieved_ids: Vec<String> = retrieved_capture_ids;
        all_retrieved_ids.extend(retrieved_memory_chunk_ids);

        if !all_retrieved_ids.is_empty() {
            let relation_engine = crate::services::chunk_relation_engine::ChunkRelationEngine::new(
                self.pool.clone(),
                self.embedder.clone(),
                self.vector_store.clone(),
            );
            if let Ok(linked) = relation_engine
                .fetch_linked_chunks(&all_retrieved_ids)
                .await
            {
                for lc in linked {
                    if !lc.content.is_empty()
                        && data_context.len() < CAPTURES_LIMIT + MEMORY_DATA_LIMIT
                    {
                        let label = match lc.relation.as_str() {
                            "contradicts" => "Contradiction",
                            "extends" => "Extension",
                            _ => "Related",
                        };
                        data_context.push(format!("[{label}]\n{}", lc.content));
                    }
                }
            }
        }

        if !all_retrieved_ids.is_empty() {
            if let Ok(neighbors) = self
                .fetch_neighbor_capture_context(&all_retrieved_ids)
                .await
            {
                for ctx in neighbors {
                    if !ctx.is_empty()
                        && data_context.len() < CAPTURES_LIMIT + MEMORY_DATA_LIMIT + 4
                    {
                        data_context.push(ctx);
                    }
                }
            }
        }

        let compiled_knowledge: Vec<String> = {
            let mut ck: Vec<String> = Vec::new();

            if let Ok(global) = sqlx::query_scalar::<_, String>(
                "SELECT content FROM compiled_knowledge WHERE space_id IS NULL ORDER BY updated_at DESC LIMIT 1"
            )
            .fetch_optional(&self.pool)
            .await
            {
                if let Some(text) = global {
                    if !text.trim().is_empty() {
                        ck.push(text);
                    }
                }
            }

            if let Some(pid) = project_id {
                if let Ok(rows) = sqlx::query_scalar::<_, String>(
                    "SELECT ck.content FROM compiled_knowledge ck
                     INNER JOIN memory_chunks mc ON mc.space_id = ck.space_id
                     WHERE mc.project_id = ?
                     GROUP BY ck.id
                     ORDER BY ck.updated_at DESC
                     LIMIT 2",
                )
                .bind(pid)
                .fetch_all(&self.pool)
                .await
                {
                    for text in rows {
                        if !text.trim().is_empty() && !ck.contains(&text) {
                            ck.push(text);
                        }
                    }
                }
            }

            ck
        };

        Ok(json!({
            "compiled_knowledge": compiled_knowledge,
            "pattern": pattern_context,
            "log": log_context,
            "data": data_context,
            "external": external_context,
            "citation_sources": citation_sources,
            "source_types": source_types.into_iter().collect::<Vec<_>>(),
            "pattern_hints": pattern_hints,
            "log_hints": log_hints,
        }))
    }

    async fn fetch_neighbor_capture_context(
        &self,
        chunk_ids: &[String],
    ) -> Result<Vec<String>, String> {
        if chunk_ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders = chunk_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT c.id, c.source_id, c.chunk_index \
             FROM captures c \
             WHERE c.id IN ({}) AND c.source_id IS NOT NULL",
            placeholders
        );

        let mut q = sqlx::query(&sql);
        for id in chunk_ids {
            q = q.bind(id);
        }

        let hits = q.fetch_all(&self.pool).await.map_err(|e| e.to_string())?;
        let hit_ids: HashSet<String> = chunk_ids.iter().cloned().collect();
        let mut seen: HashSet<String> = hit_ids.clone();
        let mut contexts = Vec::new();

        for hit in hits {
            let source_id: String = hit.try_get("source_id").unwrap_or_default();
            let chunk_index: i64 = hit.try_get("chunk_index").unwrap_or(0);
            if source_id.is_empty() {
                continue;
            }

            let rows = sqlx::query(
                "SELECT c.id, c.clean_content, c.chunk_index, s.title AS source_title \
                 FROM captures c \
                 LEFT JOIN sources s ON c.source_id = s.id \
                 WHERE c.source_id = ? \
                   AND c.status = 'processed' \
                   AND c.capture_method != 'temp_attachment' \
                   AND c.chunk_index BETWEEN ? AND ? \
                 ORDER BY c.chunk_index ASC",
            )
            .bind(&source_id)
            .bind(chunk_index.saturating_sub(2))
            .bind(chunk_index + 2)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

            for row in rows {
                let id: String = row.try_get("id").unwrap_or_default();
                if id.is_empty() || seen.contains(&id) {
                    continue;
                }
                seen.insert(id);

                let content: String = row.try_get("clean_content").unwrap_or_default();
                if content.trim().is_empty() {
                    continue;
                }
                let title: String = row.try_get("source_title").unwrap_or_default();
                let idx: i64 = row.try_get("chunk_index").unwrap_or(0);
                let label = if title.is_empty() {
                    format!("       #{}", idx)
                } else {
                    format!("      : {} #{}", title, idx)
                };
                contexts.push(format!("[{}]\n{}", label, content));

                if contexts.len() >= 6 {
                    return Ok(contexts);
                }
            }
        }

        Ok(contexts)
    }

    pub async fn build_prompt(
        &self,
        query: &str,
        history: Vec<(String, String)>,
        conversation_summary: Option<String>,
        project_id: Option<String>,
        source_ids: Option<Vec<String>>,
        tag_filter: Option<Vec<String>>,
        rag_enabled: bool,
        temp_chunk_ids: Option<Vec<String>>,
        instruction_override: Option<String>,
    ) -> Result<
        (
            String,
            Vec<(String, String)>,
            Vec<String>,
            serde_json::Value,
        ),
        String,
    > {
        let settings = crate::settings::store::get_settings(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        // 無論 RAG 開關，我們都執行檢索
        // RAG 開：檢索全部 (only_memory=false)
        // RAG 關：僅檢索 Pattern/Log/Data 記憶 (only_memory=true)
        let ctx = self
            .retrieve_context(
                query,
                project_id.as_deref(),
                source_ids.as_deref(),
                tag_filter.as_deref(),
                !rag_enabled,
            )
            .await?;

        let mut system_parts: Vec<String> = Vec::new();
        let mut extra_citations: Vec<String> = Vec::new();
        let mut extra_source_types: HashSet<String> = HashSet::new();

        if let Some(ids) = &temp_chunk_ids {
            if !ids.is_empty() {
                let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let sql = format!(
                    "SELECT clean_content FROM captures WHERE id IN ({}) AND capture_method = 'temp_attachment'",
                    placeholders
                );
                let mut q = sqlx::query(&sql);
                for id in ids {
                    q = q.bind(id);
                }
                if let Ok(rows) = q.fetch_all(&self.pool).await {
                    let contents: Vec<String> = rows
                        .into_iter()
                        .filter_map(|r| r.try_get::<String, _>("clean_content").ok())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if !contents.is_empty() {
                        system_parts
                            .push(format!("##          \n{}", contents.join("\n\n---\n\n")));
                    }
                }
            }
        }

        if let Some(sids) = &source_ids {
            if !sids.is_empty() {
                eprintln!("[RAG] @ mention source_ids: {:?}", sids);
                let placeholders = sids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

                let cap_sql = format!(
                    "SELECT c.clean_content, c.type AS capture_type, c.content_type, s.title AS source_title, s.type AS source_type, s.url AS source_url \
                     FROM captures c \
                     LEFT JOIN sources s ON c.source_id = s.id \
                     WHERE c.source_id IN ({}) AND c.status = 'processed' AND c.capture_method != 'temp_attachment' \
                     ORDER BY c.chunk_index ASC",
                    placeholders
                );
                let mut q = sqlx::query(&cap_sql);
                for sid in sids {
                    q = q.bind(sid);
                }
                let mut source_contents: Vec<String> = Vec::new();
                if let Ok(rows) = q.fetch_all(&self.pool).await {
                    for r in &rows {
                        if let Ok(content) = r.try_get::<String, _>("clean_content") {
                            if !content.is_empty() {
                                let title =
                                    r.try_get::<String, _>("source_title").unwrap_or_default();
                                insert_source_type(
                                    &mut extra_source_types,
                                    r.try_get("source_type").ok(),
                                );
                                insert_source_type(
                                    &mut extra_source_types,
                                    r.try_get("source_url").ok(),
                                );
                                insert_source_type(
                                    &mut extra_source_types,
                                    r.try_get("capture_type").ok(),
                                );
                                insert_source_type(
                                    &mut extra_source_types,
                                    r.try_get("content_type").ok(),
                                );
                                if !title.is_empty() {
                                    source_contents.push(format!("[  : {}]\n{}", title, content));
                                    if !extra_citations.iter().any(|s| s == &title) {
                                        extra_citations.push(title);
                                    }
                                } else {
                                    source_contents.push(content);
                                }
                            }
                        }
                    }
                }

                if source_contents.is_empty() {
                    eprintln!("[RAG] No captures found for @ sources, falling back to sources.clean_content");
                    let src_sql = format!(
                        "SELECT id, title, type AS source_type, url AS source_url, clean_content, file_path FROM sources WHERE id IN ({})",
                        placeholders
                    );
                    let mut q2 = sqlx::query(&src_sql);
                    for sid in sids {
                        q2 = q2.bind(sid);
                    }
                    if let Ok(src_rows) = q2.fetch_all(&self.pool).await {
                        for r in &src_rows {
                            let title: String = r.try_get("title").unwrap_or_default();
                            insert_source_type(
                                &mut extra_source_types,
                                r.try_get("source_type").ok(),
                            );
                            insert_source_type(
                                &mut extra_source_types,
                                r.try_get("source_url").ok(),
                            );
                            let mut content: String =
                                r.try_get("clean_content").unwrap_or_default();

                            if content.trim().is_empty() {
                                if let Ok(fpath) = r.try_get::<String, _>("file_path") {
                                    if !fpath.is_empty() {
                                        let kb_path =
                                            crate::settings::store::get_settings(&self.pool)
                                                .await
                                                .map(|s| s.knowledge.kb_path)
                                                .unwrap_or_default();
                                        if let Ok(parsed) = crate::capture::file_parser::parse_file(
                                            &kb_path, &fpath, None,
                                        )
                                        .await
                                        {
                                            content = parsed
                                                .chunks
                                                .iter()
                                                .map(|c| c.content.as_str())
                                                .collect::<Vec<_>>()
                                                .join("\n\n");
                                        }
                                    }
                                }
                            }

                            if !content.trim().is_empty() {
                                if !title.is_empty() {
                                    source_contents.push(format!("[  : {}]\n{}", title, content));
                                    if !extra_citations.iter().any(|s| s == &title) {
                                        extra_citations.push(title);
                                    }
                                } else {
                                    source_contents.push(content);
                                }
                            }
                        }
                    }
                }

                if !source_contents.is_empty() {
                    eprintln!(
                        "[RAG] Injecting {} @ source content(s), total chars: {}",
                        source_contents.len(),
                        source_contents.iter().map(|s| s.len()).sum::<usize>()
                    );
                    system_parts.push(format!(
                        "##         \n{}",
                        source_contents.join("\n\n---\n\n")
                    ));
                } else {
                    eprintln!("[RAG] @ mention source_ids provided but NO content found!");
                }
            }
        }

        let compiled = ctx["compiled_knowledge"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0);
        if compiled > 0 {
            let text = ctx["compiled_knowledge"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            system_parts.push(format!("{}\n{}", prompts::RAG_CONTEXT_COMPILED, text));
        }

        let patterns = ctx["pattern"].as_array().map(|a| a.len()).unwrap_or(0);
        if patterns > 0 {
            let text = ctx["pattern"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            system_parts.push(format!("{}\n{}", prompts::RAG_CONTEXT_PATTERN, text));
        }

        let logs = ctx["log"].as_array().map(|a| a.len()).unwrap_or(0);
        if logs > 0 {
            let text = ctx["log"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            system_parts.push(format!("{}\n{}", prompts::RAG_CONTEXT_LOG, text));
        }

        let data_items = ctx["data"].as_array().map(|a| a.len()).unwrap_or(0);
        if data_items > 0 {
            let text = ctx["data"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            system_parts.push(format!("{}\n{}", prompts::RAG_CONTEXT_DATA, text));
        }

        let externals = ctx["external"].as_array().map(|a| a.len()).unwrap_or(0);
        if externals > 0 {
            let text = ctx["external"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            system_parts.push(format!("{}\n{}", prompts::RAG_CONTEXT_EXTERNAL, text));
        }

        // 注入提醒訊息 (Reminders) - 永遠啟用
        if settings.reminders.ai_enabled {
            let reminder_engine =
                crate::services::reminder_engine::ReminderEngine::new(self.pool.clone());
            if let Ok(active_reminders) = reminder_engine.get_active_reminders().await {
                if !active_reminders.is_empty() {
                    let now_local = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                    let mut reminder_text = format!("## 待辦提醒資訊 (當前時間: {})\n", now_local);
                    for r in active_reminders {
                        let title = r["title"].as_str().unwrap_or("Unknown");
                        let date = r["event_date"].as_str().unwrap_or("");
                        let time = r["event_time"].as_str().unwrap_or("");
                        let r_type = r["event_type"].as_str().unwrap_or("event");
                        reminder_text
                            .push_str(&format!("- [{}] {} {} ({})\n", r_type, date, time, title));
                    }
                    reminder_text.push_str("\n若用戶詢問相關行程或待辦事項，請以此資訊回覆。");
                    system_parts.push(reminder_text);
                }
            }
        }

        let ctx_citation_count = ctx["citation_sources"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0);
        let total_citation_count = ctx_citation_count + extra_citations.len();
        let selected_source_count = source_ids.as_ref().map(|ids| ids.len()).unwrap_or(0);
        let temp_attachment_count = temp_chunk_ids.as_ref().map(|ids| ids.len()).unwrap_or(0);
        let mut all_source_types = ctx["source_types"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        all_source_types.extend(extra_source_types.iter().cloned());
        all_source_types.sort();
        all_source_types.dedup();
        let source_scope_guidance = if rag_enabled {
            Some(build_source_scope_guidance(
                selected_source_count,
                temp_attachment_count,
                total_citation_count,
                data_items,
                patterns,
                logs,
                compiled,
                externals,
                &all_source_types,
            ))
        } else {
            None
        };

        if let Some(guidance) = &source_scope_guidance {
            system_parts.push(format!("## Source Scope\n{}", guidance.guidance));
        }
        let has_source_scope = source_ids
            .as_ref()
            .map(|ids| !ids.is_empty())
            .unwrap_or(false);
        let has_tag_scope = tag_filter
            .as_ref()
            .map(|tags| !tags.is_empty())
            .unwrap_or(false);
        if has_source_scope || has_tag_scope {
            system_parts.push(
                "## User Selected Scope\n@ source chips and # tag chips are a union retrieval scope: any selected source or selected tag may provide usable context. Cite the actual source content you used; do not treat a tag itself as evidence."
                    .to_string(),
            );
        }

        let user_instruction = instruction_override
            .unwrap_or_else(|| settings.chat_prompt_instruction.trim().to_string());

        let system_prompt = prompts::build_chat_system_prompt(prompts::ChatPromptInput {
            kind: prompts::ChatPromptKind::Rag,
            conversation_summary: conversation_summary.as_deref(),
            context_sections: &system_parts,
            user_instruction: &user_instruction,
        });

        let mut citation_sources: Vec<String> = ctx["citation_sources"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for title in extra_citations {
            if !citation_sources.iter().any(|s| s == &title) {
                citation_sources.push(title);
            }
        }

        let context_hints = json!({
            "compiledKnowledgeCount": ctx["compiled_knowledge"].as_array().map(|a| a.len()).unwrap_or(0),
            "patternCount": ctx["pattern"].as_array().map(|a| a.len()).unwrap_or(0),
            "logCount": ctx["log"].as_array().map(|a| a.len()).unwrap_or(0),
            "dataCount": ctx["data"].as_array().map(|a| a.len()).unwrap_or(0),
            "sourceScope": source_scope_guidance.as_ref().map(|g| g.scope).unwrap_or("rag_disabled"),
            "sourceScopePhrase": source_scope_guidance.as_ref().map(|g| g.phrase).unwrap_or("no RAG context"),
            "sourceCount": total_citation_count,
            "sourceTypes": all_source_types,
            "patternHints": ctx.get("pattern_hints").cloned().unwrap_or(json!([])),
            "logHints": ctx.get("log_hints").cloned().unwrap_or(json!([])),
        });

        Ok((system_prompt, history, citation_sources, context_hints))
    }

    pub async fn generate_answer(
        &self,
        query: &str,
        history: Vec<(String, String)>,
        conversation_summary: Option<String>,
        project_id: Option<String>,
        source_ids: Option<Vec<String>>,
        tag_filter: Option<Vec<String>>,
        rag_enabled: bool,
        temp_chunk_ids: Option<Vec<String>>,
        think_mode: bool,
        web_context: Option<String>,
        instruction_override: Option<String>,
    ) -> Result<serde_json::Value, String> {
        let settings = crate::settings::store::get_settings(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let ai_usage = settings.ai_usage.clone();
        let cfg = settings.ai_models.chat_llm;

        let is_ollama = cfg.provider == "ollama";
        let api_key = cfg.api_key.clone().unwrap_or_default();
        let opt_provider = if !api_key.is_empty() || is_ollama {
            Some(OpenAiProvider::new(
                api_key,
                cfg.base_url.clone(),
                cfg.model.clone(),
                cfg.provider.clone(),
            ))
        } else {
            None
        };

        let (base_prompt, history_vec, citation_sources, context_hints) = self
            .build_prompt(
                query,
                history,
                conversation_summary,
                project_id,
                source_ids,
                tag_filter,
                rag_enabled,
                temp_chunk_ids,
                instruction_override,
            )
            .await?;

        let base_prompt_with_web = if let Some(web_ctx) = &web_context {
            format!(
                "{}\n\n{}\n{}",
                base_prompt,
                crate::prompts::RAG_CONTEXT_WEB_SEARCH,
                web_ctx
            )
        } else {
            base_prompt
        };

        let reasoning_style = crate::providers::llm::model_caps::detect(&cfg.model, &cfg.provider);
        let thinking_control =
            crate::providers::llm::model_caps::thinking_control(&cfg.model, &cfg.provider);
        let has_native_reasoning =
            reasoning_style != crate::providers::llm::model_caps::ReasoningStyle::None;
        let system_prompt = if think_mode
            && thinking_control == crate::providers::llm::model_caps::ThinkingControl::None
            && !has_native_reasoning
        {
            format!(
                "{}{}",
                crate::prompts::THINK_MODE_PREFIX,
                base_prompt_with_web
            )
        } else {
            base_prompt_with_web
        };

        let llm_opts = if think_mode {
            LLMOptions {
                temperature: 0.6,
                max_tokens: 8192,
                stream: false,
                think_mode: Some(
                    thinking_control != crate::providers::llm::model_caps::ThinkingControl::None,
                ),
            }
        } else {
            LLMOptions {
                max_tokens: 8192,
                think_mode: Some(false),
                ..LLMOptions::default()
            }
        };
        let llm_opts = apply_llm_usage_policy(
            llm_opts,
            &ai_usage,
            if think_mode {
                LLMTaskKind::InteractiveThink
            } else {
                LLMTaskKind::InteractiveChat
            },
        );

        let is_simulated = opt_provider.is_none();
        let answer = if let Some(llm) = opt_provider {
            llm.complete_with_history(&system_prompt, &history_vec, query, llm_opts)
                .await
                .map_err(|e| e.to_string())?
        } else {
            format!("[    ]     LLM \n   {}", query)
        };

        Ok(json!({
            "answer": answer,
            "is_simulated": is_simulated,
            "citationSources": citation_sources,
            "contextHints": context_hints,
        }))
    }
}
