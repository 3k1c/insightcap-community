use std::time::Duration;

use chrono::Utc;
use serde::Deserialize;
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::db::AppState;
use crate::prompts;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::usage_policy::{apply_llm_usage_policy, LLMTaskKind};
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::settings::store::get_settings;

const DEFAULT_FREQUENCY_MINUTES: u64 = 30;
const LLM_TIMEOUT_SECS: u64 = 60;

#[derive(Debug, Deserialize)]
struct SynthesisResult {
    #[serde(default)]
    updated_entities: Vec<EntityUpdate>,
    #[serde(default)]
    updated_concepts: Vec<ConceptUpdate>,
    #[serde(default)]
    new_syntheses: Vec<NewSynthesis>,
    #[serde(default)]
    contradictions: Vec<Contradiction>,
}

#[derive(Debug, Deserialize)]
struct EntityUpdate {
    chunk_id: String,
    #[serde(default)]
    entity: String,
    #[serde(default)]
    #[allow(dead_code)]
    summary: String,
}

#[derive(Debug, Deserialize)]
struct ConceptUpdate {
    chunk_id: String,
    #[serde(default)]
    concept: String,
    #[serde(default)]
    #[allow(dead_code)]
    summary: String,
}

#[derive(Debug, Deserialize)]
struct NewSynthesis {
    #[serde(default)]
    source_ids: Vec<String>,
    #[serde(default)]
    synthesis: String,
}

#[derive(Debug, Deserialize)]
struct Contradiction {
    #[serde(default)]
    chunk_ids: Vec<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    confidence: f32,
}

pub fn start_deep_synthesis_worker(app: AppHandle) {
    let mut shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();

    tauri::async_runtime::spawn(async move {
        println!("[DeepSynthesis] Worker   ");

        tokio::time::sleep(Duration::from_secs(300)).await;

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[DeepSynthesis]           ");
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(DEFAULT_FREQUENCY_MINUTES * 60)) => {
                    run_deep_synthesis(&app).await;
                }
            }
        }
    });
}

async fn run_deep_synthesis(app: &AppHandle) {
    let app_state = app.state::<AppState>();
    let pool = app_state.db.clone();

    let settings = match get_settings(&pool).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[DeepSynthesis]       : {}", e);
            return;
        }
    };

    if !settings.background_synthesis.enabled {
        return;
    }

    let conv_lock = app_state.current_conversation_id.lock().await;
    if conv_lock.is_some() {
        println!("[DeepSynthesis]           ");
        return;
    }
    drop(conv_lock);

    let ai_usage = settings.ai_usage.clone();
    let cfg = settings.ai_models.content_processor_llm;
    let api_key = cfg.api_key.clone().unwrap_or_default();

    if api_key.is_empty() && cfg.provider != "ollama" {
        println!("[DeepSynthesis]    content_processor_llm    ");
        return;
    }

    let max_chunks = settings.background_synthesis.max_chunks_per_batch as i64;
    let candidates = match fetch_candidates(&pool, max_chunks).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[DeepSynthesis]       : {}", e);
            return;
        }
    };

    if candidates.is_empty() {
        return;
    }

    println!("[DeepSynthesis]    {}     chunk     ", candidates.len());

    let llm = OpenAiProvider::new(
        api_key,
        cfg.base_url,
        cfg.model.clone(),
        cfg.provider.clone(),
    );

    let new_chunks_text = candidates
        .iter()
        .map(|c| {
            format!(
                "[{}] (type={}, space={})\n{}",
                c.id,
                c.knowledge_type,
                c.space_id.as_deref().unwrap_or("none"),
                c.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n---\n");

    let existing_knowledge = match fetch_existing_knowledge(&pool).await {
        Ok(k) => k,
        Err(_) => String::new(),
    };

    let prompt = prompts::build_deep_synthesis_prompt(prompts::DeepSynthesisPromptInput {
        new_chunks: &new_chunks_text,
        existing_knowledge: &existing_knowledge,
    });

    let opts = apply_llm_usage_policy(
        LLMOptions {
            temperature: 0.2,
            max_tokens: 2048,
            stream: false,
            think_mode: None,
        },
        &ai_usage,
        LLMTaskKind::BackgroundSynthesis,
    );

    let result = match tokio::time::timeout(
        Duration::from_secs(LLM_TIMEOUT_SECS),
        llm.complete_json(&prompt, opts),
    )
    .await
    {
        Ok(Ok(json)) => json,
        Ok(Err(e)) => {
            eprintln!("[DeepSynthesis] LLM     : {}", e);
            return;
        }
        Err(_) => {
            eprintln!("[DeepSynthesis] LLM      ({}s)", LLM_TIMEOUT_SECS);
            return;
        }
    };

    let synthesis: SynthesisResult = match serde_json::from_value(result.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[DeepSynthesis] JSON     : {} | raw: {}", e, result);
            return;
        }
    };

    let chunk_ids: Vec<String> = candidates.iter().map(|c| c.id.clone()).collect();

    let mut total_writes = 0usize;

    total_writes += process_entities(&pool, &synthesis.updated_entities, &chunk_ids).await;
    total_writes += process_concepts(&pool, &synthesis.updated_concepts, &chunk_ids).await;
    total_writes += process_syntheses(&pool, &synthesis.new_syntheses, &candidates).await;
    total_writes += process_contradictions(&pool, &synthesis.contradictions, &chunk_ids).await;

    mark_synthesized(&pool, &chunk_ids).await;

    if total_writes > 0 {
        println!("[DeepSynthesis]        {}    ", total_writes);
        let _ = app.emit("deep-synthesis-completed", total_writes);
    }

    let mut space_ids: Vec<Option<String>> =
        candidates.iter().map(|c| c.space_id.clone()).collect();
    space_ids.sort();
    space_ids.dedup();
    if !space_ids.contains(&None) {
        space_ids.push(None);
    }

    for sid in &space_ids {
        if let Err(e) = generate_compiled_knowledge(&pool, &llm, &ai_usage, sid.as_deref()).await {
            eprintln!("[DeepSynthesis]          (space={:?}): {}", sid, e);
        }
    }
}

async fn generate_compiled_knowledge(
    pool: &SqlitePool,
    llm: &OpenAiProvider,
    ai_usage: &crate::settings::store::AIUsageSettings,
    space_id: Option<&str>,
) -> Result<(), String> {
    let chunks_text: String = if let Some(sid) = space_id {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT content FROM memory_chunks
             WHERE knowledge_type IN ('pattern', 'log')
               AND pending_confirm = 0
               AND promotion_count >= 1
               AND space_id = ?
             ORDER BY promotion_count DESC
             LIMIT 20",
        )
        .bind(sid)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
        rows.join("\n---\n")
    } else {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT content FROM memory_chunks
             WHERE knowledge_type IN ('pattern', 'log')
               AND pending_confirm = 0
               AND promotion_count >= 1
             ORDER BY promotion_count DESC
             LIMIT 20",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
        rows.join("\n---\n")
    };

    if chunks_text.trim().len() < 50 {
        return Ok(());
    }

    let prompt = prompts::build_compiled_knowledge_prompt(prompts::CompiledKnowledgePromptInput {
        chunks: &chunks_text,
    });

    let opts = apply_llm_usage_policy(
        LLMOptions {
            temperature: 0.3,
            max_tokens: 1024,
            stream: false,
            think_mode: None,
        },
        ai_usage,
        LLMTaskKind::BackgroundSummary,
    );

    let result = match tokio::time::timeout(
        Duration::from_secs(LLM_TIMEOUT_SECS),
        llm.complete(&prompt, opts),
    )
    .await
    {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => return Err(format!("LLM     : {}", e)),
        Err(_) => return Err("LLM     ".to_string()),
    };

    let content = result.trim().to_string();
    if content.len() < 20 || content.to_uppercase().contains("INSUFFICIENT") {
        return Ok(());
    }

    let now = Utc::now().to_rfc3339();
    let new_id = Uuid::now_v7().to_string();

    sqlx::query(
        "INSERT INTO compiled_knowledge (id, space_id, content, source_chunk_count, created_at, updated_at)
         VALUES (?, ?, ?, 20, ?, ?)
         ON CONFLICT (COALESCE(space_id, '__global__'))
         DO UPDATE SET content = excluded.content,
                       source_chunk_count = excluded.source_chunk_count,
                       updated_at = excluded.updated_at",
    )
    .bind(&new_id)
    .bind(space_id)
    .bind(&content)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    println!(
        "[DeepSynthesis]         (space={:?}, {} chars)",
        space_id,
        content.len()
    );
    Ok(())
}

struct CandidateChunk {
    id: String,
    content: String,
    space_id: Option<String>,
    knowledge_type: String,
}

async fn fetch_candidates(pool: &SqlitePool, limit: i64) -> Result<Vec<CandidateChunk>, String> {
    let rows = sqlx::query(
        "SELECT id, content, space_id, knowledge_type
         FROM memory_chunks
         WHERE knowledge_type IN ('pattern', 'log')
           AND pending_confirm = 0
           AND updated_at > datetime('now', '-24 hours')
           AND (last_synthesized_at IS NULL
                OR last_synthesized_at < datetime('now', '-12 hours'))
         ORDER BY promotion_count DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|r| CandidateChunk {
            id: r.try_get("id").unwrap_or_default(),
            content: r.try_get("content").unwrap_or_default(),
            space_id: r.try_get("space_id").ok(),
            knowledge_type: r.try_get("knowledge_type").unwrap_or_default(),
        })
        .collect())
}

async fn fetch_existing_knowledge(pool: &SqlitePool) -> Result<String, String> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT content FROM memory_chunks
         WHERE knowledge_type IN ('pattern', 'log')
           AND pending_confirm = 0
           AND promotion_count >= 2
         ORDER BY promotion_count DESC
         LIMIT 10",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        Ok("No prior promoted knowledge found.".to_string())
    } else {
        Ok(rows.join("\n---\n"))
    }
}

async fn process_entities(
    pool: &SqlitePool,
    entities: &[EntityUpdate],
    valid_ids: &[String],
) -> usize {
    let mut count = 0;
    for e in entities {
        if !valid_ids.contains(&e.chunk_id) {
            continue;
        }
        if e.entity.is_empty() {
            continue;
        }

        if append_tag(pool, &e.chunk_id, &format!("entity:{}", e.entity))
            .await
            .is_ok()
        {
            count += 1;
        }
    }
    count
}

async fn process_concepts(
    pool: &SqlitePool,
    concepts: &[ConceptUpdate],
    valid_ids: &[String],
) -> usize {
    let mut count = 0;
    for c in concepts {
        if !valid_ids.contains(&c.chunk_id) {
            continue;
        }
        if c.concept.is_empty() {
            continue;
        }

        if append_tag(pool, &c.chunk_id, &format!("concept:{}", c.concept))
            .await
            .is_ok()
        {
            count += 1;
        }
    }
    count
}

async fn process_syntheses(
    pool: &SqlitePool,
    syntheses: &[NewSynthesis],
    candidates: &[CandidateChunk],
) -> usize {
    let mut count = 0;
    for s in syntheses {
        if s.synthesis.trim().len() < 10 {
            continue;
        }
        let all_valid = s
            .source_ids
            .iter()
            .all(|sid| candidates.iter().any(|c| c.id == *sid));
        if !all_valid || s.source_ids.is_empty() {
            continue;
        }

        let space_id = s.source_ids.first().and_then(|sid| {
            candidates
                .iter()
                .find(|c| c.id == *sid)
                .and_then(|c| c.space_id.clone())
        });

        let now = Utc::now().to_rfc3339();
        let chunk_id = Uuid::now_v7().to_string();

        let tags = serde_json::json!(["synthesized", "deep_synthesis"]);

        let result = sqlx::query(
            "INSERT INTO memory_chunks (id, space_id, knowledge_type, content, tags, confidence, pending_confirm, promotion_count, placed_by, created_at, updated_at, last_synthesized_at)
             VALUES (?, ?, 'pattern', ?, ?, 0.8, 0, 0, 'deep_synthesis', ?, ?, ?)",
        )
        .bind(&chunk_id)
        .bind(&space_id)
        .bind(s.synthesis.trim())
        .bind(tags.to_string())
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await;

        match result {
            Ok(_) => {
                for source_id in &s.source_ids {
                    let _ = write_relation(
                        pool,
                        &chunk_id,
                        "memory_chunk",
                        source_id,
                        "memory_chunk",
                        "extends",
                        0.8,
                    )
                    .await;
                }
                count += 1;
            }
            Err(e) => {
                eprintln!("[DeepSynthesis]      chunk   : {}", e);
            }
        }
    }
    count
}

async fn process_contradictions(
    pool: &SqlitePool,
    contradictions: &[Contradiction],
    valid_ids: &[String],
) -> usize {
    let mut count = 0;
    for c in contradictions {
        if c.confidence < 0.7 {
            continue;
        }
        if c.chunk_ids.len() < 2 {
            continue;
        }
        if !c.chunk_ids.iter().all(|id| valid_ids.contains(id)) {
            continue;
        }

        for i in 0..c.chunk_ids.len() {
            for j in (i + 1)..c.chunk_ids.len() {
                match write_relation(
                    pool,
                    &c.chunk_ids[i],
                    "memory_chunk",
                    &c.chunk_ids[j],
                    "memory_chunk",
                    "contradicts",
                    c.confidence,
                )
                .await
                {
                    Ok(_) => count += 1,
                    Err(e) => eprintln!("[DeepSynthesis]         : {}", e),
                }
            }
        }

        if c.description.trim().len() >= 10 {
            let now = Utc::now().to_rfc3339();
            let chunk_id = Uuid::now_v7().to_string();
            let tags = serde_json::json!(["contradiction", "deep_synthesis"]);

            let _ = sqlx::query(
                "INSERT INTO memory_chunks (id, knowledge_type, content, tags, confidence, pending_confirm, placed_by, created_at, updated_at, last_synthesized_at)
                 VALUES (?, 'log', ?, ?, ?, 0, 'deep_synthesis', ?, ?, ?)",
            )
            .bind(&chunk_id)
            .bind(format!("        {}", c.description.trim()))
            .bind(tags.to_string())
            .bind(c.confidence)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await;
        }
    }
    count
}

async fn append_tag(pool: &SqlitePool, chunk_id: &str, new_tag: &str) -> Result<(), String> {
    let existing: String = sqlx::query_scalar("SELECT tags FROM memory_chunks WHERE id = ?")
        .bind(chunk_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "[]".to_string());

    let mut tags: Vec<String> = serde_json::from_str(&existing).unwrap_or_default();

    if tags.iter().any(|t| t == new_tag) {
        return Ok(());
    }

    tags.push(new_tag.to_string());
    let tags_json = serde_json::to_string(&tags).map_err(|e| e.to_string())?;

    sqlx::query("UPDATE memory_chunks SET tags = ? WHERE id = ?")
        .bind(&tags_json)
        .bind(chunk_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

async fn write_relation(
    pool: &SqlitePool,
    from_id: &str,
    from_type: &str,
    to_id: &str,
    to_type: &str,
    relation: &str,
    confidence: f32,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT OR IGNORE INTO chunk_relations (id, from_id, to_id, from_type, to_type, relation, confidence, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(from_id)
    .bind(to_id)
    .bind(from_type)
    .bind(to_type)
    .bind(relation)
    .bind(confidence)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

async fn mark_synthesized(pool: &SqlitePool, chunk_ids: &[String]) {
    let now = Utc::now().to_rfc3339();
    for id in chunk_ids {
        let _ = sqlx::query("UPDATE memory_chunks SET last_synthesized_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(pool)
            .await;
    }
}
