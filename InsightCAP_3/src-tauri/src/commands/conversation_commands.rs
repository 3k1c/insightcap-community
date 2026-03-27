use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tauri::State;
use uuid::Uuid;

use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::providers::llm::openai::OpenAiProvider;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[tauri::command]
pub async fn get_conversations(pool: State<'_, SqlitePool>) -> Result<Vec<Conversation>, String> {
    let rows = sqlx::query(
        "SELECT id, title, summary, created_at, updated_at FROM conversations WHERE is_archived = 0 ORDER BY updated_at DESC LIMIT 50"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let convs = rows.into_iter().map(|r| Conversation {
        id: r.get("id"),
        title: r.try_get("title").unwrap_or_default(),
        summary: r.try_get("summary").unwrap_or_default(),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }).collect();

    Ok(convs)
}

#[tauri::command]
pub async fn create_conversation(pool: State<'_, SqlitePool>) -> Result<String, String> {
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?, '新對話', ?, ?)"
    )
    .bind(&id)
    .bind(&now)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub async fn get_messages(pool: State<'_, SqlitePool>, conversation_id: String) -> Result<Vec<Message>, String> {
    let rows = sqlx::query(
        "SELECT id, role, content, created_at FROM messages WHERE conversation_id = ? ORDER BY created_at ASC"
    )
    .bind(conversation_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let msgs = rows.into_iter().map(|r| Message {
        id: r.get("id"),
        role: r.get("role"),
        content: r.get("content"),
        created_at: r.get("created_at"),
    }).collect();

    Ok(msgs)
}

#[tauri::command]
pub async fn add_message(
    pool: State<'_, SqlitePool>,
    conversation_id: String,
    role: String,
    content: String,
) -> Result<String, String> {
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&conversation_id)
    .bind(&role)
    .bind(&content)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "UPDATE conversations SET updated_at = ?, message_count = message_count + 1 WHERE id = ?"
    )
    .bind(&now)
    .bind(&conversation_id)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub async fn summarize_conversation(
    pool: State<'_, SqlitePool>,
    conversation_id: String
) -> Result<String, String> {
    let msgs = get_messages(pool.clone(), conversation_id.clone()).await?;
    if msgs.len() < 2 {
        return Ok("Not enough messages to summarize".to_string());
    }

    let mut text = String::new();
    for m in &msgs {
        text.push_str(&format!("{}: {}\n", m.role, m.content));
    }

    let settings = crate::settings::store::get_settings(pool.inner()).await.map_err(|e| e.to_string())?;
    // 優先使用 summary_model，若未設定則使用 chat_llm
    let cfg = settings.ai_models.chat_llm;
    
    let mut opt_provider: Option<OpenAiProvider> = None;
    let is_ollama = cfg.provider == "ollama";
    let api_key = cfg.api_key.unwrap_or_default();

    if !api_key.is_empty() || is_ollama {
        opt_provider = Some(OpenAiProvider::new(api_key, cfg.base_url, cfg.model));
    }

    let summary = if let Some(llm) = opt_provider {
        let prompt = format!("Summarize the following conversation in 2-3 sentences. Identify key patterns or tasks.\n\n{}", text);
        llm.complete(&prompt, LLMOptions::default()).await.map_err(|e| e.to_string())?
    } else {
        "Simulated conversation summary (No LLM config)".to_string()
    };

    sqlx::query("UPDATE conversations SET summary = ?, updated_at = ? WHERE id = ?")
        .bind(&summary)
        .bind(Utc::now().to_rfc3339())
        .bind(&conversation_id)
        .execute(pool.inner()).await.map_err(|e| e.to_string())?;

    let engine = crate::services::memory_engine::MemoryEngine::new(pool.inner().clone());
    engine.process_conversation_summary(&conversation_id, &summary).await?;

    Ok(summary)
}
