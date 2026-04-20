use std::time::Duration;

use chrono::Utc;
use serde::Deserialize;
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::db::AppState;
use crate::prompts;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::settings::store::get_settings;

// ─── 常數 ─────────────────────────────────────────────────────────────────

/// 預設合成頻率（分鐘）
const DEFAULT_FREQUENCY_MINUTES: u64 = 30;
/// LLM 呼叫超時（秒）
const LLM_TIMEOUT_SECS: u64 = 60;

// ─── LLM 回傳結構 ─────────────────────────────────────────────────────────

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

// ─── 背景 Worker ──────────────────────────────────────────────────────────

/// 啟動深度合成背景工作程式
/// - 每 N 分鐘（settings 可調）檢查候選 chunk
/// - 僅在無活躍對話時執行，避免與 chat LLM 競爭
pub fn start_deep_synthesis_worker(app: AppHandle) {
    let mut shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();

    tauri::async_runtime::spawn(async move {
        println!("[DeepSynthesis] Worker 啟動");

        // 啟動後等待 5 分鐘再開始，給系統初始化的時間
        tokio::time::sleep(Duration::from_secs(300)).await;

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[DeepSynthesis] 收到停止訊號，退出。");
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

    // ── 前置檢查 ──────────────────────────────────────────────────────

    // 1. 讀取設定，檢查功能是否啟用
    let settings = match get_settings(&pool).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[DeepSynthesis] 無法讀取設定: {}", e);
            return;
        }
    };

    if !settings.background_synthesis.enabled {
        return;
    }

    // 2. 智慧跳過：有活躍對話時不執行（Ollama 單模型保護）
    let conv_lock = app_state.current_conversation_id.lock().await;
    if conv_lock.is_some() {
        println!("[DeepSynthesis] 跳過：目前有活躍對話");
        return;
    }
    drop(conv_lock);

    // 3. 使用 content_processor_llm（輕量模型）
    let cfg = settings.ai_models.content_processor_llm;
    let api_key = cfg.api_key.clone().unwrap_or_default();

    if api_key.is_empty() && cfg.provider != "ollama" {
        println!("[DeepSynthesis] 跳過：content_processor_llm 未設定");
        return;
    }

    // ── 查詢候選 chunk ────────────────────────────────────────────────

    let max_chunks = settings.background_synthesis.max_chunks_per_batch as i64;
    let candidates = match fetch_candidates(&pool, max_chunks).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[DeepSynthesis] 查詢候選失敗: {}", e);
            return;
        }
    };

    if candidates.is_empty() {
        return;
    }

    println!(
        "[DeepSynthesis] 找到 {} 筆候選 chunk，開始合成",
        candidates.len()
    );

    // ── 建構 Prompt 並呼叫 LLM ────────────────────────────────────────

    let llm = OpenAiProvider::new(
        api_key,
        cfg.base_url,
        cfg.model.clone(),
        cfg.provider.clone(),
    );

    // 組裝 new_chunks 文字
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

    // 查詢既有相關知識（promotion_count 高的 pattern/log，作為上下文）
    let existing_knowledge = match fetch_existing_knowledge(&pool).await {
        Ok(k) => k,
        Err(_) => String::new(),
    };

    let prompt = prompts::DEEP_SYNTHESIS_PROMPT
        .replace("{{new_chunks}}", &new_chunks_text)
        .replace("{{existing_knowledge}}", &existing_knowledge);

    let opts = LLMOptions {
        temperature: 0.2,
        max_tokens: 2048,
        stream: false,
        think_mode: None,
    };

    let result = match tokio::time::timeout(
        Duration::from_secs(LLM_TIMEOUT_SECS),
        llm.complete_json(&prompt, opts),
    )
    .await
    {
        Ok(Ok(json)) => json,
        Ok(Err(e)) => {
            eprintln!("[DeepSynthesis] LLM 呼叫失敗: {}", e);
            return;
        }
        Err(_) => {
            eprintln!("[DeepSynthesis] LLM 呼叫超時 ({}s)", LLM_TIMEOUT_SECS);
            return;
        }
    };

    // ── 解析 JSON 並安全寫入 ──────────────────────────────────────────

    let synthesis: SynthesisResult = match serde_json::from_value(result.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[DeepSynthesis] JSON 解析失敗: {} | raw: {}", e, result);
            return;
        }
    };

    let chunk_ids: Vec<String> = candidates.iter().map(|c| c.id.clone()).collect();

    // 安全寫入各項結果
    let mut total_writes = 0usize;

    total_writes += process_entities(&pool, &synthesis.updated_entities, &chunk_ids).await;
    total_writes += process_concepts(&pool, &synthesis.updated_concepts, &chunk_ids).await;
    total_writes += process_syntheses(&pool, &synthesis.new_syntheses, &candidates).await;
    total_writes += process_contradictions(&pool, &synthesis.contradictions, &chunk_ids).await;

    // 標記已處理的 chunk
    mark_synthesized(&pool, &chunk_ids).await;

    if total_writes > 0 {
        println!("[DeepSynthesis] 完成，共寫入 {} 筆結果", total_writes);
        let _ = app.emit("deep-synthesis-completed", total_writes);
    }

    // ── 編譯後知識生成（Compiled Knowledge）──────────────────────────────
    // 收集所有 space_id（含 None = 全域），逐個生成精煉知識
    let mut space_ids: Vec<Option<String>> =
        candidates.iter().map(|c| c.space_id.clone()).collect();
    space_ids.sort();
    space_ids.dedup();
    // 確保全域也有一份
    if !space_ids.contains(&None) {
        space_ids.push(None);
    }

    for sid in &space_ids {
        if let Err(e) = generate_compiled_knowledge(&pool, &llm, sid.as_deref()).await {
            eprintln!("[DeepSynthesis] 編譯知識生成失敗 (space={:?}): {}", sid, e);
        }
    }
}

/// 為指定 space（或全域）生成編譯後知識
async fn generate_compiled_knowledge(
    pool: &SqlitePool,
    llm: &OpenAiProvider,
    space_id: Option<&str>,
) -> Result<(), String> {
    // 查詢該 space 的高品質 pattern/log chunks
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
        // 全域：不限 space
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
        // 知識量不足，跳過
        return Ok(());
    }

    let prompt = prompts::COMPILED_KNOWLEDGE_PROMPT.replace("{{chunks}}", &chunks_text);

    let opts = LLMOptions {
        temperature: 0.3,
        max_tokens: 1024,
        stream: false,
        think_mode: None,
    };

    let result = match tokio::time::timeout(
        Duration::from_secs(LLM_TIMEOUT_SECS),
        llm.complete(&prompt, opts),
    )
    .await
    {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => return Err(format!("LLM 呼叫失敗: {}", e)),
        Err(_) => return Err("LLM 呼叫超時".to_string()),
    };

    let content = result.trim().to_string();
    if content.len() < 20 || content.to_uppercase().contains("INSUFFICIENT") {
        return Ok(());
    }

    // UPSERT：使用 COALESCE(space_id, '__global__') 作為唯一鍵
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
        "[DeepSynthesis] 編譯知識已更新 (space={:?}, {} chars)",
        space_id,
        content.len()
    );
    Ok(())
}

// ─── 資料結構 ─────────────────────────────────────────────────────────────

struct CandidateChunk {
    id: String,
    content: String,
    space_id: Option<String>,
    knowledge_type: String,
}

// ─── 查詢函數 ─────────────────────────────────────────────────────────────

/// 查詢候選 chunk：過去 24 小時更新的 pattern/log，且尚未被合成處理過
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

/// 查詢既有高品質知識作為合成上下文
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
        Ok("（目前無既有高階知識）".to_string())
    } else {
        Ok(rows.join("\n---\n"))
    }
}

// ─── 寫入函數（安全優先）──────────────────────────────────────────────────

/// 處理 entity 更新：將 entity 資訊加到 chunk 的 tags 中（僅 additive）
async fn process_entities(
    pool: &SqlitePool,
    entities: &[EntityUpdate],
    valid_ids: &[String],
) -> usize {
    let mut count = 0;
    for e in entities {
        // 安全檢查：只處理本批次的 chunk
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

/// 處理 concept 更新：將 concept 資訊加到 chunk 的 tags 中（僅 additive）
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

/// 處理新合成結果：建立新 memory_chunk（knowledge_type = 'pattern'）
async fn process_syntheses(
    pool: &SqlitePool,
    syntheses: &[NewSynthesis],
    candidates: &[CandidateChunk],
) -> usize {
    let mut count = 0;
    for s in syntheses {
        // 驗證 synthesis 內容非空且有意義
        if s.synthesis.trim().len() < 10 {
            continue;
        }
        // 驗證 source_ids 都在候選列表中
        let all_valid = s
            .source_ids
            .iter()
            .all(|sid| candidates.iter().any(|c| c.id == *sid));
        if !all_valid || s.source_ids.is_empty() {
            continue;
        }

        // 取第一個 source 的 space_id 作為新 chunk 的 space
        let space_id = s.source_ids.first().and_then(|sid| {
            candidates
                .iter()
                .find(|c| c.id == *sid)
                .and_then(|c| c.space_id.clone())
        });

        let now = Utc::now().to_rfc3339();
        let chunk_id = Uuid::now_v7().to_string();

        // 建立 tags：標記來源 + 合成標記
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
                // 建立 extends 關係，連結到所有 source chunk
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
                eprintln!("[DeepSynthesis] 寫入合成 chunk 失敗: {}", e);
            }
        }
    }
    count
}

/// 處理矛盾檢測結果：寫入 chunk_relations
async fn process_contradictions(
    pool: &SqlitePool,
    contradictions: &[Contradiction],
    valid_ids: &[String],
) -> usize {
    let mut count = 0;
    for c in contradictions {
        // 只處理高信心度的矛盾
        if c.confidence < 0.7 {
            continue;
        }
        if c.chunk_ids.len() < 2 {
            continue;
        }
        // 驗證 chunk_ids 都在候選列表中
        if !c.chunk_ids.iter().all(|id| valid_ids.contains(id)) {
            continue;
        }

        // 為每對 chunk 建立矛盾關係
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
                    Err(e) => eprintln!("[DeepSynthesis] 寫入矛盾關係失敗: {}", e),
                }
            }
        }

        // 產生矛盾說明 Log chunk（若 description 有意義）
        if c.description.trim().len() >= 10 {
            let now = Utc::now().to_rfc3339();
            let chunk_id = Uuid::now_v7().to_string();
            let tags = serde_json::json!(["contradiction", "deep_synthesis"]);

            let _ = sqlx::query(
                "INSERT INTO memory_chunks (id, knowledge_type, content, tags, confidence, pending_confirm, placed_by, created_at, updated_at, last_synthesized_at)
                 VALUES (?, 'log', ?, ?, ?, 0, 'deep_synthesis', ?, ?, ?)",
            )
            .bind(&chunk_id)
            .bind(format!("⚠️ 矛盾檢測：{}", c.description.trim()))
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

// ─── 工具函數 ─────────────────────────────────────────────────────────────

/// 安全地向 chunk 的 tags JSON 陣列附加一個 tag（不重複）
async fn append_tag(pool: &SqlitePool, chunk_id: &str, new_tag: &str) -> Result<(), String> {
    // 讀取現有 tags
    let existing: String = sqlx::query_scalar("SELECT tags FROM memory_chunks WHERE id = ?")
        .bind(chunk_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "[]".to_string());

    let mut tags: Vec<String> = serde_json::from_str(&existing).unwrap_or_default();

    // 不重複添加
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

/// 寫入 chunk_relations（使用 INSERT OR IGNORE 避免重複）
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

/// 標記已處理的 chunk（更新 last_synthesized_at）
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
