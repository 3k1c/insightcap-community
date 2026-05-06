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

pub fn start_scheduler(app: AppHandle) {
    let shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        println!("[ConversationScheduler] Worker started");
        let mut shutdown_rx = shutdown_rx;
        let wakeup_rx = app.state::<AppState>().summary_wakeup_tx.clone();
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[ConversationScheduler] Stop signal received, exiting.");
                        break;
                    }
                }
                _ = wakeup_rx.notified() => {
                    if let Err(e) = process_next_summary(&app).await {
                        eprintln!("[ConversationScheduler] Processing failed: {}", e);
                    }
                }
                _ = sleep(Duration::from_secs(30)) => {
                    if let Err(e) = process_next_summary(&app).await {
                        eprintln!("[ConversationScheduler] Processing failed: {}", e);
                    }
                }
            }
        }
    });
}

pub async fn enqueue_conversation(
    pool: &sqlx::SqlitePool,
    conversation_id: &str,
    trigger_type: &str,
) -> Result<(), String> {
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

    println!(
        "[ConversationScheduler] Enqueued: {} ({})",
        conversation_id, trigger_type
    );
    Ok(())
}

async fn process_next_summary(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pool = &state.db;

    let row = sqlx::query(
        "SELECT id, conversation_id FROM conversation_summary_queue \
         WHERE status = 'pending' ORDER BY created_at ASC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let row = match row {
        Some(r) => r,
        None => return Ok(()), // No pending tasks
    };

    let queue_id: String = row.get("id");
    let conversation_id: String = row.get("conversation_id");

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE conversation_summary_queue SET status = 'processing', updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&queue_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let msgs = sqlx::query(
        "SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY created_at ASC",
    )
    .bind(&conversation_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if msgs.len() < 2 {
        mark_queue_done(pool, &queue_id, true).await?;
        return Ok(());
    }

    let mut dialogue = String::new();
    for m in &msgs {
        let role: String = m.get("role");
        let content: String = m.get("content");
        dialogue.push_str(&format!("{}: {}\n", role, content));
    }

    let settings = crate::settings::store::get_settings(pool)
        .await
        .map_err(|e| e.to_string())?;
    let ai_models = settings.ai_models;
    let summary_model = ai_models
        .summary_model
        .clone()
        .unwrap_or_else(|| "follow_chat".to_string());

    let (primary_cfg, fallback_cfg) = if summary_model == "follow_content_processor" {
        (
            ai_models.content_processor_llm.clone(),
            Some(ai_models.chat_llm.clone()),
        )
    } else {
        (
            ai_models.chat_llm.clone(),
            Some(ai_models.content_processor_llm.clone()),
        )
    };

    let api_key = primary_cfg.api_key.clone().unwrap_or_default();
    let is_ollama = primary_cfg.provider == "ollama";

    let existing_summary: Option<String> =
        sqlx::query_scalar("SELECT summary FROM conversations WHERE id = ?")
            .bind(&conversation_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None)
            .flatten()
            .filter(|s: &String| !s.trim().is_empty());

    let raw_summary = if !api_key.is_empty() || is_ollama {
        let prompt = if let Some(prev) = &existing_summary {
            format!(
                "Below are an existing summary and new messages from the same conversation.\n\
                 Integrate the new messages into the existing summary and output a single updated summary.\n\
                 Requirements:\n\
                 - Write in clear concise English\n\
                 - Preserve decisions, chosen technical approaches, issues, and conclusions\n\
                 - Keep concrete technical details (function names, tool names, error messages)\n\
                 - Target length: 150-250 words\n\
                 - Output summary only, no title or preface\n\n\
                 [Existing Summary]\n{}\n\n\
                 [New Messages]\n{}",
                prev, dialogue
            )
        } else {
            format!(
                "Generate a summary for the conversation below.\n\
                 Requirements:\n\
                 - Write in clear concise English\n\
                 - Preserve decisions, chosen technical approaches, issues, and conclusions\n\
                 - Keep concrete technical details (function names, tool names, error messages)\n\
                 - Target length: 150-250 words\n\
                 - Output summary only, no title or preface\n\n\
                 [Conversation]\n{}",
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
                                        "[ConversationScheduler] Primary model unavailable, switched to fallback: {}",
                                        cfg2.model
                                    );
                                    s
                                }
                                Err(e2) => {
                                    eprintln!(
                                        "[ConversationScheduler] Primary and fallback models both failed: {}; {}",
                                        primary_err, e2
                                    );
                                    dialogue.chars().take(300).collect::<String>()
                                }
                            }
                        } else {
                            eprintln!(
                                "[ConversationScheduler] Fallback model not configured, using local summary: {}",
                                primary_err
                            );
                            dialogue.chars().take(300).collect::<String>()
                        }
                    } else {
                        eprintln!(
                            "[ConversationScheduler] No fallback model, using local summary: {}",
                            primary_err
                        );
                        dialogue.chars().take(300).collect::<String>()
                    }
                } else {
                    eprintln!(
                        "[ConversationScheduler] LLM summarization failed: {}",
                        primary_err
                    );
                    dialogue.chars().take(300).collect::<String>()
                }
            }
        }
    } else {
        dialogue.chars().take(300).collect::<String>()
    };

    let summary = {
        let trimmed = raw_summary.trim();
        if !trimmed.is_empty() {
            trimmed.to_string()
        } else {
            let fallback = dialogue
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .take(8)
                .collect::<Vec<_>>()
                .join(" ");
            if fallback.is_empty() {
                "Summary unavailable".to_string()
            } else {
                fallback
            }
        }
    };

    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE conversations SET summary = ?, updated_at = ? WHERE id = ?")
        .bind(&summary)
        .bind(&now)
        .bind(&conversation_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    let project_id: Option<String> =
        sqlx::query_scalar("SELECT project_id FROM conversations WHERE id = ?")
            .bind(&conversation_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    let engine = MemoryEngine::new(
        pool.clone(),
        state.vector_store.clone(),
        state.embedder.clone(),
    );
    match engine
        .process_conversation_summary(&conversation_id, &summary, project_id.as_deref())
        .await
    {
        Ok(chunk_id) => {
            println!(
                "[ConversationScheduler] Conversation {} summary completed, memory_chunk: {}",
                conversation_id, chunk_id
            );
            let _ = app.emit(
                "summary-completed",
                serde_json::json!({
                    "conversationId": conversation_id,
                    "chunkId": chunk_id,
                }),
            );

            let pool_space = pool.clone();
            let embedder_space = state.embedder.clone();
            let vs_space = state.vector_store.clone();
            let summary_for_space = summary.clone();
            let chunk_id_for_space = chunk_id.clone();
            tauri::async_runtime::spawn(async move {
                let se = crate::services::space_engine::SpaceEngine::new(
                    pool_space,
                    embedder_space,
                    vs_space,
                );
                if let Err(e) = se
                    .assign_memory_chunk_to_space(&chunk_id_for_space, &summary_for_space)
                    .await
                {
                    eprintln!("[ConversationScheduler] Space assignment failed: {}", e);
                }
            });

            let pool_rel = pool.clone();
            let embedder_rel = state.embedder.clone();
            let vs_rel = state.vector_store.clone();
            let summary_clone = summary.clone();
            let chunk_id_clone = chunk_id.clone();
            tauri::async_runtime::spawn(async move {
                let rel_engine = crate::services::chunk_relation_engine::ChunkRelationEngine::new(
                    pool_rel,
                    embedder_rel,
                    vs_rel,
                );
                if let Err(e) = rel_engine
                    .analyze_and_link(&chunk_id_clone, "memory_chunk", &summary_clone)
                    .await
                {
                    eprintln!("[ChunkRelation] Analysis failed: {}", e);
                }
            });

            let pool_guide = pool.clone();
            let chunk_id_guide = chunk_id.clone();
            tauri::async_runtime::spawn(async move {
                let space_id: Option<String> =
                    sqlx::query_scalar("SELECT space_id FROM memory_chunks WHERE id = ?")
                        .bind(&chunk_id_guide)
                        .fetch_optional(&pool_guide)
                        .await
                        .ok()
                        .flatten();

                if let Some(sid) = space_id {
                    if !sid.is_empty() {
                        let guide_engine = crate::services::space_knowledge_guide_engine::SpaceKnowledgeGuideEngine::new(pool_guide);
                        if let Err(e) = guide_engine.generate_guide(&sid).await {
                            eprintln!(
                                "[SpaceKnowledgeGuide] Update failed (space {}): {}",
                                &sid, e
                            );
                        }
                    }
                }
            });

            let reminder_engine =
                crate::services::reminder_engine::ReminderEngine::new(pool.clone());
            if let Err(e) = reminder_engine
                .extract_reminders(
                    &conversation_id,
                    &summary,
                    &dialogue,
                    &now,
                    project_id.as_deref(),
                )
                .await
            {
                eprintln!("[ConversationScheduler] Reminder extraction failed: {}", e);
            }
        }
        Err(e) => {
            eprintln!("[ConversationScheduler] MemoryEngine failed: {}", e);
        }
    }

    mark_queue_done(pool, &queue_id, true).await?;
    Ok(())
}

async fn mark_queue_done(
    pool: &sqlx::SqlitePool,
    queue_id: &str,
    success: bool,
) -> Result<(), String> {
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
