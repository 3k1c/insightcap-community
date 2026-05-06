use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tauri::State;
use uuid::Uuid;

use crate::background::conversation_scheduler::enqueue_conversation;
use crate::db::AppState;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};

fn default_conversation_title(language: &str) -> &'static str {
    match language {
        "zh-CN" => "新对话",
        "en" => "New Conversation",
        _ => "新對話",
    }
}

fn is_default_conversation_title(title: &str) -> bool {
    matches!(
        title,
        "Untitled Conversation" | "New Conversation" | "新對話" | "新对话"
    )
}

fn display_conversation_title(title: String, language: &str) -> String {
    if title.trim().is_empty() || title == "Untitled Conversation" {
        default_conversation_title(language).to_string()
    } else {
        title
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub project_id: Option<String>,
    pub is_pinned: bool,
    pub is_locked: bool,
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
    pub metadata: String,
}

#[tauri::command]
pub async fn get_conversations(pool: State<'_, SqlitePool>) -> Result<Vec<Conversation>, String> {
    let _ = sqlx::query("ALTER TABLE conversations ADD COLUMN is_pinned INTEGER DEFAULT 0")
        .execute(pool.inner())
        .await;
    let _ = sqlx::query("ALTER TABLE conversations ADD COLUMN is_locked INTEGER DEFAULT 0")
        .execute(pool.inner())
        .await;

    let rows = sqlx::query(
        "SELECT id, title, summary, project_id, is_pinned, is_locked, created_at, updated_at \
         FROM conversations WHERE is_archived = 0 \
         ORDER BY is_pinned DESC, updated_at DESC LIMIT 50",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let settings = crate::settings::store::get_settings(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    let language = settings.general.language;

    let convs = rows
        .into_iter()
        .map(|r| Conversation {
            id: r.get("id"),
            title: display_conversation_title(r.try_get("title").unwrap_or_default(), &language),
            summary: r.try_get("summary").unwrap_or_default(),
            project_id: r.try_get("project_id").ok(),
            is_pinned: r.try_get::<i32, _>("is_pinned").unwrap_or(0) != 0,
            is_locked: r.try_get::<i32, _>("is_locked").unwrap_or(0) != 0,
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

    Ok(convs)
}

#[tauri::command]
pub async fn create_conversation(
    pool: State<'_, SqlitePool>,
    project_id: Option<String>,
) -> Result<String, String> {
    let id = Uuid::now_v7().to_string();
    let settings = crate::settings::store::get_settings(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    let title = default_conversation_title(&settings.general.language);
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO conversations (id, title, project_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(title)
    .bind(&project_id)
    .bind(&now)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub async fn get_messages(
    pool: State<'_, SqlitePool>,
    conversation_id: String,
) -> Result<Vec<Message>, String> {
    let rows = sqlx::query(
        "SELECT id, role, content, metadata, created_at FROM messages WHERE conversation_id = ? ORDER BY created_at ASC"
    )
    .bind(conversation_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let msgs = rows
        .into_iter()
        .map(|r| Message {
            id: r.get("id"),
            role: r.get("role"),
            content: r.get("content"),
            created_at: r.get("created_at"),
            metadata: r.try_get("metadata").unwrap_or_else(|_| "{}".to_string()),
        })
        .collect();

    Ok(msgs)
}

#[tauri::command]
pub async fn add_message(
    pool: State<'_, SqlitePool>,
    app_state: State<'_, AppState>,
    conversation_id: String,
    role: String,
    content: String,
    metadata: Option<String>,
) -> Result<String, String> {
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let meta = metadata.unwrap_or_else(|| "{}".to_string());

    sqlx::query(
        "INSERT INTO messages (id, conversation_id, role, content, metadata, created_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&conversation_id)
    .bind(&role)
    .bind(&content)
    .bind(&meta)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "UPDATE conversations SET updated_at = ?, message_count = message_count + 1 WHERE id = ?",
    )
    .bind(&now)
    .bind(&conversation_id)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    if role == "assistant" {
        let _ = crate::background::conversation_scheduler::enqueue_conversation(
            pool.inner(),
            &conversation_id,
            "new_message",
        )
        .await;
        app_state.summary_wakeup_tx.notify_one();
    }

    Ok(id)
}

#[tauri::command]
pub async fn summarize_conversation(
    pool: State<'_, SqlitePool>,
    app_state: State<'_, AppState>,
    conversation_id: String,
) -> Result<String, String> {
    let msgs = get_messages(pool.clone(), conversation_id.clone()).await?;
    if msgs.len() < 2 {
        return Ok("Not enough messages to summarize".to_string());
    }

    let mut text = String::new();
    for m in &msgs {
        text.push_str(&format!("{}: {}\n", m.role, m.content));
    }

    let settings = crate::settings::store::get_settings(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    let cfg = settings.ai_models.chat_llm;

    let mut opt_provider: Option<OpenAiProvider> = None;
    let is_ollama = cfg.provider == "ollama";
    let api_key = cfg.api_key.unwrap_or_default();

    if !api_key.is_empty() || is_ollama {
        opt_provider = Some(OpenAiProvider::new(
            api_key,
            cfg.base_url,
            cfg.model,
            cfg.provider.clone(),
        ));
    }

    let summary = if let Some(llm) = opt_provider {
        let prompt = format!("Summarize the following conversation in 2-3 sentences. Identify key patterns or tasks.\n\n{}", text);
        llm.complete(&prompt, LLMOptions::default())
            .await
            .map_err(|e| e.to_string())?
    } else {
        "Simulated conversation summary (No LLM config)".to_string()
    };

    sqlx::query("UPDATE conversations SET summary = ?, updated_at = ? WHERE id = ?")
        .bind(&summary)
        .bind(Utc::now().to_rfc3339())
        .bind(&conversation_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let engine = crate::services::memory_engine::MemoryEngine::new(
        pool.inner().clone(),
        app_state.vector_store.clone(),
        app_state.embedder.clone(),
    );
    engine
        .process_conversation_summary(&conversation_id, &summary, None)
        .await?;

    Ok(summary)
}

#[tauri::command]
pub async fn rename_conversation(
    pool: State<'_, SqlitePool>,
    conversation_id: String,
    title: String,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE conversations SET title = ?, updated_at = ? WHERE id = ?")
        .bind(&title)
        .bind(&now)
        .bind(&conversation_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_conversation(
    pool: State<'_, SqlitePool>,
    conversation_id: String,
) -> Result<(), String> {
    let locked: i32 =
        sqlx::query_scalar("SELECT COALESCE(is_locked, 0) FROM conversations WHERE id = ?")
            .bind(&conversation_id)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or(0);
    if locked != 0 {
        return Err("conversation_locked".to_string());
    }

    sqlx::query("DELETE FROM conversations WHERE id = ?")
        .bind(&conversation_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn update_conversation(
    pool: State<'_, SqlitePool>,
    conversation_id: String,
    is_pinned: Option<bool>,
    is_locked: Option<bool>,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    if let Some(pinned) = is_pinned {
        sqlx::query("UPDATE conversations SET is_pinned = ?, updated_at = ? WHERE id = ?")
            .bind(pinned as i32)
            .bind(&now)
            .bind(&conversation_id)
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(locked) = is_locked {
        sqlx::query("UPDATE conversations SET is_locked = ?, updated_at = ? WHERE id = ?")
            .bind(locked as i32)
            .bind(&now)
            .bind(&conversation_id)
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn auto_title_conversation(
    pool: State<'_, SqlitePool>,
    conversation_id: String,
) -> Result<String, String> {
    let current_title: String = sqlx::query_scalar("SELECT title FROM conversations WHERE id = ?")
        .bind(&conversation_id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    if !is_default_conversation_title(&current_title) {
        return Ok(current_title);
    }

    let msgs = sqlx::query(
        "SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY created_at ASC LIMIT 4",
    )
    .bind(&conversation_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    if msgs.len() < 2 {
        return Ok(current_title);
    }

    let mut dialogue = String::new();
    for m in &msgs {
        let role: String = m.get("role");
        let content: String = m.get("content");
        let truncated: String = content.chars().take(300).collect();
        dialogue.push_str(&format!("{role}: {truncated}\n"));
    }

    let settings = crate::settings::store::get_settings(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    let cfg = settings.ai_models.chat_llm;
    let api_key = cfg.api_key.clone().unwrap_or_default();
    let is_ollama = cfg.provider == "ollama";

    let title = if !api_key.is_empty() || is_ollama {
        let prompt = format!(
            "{}\n\nPlease generate a concise conversation title.\n{}",
            crate::prompts::AUTO_TITLE_SYSTEM,
            dialogue
        );

        let provider = OpenAiProvider::new(
            api_key,
            cfg.base_url.clone(),
            cfg.model.clone(),
            cfg.provider.clone(),
        );

        match tokio::time::timeout(
            std::time::Duration::from_secs(15),
            provider.complete(
                &prompt,
                LLMOptions {
                    temperature: 0.3,
                    max_tokens: 60,
                    stream: false,
                    think_mode: None,
                },
            ),
        )
        .await
        {
            Ok(Ok(raw)) => {
                let cleaned = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim();
                if cleaned.is_empty() {
                    current_title.clone()
                } else {
                    cleaned.to_string()
                }
            }
            _ => current_title.clone(),
        }
    } else {
        let first_content: String = msgs[0].get("content");
        let fallback: String = first_content.chars().take(20).collect();
        if fallback.is_empty() {
            current_title.clone()
        } else {
            fallback
        }
    };

    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE conversations SET title = ?, updated_at = ? WHERE id = ?")
        .bind(&title)
        .bind(&now)
        .bind(&conversation_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(title)
}

#[tauri::command]
pub async fn enqueue_summary(
    pool: State<'_, SqlitePool>,
    conversation_id: String,
    trigger_type: Option<String>,
) -> Result<(), String> {
    let trigger = trigger_type.as_deref().unwrap_or("switch");
    enqueue_conversation(pool.inner(), &conversation_id, trigger).await
}

#[tauri::command]
pub async fn decide_reminder_ack(
    pool: State<'_, SqlitePool>,
    user_message: String,
    recent_assistant_context: String,
    current_answer: String,
) -> Result<Option<String>, String> {
    let settings = crate::settings::store::get_settings(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    let cfg = settings.ai_models.chat_llm;
    let api_key = cfg.api_key.clone().unwrap_or_default();
    let is_ollama = cfg.provider == "ollama";

    if api_key.is_empty() && !is_ollama {
        return Ok(None);
    }

    let provider = OpenAiProvider::new(
        api_key,
        cfg.base_url.clone(),
        cfg.model.clone(),
        cfg.provider.clone(),
    );

    let prompt = format!(
        "You are a dialogue policy checker.\n\
Decide whether the assistant should append ONE extra sentence confirming reminder setup.\n\
Return a JSON object only, with this exact schema:\n\
{{\"append\": boolean, \"message\": string}}\n\n\
Rules:\n\
1) append=true only if user message means 'no further help needed / thanks'.\n\
2) append=true only if recent assistant context indicates reminder/schedule was already created.\n\
3) If current answer already confirms reminder setup, set append=false.\n\
4) If append=true, message must be short, natural, and in the same language as user message.\n\
5) If append=false, message must be an empty string.\n\
6) Keep factual: confirm reminder is set, do not add new details.\n\n\
User message:\n{user}\n\n\
Recent assistant context:\n{ctx}\n\n\
Current answer:\n{ans}\n",
        user = user_message,
        ctx = recent_assistant_context,
        ans = current_answer
    );

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        provider.complete_json(
            &prompt,
            LLMOptions {
                temperature: 0.0,
                max_tokens: 60,
                stream: false,
                think_mode: Some(false),
            },
        ),
    )
    .await;

    let parsed = match result {
        Ok(Ok(v)) => v,
        _ => return Ok(None),
    };

    let append = parsed
        .get("append")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let message = parsed
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string();

    if !append {
        return Ok(None);
    }
    if message.is_empty() {
        return Ok(None);
    }

    Ok(Some(message))
}
