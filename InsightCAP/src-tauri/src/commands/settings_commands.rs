use crate::settings::store::{self, AllSettings};
use crate::providers::llm::model_caps;
use reqwest::Client;
use sqlx::SqlitePool;
use tauri::{Manager, State};

#[tauri::command]
pub async fn get_settings(pool: State<'_, SqlitePool>) -> Result<AllSettings, String> {
    store::get_settings(pool.inner())
        .await
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub async fn save_settings(
    handle: tauri::AppHandle,
    settings: AllSettings,
    pool: State<'_, SqlitePool>,
) -> Result<(), String> {
    // 儲存至 DB
    store::save_settings(pool.inner(), settings.clone())
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    // 更新 bootstrap.json
    let app_data_dir = handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Path error: {}", e))?;

    crate::db::connection::write_bootstrap(
        &app_data_dir,
        &settings.knowledge.kb_path,
    )?;

    Ok(())
}

/// 首次啟動：在指定路徑建立工作區目錄結構
#[tauri::command]
pub async fn initialize_workspace(handle: tauri::AppHandle, path: String) -> Result<(), String> {
    let new_path = std::path::PathBuf::from(&path);

    for sub in &[".insightcap", "notes", "inbox", "attachments"] {
        std::fs::create_dir_all(new_path.join(sub))
            .map_err(|e| format!("無法建立目錄 {}: {}", sub, e))?;
    }

    let app_data_dir = handle.path().app_data_dir().map_err(|e| e.to_string())?;
    crate::db::connection::write_bootstrap(&app_data_dir, &path)?;

    println!("[INIT] Workspace initialized at: {}", path);
    handle.restart();
}

/// 切換工作區路徑
#[tauri::command]
pub async fn switch_kb_path(handle: tauri::AppHandle, new_path: String) -> Result<(), String> {
    let app_data_dir = handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Path error: {}", e))?;

    crate::db::connection::write_bootstrap(&app_data_dir, &new_path)?;
    println!("[KB-SWITCH] bootstrap.json updated to: {}", new_path);

    handle.restart();
}

/// 測試 Ollama 連線
#[tauri::command]
pub async fn test_ollama(base_url: String) -> Result<bool, String> {
    let url = if base_url.trim().is_empty() {
        "http://localhost:11434".to_string()
    } else {
        base_url
    };
    let tags_url = format!("{}/api/tags", url.trim_end_matches('/'));
    let client = Client::new();
    match client
        .get(&tags_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(r) => Ok(r.status().is_success()),
        Err(_) => Ok(false),
    }
}

/// 測試任意 AI 供應商連線
#[tauri::command]
pub async fn test_provider_connection(
    provider: String,
    base_url: String,
    api_key: String,
) -> Result<bool, String> {
    let client = Client::new();
    let timeout = std::time::Duration::from_secs(8);

    match provider.as_str() {
        "ollama" => {
            let url = if base_url.trim().is_empty() {
                "http://localhost:11434".to_string()
            } else {
                base_url
            };
            let tags_url = format!("{}/api/tags", url.trim_end_matches('/'));
            match client.get(&tags_url).timeout(timeout).send().await {
                Ok(r) => Ok(r.status().is_success()),
                Err(_) => Ok(false),
            }
        }
        _ => {
            let base = base_url.trim_end_matches('/');
            let models_url = if base.ends_with("/v1") || base.contains("/v1/") {
                format!("{}/models", base)
            } else {
                format!("{}/v1/models", base)
            };
            let mut req = client.get(&models_url).timeout(timeout);
            if !api_key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", api_key));
            }
            match req.send().await {
                Ok(r) => Ok(r.status().is_success()),
                Err(_) => Ok(false),
            }
        }
    }
}

/// 測試指定模型的對話連線
#[tauri::command]
pub async fn test_model_connection(
    provider: String,
    model: String,
    base_url: Option<String>,
    api_key: Option<String>,
) -> Result<String, String> {
    use crate::providers::llm::openai::OpenAiProvider;
    use crate::providers::llm::{LLMProvider, LLMOptions};

    let base_url_str = base_url.unwrap_or_default();
    let api_key_str = api_key.unwrap_or_default();

    let ai_provider = OpenAiProvider::new(api_key_str, Some(base_url_str), model, provider);
    let options = LLMOptions { max_tokens: 20, stream: false, temperature: 0.1, think_mode: None };
    
    match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        ai_provider.complete("Hi", options)
    ).await {
        Ok(Ok(res)) => Ok(res),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("Timeout".to_string()),
    }
}

/// 查詢當前 chat_llm 是否支援 thinking
#[tauri::command]
pub async fn get_chat_llm_supports_thinking(
    pool: State<'_, SqlitePool>,
) -> Result<bool, String> {
    let settings = store::get_settings(pool.inner())
        .await
        .map_err(|e| format!("Database error: {}", e))?;
    let cfg = settings.ai_models.chat_llm;
    let style = model_caps::detect(&cfg.model, &cfg.provider);
    Ok(style != model_caps::ReasoningStyle::None)
}
