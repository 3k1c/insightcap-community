use std::time::Duration;

use chrono::Utc;
use sqlx::Row;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::sleep;
use uuid::Uuid;

use crate::db::AppState;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::services::memory_engine::MemoryEngine;

fn is_model_not_found_error(err: &str) -> bool {
    let s = err.to_lowercase();
    s.contains("404") && s.contains("not found") && s.contains("model")
}

async fn call_summary_llm(
    cfg: &crate::settings::store::ModelSettings,
    prompt: &str,
) -> Result<String, String> {
    let provider = OpenAiProvider::new(
        cfg.api_key.clone().unwrap_or_default(),
        cfg.base_url.clone(),
        cfg.model.clone(),
        cfg.provider.clone(),
    );

    tokio::time::timeout(
        Duration::from_secs(45),
        provider.complete(
            prompt,
            LLMOptions {
                temperature: 0.2,
                max_tokens: 600,
                stream: false,
                think_mode: None,
            },
        ),
    )
    .await
    .map_err(|_| "LLM summary timeout".to_string())?
    .map_err(|e| e.to_string())
}

/// 啟動對話總結排程 worker
/// - 每 30 秒輪詢 conversation_summary_queue
/// - 取出 pending 任務 → 生成摘要 → 呼叫 MemoryEngine 深度推斷 knowledge_type
pub fn start_scheduler(app: AppHandle) {
    let shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        println!("[ConversationScheduler] Worker 啟動");
        let mut shutdown_rx = shutdown_rx;
        let wakeup_rx = app.state::<AppState>().summary_wakeup_tx.clone();
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[ConversationScheduler] 收到停止訊號，退出。");
                        break;
                    }
                }
                _ = wakeup_rx.notified() => {
                    if let Err(e) = process_next_summary(&app).await {
                        eprintln!("[ConversationScheduler] 處理失敗: {}", e);
                    }
                }
                _ = sleep(Duration::from_secs(30)) => {
                    if let Err(e) = process_next_summary(&app).await {
                        eprintln!("[ConversationScheduler] 處理失敗: {}", e);
                    }
                }
            }
        }
    });
}

/// 前端通知：將對話加入總結佇列（對話切換 / 關閉時呼叫）
pub async fn enqueue_conversation(pool: &sqlx::SqlitePool, conversation_id: &str, trigger_type: &str) -> Result<(), String> {
    // 若已有 pending 任務，不重複加入
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM conversation_summary_queue WHERE conversation_id = ? AND status = 'pending'"
    )
    .bind(conversation_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Ok(());
    }

    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO conversation_summary_queue (id, conversation_id, trigger_type, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'pending', ?, ?)"
    )
    .bind(&id)
    .bind(conversation_id)
    .bind(trigger_type)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    println!("[ConversationScheduler] 已加入佇列: {} ({})", conversation_id, trigger_type);
    Ok(())
}

// ─── 內部：處理下一筆待總結任務 ─────────────────────────────────────────────

async fn process_next_summary(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    // 1. 取出最舊的 pending 任務
    let row = sqlx::query(
        "SELECT id, conversation_id FROM conversation_summary_queue \
         WHERE status = 'pending' ORDER BY created_at ASC LIMIT 1"
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let row = match row {
        Some(r) => r,
        None => return Ok(()), // 無待處理任務
    };

    let queue_id: String = row.get("id");
    let conversation_id: String = row.get("conversation_id");

    // 2. 標記處理中
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE conversation_summary_queue SET status = 'processing', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&queue_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // 3. 取得對話訊息
    let msgs = sqlx::query(
        "SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY created_at ASC"
    )
    .bind(&conversation_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if msgs.len() < 2 {
        // 訊息太少（至少需要 1 輪對話），直接標記完成
        mark_queue_done(pool, &queue_id, true).await?;
        return Ok(());
    }

    // 4. 組裝對話文字
    let mut dialogue = String::new();
    for m in &msgs {
        let role: String = m.get("role");
        let content: String = m.get("content");
        dialogue.push_str(&format!("{}: {}\n", role, content));
    }

    // 5. 呼叫 LLM 生成摘要（增量：若已有摘要，只更新新增部分）
    let settings = crate::settings::store::get_settings(pool)
        .await
        .map_err(|e| e.to_string())?;
    let ai_models = settings.ai_models;
    let summary_model = ai_models
        .summary_model
        .clone()
        .unwrap_or_else(|| "follow_chat".to_string());

    let (primary_cfg, fallback_cfg) = if summary_model == "follow_content_processor" {
        (ai_models.content_processor_llm.clone(), Some(ai_models.chat_llm.clone()))
    } else {
        (ai_models.chat_llm.clone(), Some(ai_models.content_processor_llm.clone()))
    };

    let api_key = primary_cfg.api_key.clone().unwrap_or_default();
    let is_ollama = primary_cfg.provider == "ollama";

    // 取得現有摘要（若有，用增量模式）
    let existing_summary: Option<String> = sqlx::query_scalar(
        "SELECT summary FROM conversations WHERE id = ?"
    )
    .bind(&conversation_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
    .flatten()
    .filter(|s: &String| !s.trim().is_empty());

    let summary = if !api_key.is_empty() || is_ollama {
        let prompt = if let Some(prev) = &existing_summary {
            format!(
                "以下是一段對話的【舊有摘要】和【新增訊息】。\n\
                 請將新增訊息整合進舊有摘要，輸出一份更新後的完整摘要。\n\
                 要求：\n\
                 - 用繁體中文撰寫，技術名詞保留英文\n\
                 - 保留所有決策、使用的技術方案、遇到的問題和結論\n\
                 - 保留具體的技術細節（如函式名稱、工具名稱、錯誤訊息）\n\
                 - 長度控制在 150-250 字\n\
                 - 只輸出摘要本身，不要加標題或前言\n\n\
                 【舊有摘要】\n{}\n\n\
                 【新增訊息】\n{}",
                prev, dialogue
            )
        } else {
            format!(
                "請為以下對話生成摘要。\n\
                 要求：\n\
                 - 用繁體中文撰寫，技術名詞保留英文\n\
                 - 保留所有決策、使用的技術方案、遇到的問題和結論\n\
                 - 保留具體的技術細節（如函式名稱、工具名稱、錯誤訊息）\n\
                 - 長度控制在 150-250 字\n\
                 - 只輸出摘要本身，不要加標題或前言\n\n\
                 【對話內容】\n{}",
                dialogue
            )
        };

        match call_summary_llm(&primary_cfg, &prompt).await {
            Ok(s) => s,
            Err(primary_err) => {
                if primary_cfg.provider == "ollama" && is_model_not_found_error(&primary_err) {
                    if let Some(cfg2) = &fallback_cfg {
                        let api_key2 = cfg2.api_key.clone().unwrap_or_default();
                        let is_ollama2 = cfg2.provider == "ollama";
                        if !api_key2.is_empty() || is_ollama2 {
                            match call_summary_llm(cfg2, &prompt).await {
                                Ok(s) => {
                                    println!(
                                        "[ConversationScheduler] 主模型不可用，已改用備援模型: {}",
                                        cfg2.model
                                    );
                                    s
                                }
                                Err(e2) => {
                                    eprintln!(
                                        "[ConversationScheduler] 主/備援模型皆失敗: {}; {}",
                                        primary_err, e2
                                    );
                                    dialogue.chars().take(300).collect::<String>()
                                }
                            }
                        } else {
                            eprintln!(
                                "[ConversationScheduler] 備援模型未配置，改用本地摘要: {}",
                                primary_err
                            );
                            dialogue.chars().take(300).collect::<String>()
                        }
                    } else {
                        eprintln!(
                            "[ConversationScheduler] 無備援模型，改用本地摘要: {}",
                            primary_err
                        );
                        dialogue.chars().take(300).collect::<String>()
                    }
                } else {
                    eprintln!("[ConversationScheduler] LLM 摘要失敗: {}", primary_err);
                    dialogue.chars().take(300).collect::<String>()
                }
            }
        }
    } else {
        // 無 LLM：用前 300 字作為摘要
        dialogue.chars().take(300).collect::<String>()
    };

    // 6. 寫回 conversations.summary
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE conversations SET summary = ?, updated_at = ? WHERE id = ?")
        .bind(&summary)
        .bind(&now)
        .bind(&conversation_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // 7. 取得對話的 project_id
    let project_id: Option<String> = sqlx::query_scalar(
        "SELECT project_id FROM conversations WHERE id = ?"
    )
    .bind(&conversation_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    // 8. 呼叫 MemoryEngine 深度推斷 knowledge_type → 寫入 memory_chunks
    let engine = MemoryEngine::new(
        pool.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );
    match engine.process_conversation_summary(
        &conversation_id,
        &summary,
        project_id.as_deref(),
    )
    .await
    {
        Ok(chunk_id) => {
            println!(
                "[ConversationScheduler] 對話 {} 總結完成，memory_chunk: {}",
                conversation_id, chunk_id
            );
            // 通知前端摘要完成（觸發 pending confirmation toast）
            let _ = app.emit("summary-completed", serde_json::json!({
                "conversationId": conversation_id,
                "chunkId": chunk_id,
            }));

            // 非同步為 memory_chunk 分配 Space（向量相似度，不呼叫 LLM）
            let pool_space = pool.clone();
            let embedder_space = state.embedder.clone();
            let vs_space = state.vector_store.clone();
            let summary_for_space = summary.clone();
            let chunk_id_for_space = chunk_id.clone();
            tauri::async_runtime::spawn(async move {
                let se = crate::services::space_engine::SpaceEngine::new(
                    pool_space, embedder_space, vs_space,
                );
                if let Err(e) = se.assign_memory_chunk_to_space(&chunk_id_for_space, &summary_for_space).await {
                    eprintln!("[ConversationScheduler] Space 分配失敗: {}", e);
                }
            });

            // 非同步分析反向鏈接（不阻塞主流程）
            let pool_rel = pool.clone();
            let embedder_rel = state.embedder.clone();
            let vs_rel = state.vector_store.clone();
            let summary_clone = summary.clone();
            let chunk_id_clone = chunk_id.clone();
            tauri::async_runtime::spawn(async move {
                let rel_engine = crate::services::chunk_relation_engine::ChunkRelationEngine::new(
                    pool_rel, embedder_rel, vs_rel,
                );
                if let Err(e) = rel_engine.analyze_and_link(&chunk_id_clone, "memory_chunk", &summary_clone).await {
                    eprintln!("[ChunkRelation] 分析失敗: {}", e);
                }
            });

            // 非同步更新 Space Wiki（不阻塞主流程）
            let pool_wiki = pool.clone();
            let chunk_id_wiki = chunk_id.clone();
            tauri::async_runtime::spawn(async move {
                // 查 memory_chunk 的 space_id
                let space_id: Option<String> = sqlx::query_scalar(
                    "SELECT space_id FROM memory_chunks WHERE id = ?"
                )
                .bind(&chunk_id_wiki)
                .fetch_optional(&pool_wiki)
                .await
                .ok()
                .flatten();

                if let Some(sid) = space_id {
                    if !sid.is_empty() {
                        let wiki_engine = crate::services::space_wiki_engine::SpaceWikiEngine::new(pool_wiki);
                        if let Err(e) = wiki_engine.update_wiki_for_space(&sid).await {
                            eprintln!("[SpaceWiki] 更新失敗 (space {}): {}", &sid, e);
                        }
                    }
                }
            });

            // --- 新增：從對話與摘要中提取提醒事項 ---
            let reminder_engine = crate::services::reminder_engine::ReminderEngine::new(pool.clone());
            if let Err(e) = reminder_engine.extract_reminders(
                &conversation_id,
                &summary,
                &dialogue,
                &now,
                project_id.as_deref()
            ).await {
                eprintln!("[ConversationScheduler] 提醒提取失敗: {}", e);
            }
        }
        Err(e) => {
            eprintln!("[ConversationScheduler] MemoryEngine 失敗: {}", e);
        }
    }

    // 9. 標記任務完成
    mark_queue_done(pool, &queue_id, true).await?;
    Ok(())
}

async fn mark_queue_done(pool: &sqlx::SqlitePool, queue_id: &str, success: bool) -> Result<(), String> {
    let status = if success { "done" } else { "failed" };
    sqlx::query("UPDATE conversation_summary_queue SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(Utc::now().to_rfc3339())
        .bind(queue_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
