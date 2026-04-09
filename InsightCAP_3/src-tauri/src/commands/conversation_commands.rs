use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tauri::State;
use uuid::Uuid;

use crate::background::conversation_scheduler::enqueue_conversation;
use crate::db::AppState;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::providers::llm::openai::OpenAiProvider;

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
    // 確保新欄位存在（自動遷移，容忍重複執行）
    let _ = sqlx::query("ALTER TABLE conversations ADD COLUMN is_pinned INTEGER DEFAULT 0").execute(pool.inner()).await;
    let _ = sqlx::query("ALTER TABLE conversations ADD COLUMN is_locked INTEGER DEFAULT 0").execute(pool.inner()).await;

    let rows = sqlx::query(
        "SELECT id, title, summary, project_id, is_pinned, is_locked, created_at, updated_at \
         FROM conversations WHERE is_archived = 0 \
         ORDER BY is_pinned DESC, updated_at DESC LIMIT 50"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let convs = rows.into_iter().map(|r| Conversation {
        id: r.get("id"),
        title: r.try_get("title").unwrap_or_default(),
        summary: r.try_get("summary").unwrap_or_default(),
        project_id: r.try_get("project_id").ok(),
        is_pinned: r.try_get::<i32, _>("is_pinned").unwrap_or(0) != 0,
        is_locked: r.try_get::<i32, _>("is_locked").unwrap_or(0) != 0,
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }).collect();

    Ok(convs)
}

#[tauri::command]
pub async fn create_conversation(
    pool: State<'_, SqlitePool>,
    project_id: Option<String>,
) -> Result<String, String> {
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO conversations (id, title, project_id, created_at, updated_at) VALUES (?, '新對話', ?, ?, ?)"
    )
    .bind(&id)
    .bind(&project_id)
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
        "SELECT id, role, content, metadata, created_at FROM messages WHERE conversation_id = ? ORDER BY created_at ASC"
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
        metadata: r.try_get("metadata").unwrap_or_else(|_| "{}".to_string()),
    }).collect();

    Ok(msgs)
}

#[tauri::command]
pub async fn add_message(
    pool: State<'_, SqlitePool>,
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
    app_state: State<'_, AppState>,
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
        opt_provider = Some(OpenAiProvider::new(api_key, cfg.base_url, cfg.model, cfg.provider.clone()));
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

    let engine = crate::services::memory_engine::MemoryEngine::new(
        pool.inner().clone(),
        app_state.vector_store.clone(),
        app_state.embedder.clone(),
    );
    engine.process_conversation_summary(&conversation_id, &summary, None).await?;

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
    // 鎖定中的對話禁止刪除
    let locked: i32 = sqlx::query_scalar("SELECT COALESCE(is_locked, 0) FROM conversations WHERE id = ?")
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

/// 自動為對話生成標題（根據前 2 條訊息，由 LLM 產出）
#[tauri::command]
pub async fn auto_title_conversation(
    pool: State<'_, SqlitePool>,
    conversation_id: String,
) -> Result<String, String> {
    // 1. 檢查是否仍為預設標題「新對話」
    let current_title: String = sqlx::query_scalar(
        "SELECT title FROM conversations WHERE id = ?"
    )
    .bind(&conversation_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .unwrap_or_default();

    if current_title != "新對話" {
        return Ok(current_title); // 已手動改名，不覆蓋
    }

    // 2. 取前 4 條訊息（最多 2 輪對話）
    let msgs = sqlx::query(
        "SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY created_at ASC LIMIT 4"
    )
    .bind(&conversation_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    if msgs.len() < 2 {
        return Ok(current_title); // 訊息不足，不生成
    }

    let mut dialogue = String::new();
    for m in &msgs {
        let role: String = m.get("role");
        let content: String = m.get("content");
        // 每條訊息最多取前 300 字，避免 token 浪費
        let truncated: String = content.chars().take(300).collect();
        dialogue.push_str(&format!("{}: {}\n", role, truncated));
    }

    // 3. 呼叫 LLM 生成標題
    let settings = crate::settings::store::get_settings(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    let cfg = settings.ai_models.chat_llm;
    let api_key = cfg.api_key.clone().unwrap_or_default();
    let is_ollama = cfg.provider == "ollama";

    let title = if !api_key.is_empty() || is_ollama {
        let prompt = format!(
            "{}\n\n【對話內容】\n{}",
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
                LLMOptions { temperature: 0.3, max_tokens: 60, stream: false, think_mode: None },
            ),
        )
        .await
        {
            Ok(Ok(raw)) => {
                // 清理：去掉引號、換行、多餘空白
                let cleaned = raw.trim()
                    .trim_matches(|c| c == '"' || c == '「' || c == '」' || c == '\'' )
                    .trim()
                    .to_string();
                if cleaned.is_empty() { current_title } else { cleaned }
            }
            _ => current_title.clone(),
        }
    } else {
        // 無 LLM：從第一條 user 訊息截取前 20 字
        let first_content: String = msgs[0].get("content");
        let fallback: String = first_content.chars().take(20).collect();
        if fallback.is_empty() { current_title } else { fallback }
    };

    // 4. 寫回 DB
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

/// 前端對話切換 / 關閉時呼叫，將對話加入總結佇列
#[tauri::command]
pub async fn enqueue_summary(
    pool: State<'_, SqlitePool>,
    conversation_id: String,
    trigger_type: Option<String>,
) -> Result<(), String> {
    let trigger = trigger_type.as_deref().unwrap_or("switch");
    enqueue_conversation(pool.inner(), &conversation_id, trigger).await
}
