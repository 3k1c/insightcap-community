use crate::settings::security::decrypt;
use crate::settings::security::encrypt;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub launch_at_startup: bool,
    pub minimize_to_tray: bool,
    pub language: String, // 'zh-TW' | 'zh-CN' | 'en'
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            launch_at_startup: false,
            minimize_to_tray: true,
            language: "zh-TW".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelSettings {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AIModelSettings {
    pub chat_llm: ModelSettings,
    pub content_processor_llm: ModelSettings,
    pub vision_model: ModelSettings,
    pub embedding_model: ModelSettings,
    pub summary_model: Option<String>,
    #[serde(default)]
    pub provider_profiles: Vec<ProviderProfile>,
}

impl Default for AIModelSettings {
    fn default() -> Self {
        let default_url = Some("http://localhost:11434".to_string());
        Self {
            chat_llm: ModelSettings {
                provider: "ollama".to_string(),
                model: "qwen2.5:7b".to_string(),
                api_key: None,
                base_url: default_url.clone(),
            },
            content_processor_llm: ModelSettings {
                provider: "ollama".to_string(),
                model: "qwen2.5:3b".to_string(),
                api_key: None,
                base_url: default_url.clone(),
            },
            vision_model: ModelSettings {
                provider: "ollama".to_string(),
                model: "deepseek-ocr".to_string(),
                api_key: None,
                base_url: default_url.clone(),
            },
            embedding_model: ModelSettings {
                provider: "local".to_string(),
                model: "MultilingualE5Small".to_string(),
                api_key: None,
                base_url: None,
            },
            summary_model: Some("follow_chat".to_string()),
            provider_profiles: vec![],
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSettings {
    pub kb_path: String,
    pub auto_classify_enabled: bool,
    #[serde(default = "default_auto_space_mode")]
    pub auto_space_mode: String,
}

fn default_auto_space_mode() -> String {
    "suggest".to_string()
}

impl Default for KnowledgeSettings {
    fn default() -> Self {
        Self {
            kb_path: "".to_string(),
            auto_classify_enabled: true,
            auto_space_mode: "suggest".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HotkeySettings {
    pub capture_clipboard: String,
    pub quick_input: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            capture_clipboard: "Ctrl+Alt+F".to_string(),
            quick_input: "Ctrl+Alt+G".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct AutoCleanupSettings {
    pub enabled: bool,
    pub retention_days: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TelegramSettings {
    pub bot_token: String,
    pub allowed_user_ids: Vec<i64>,
    pub enabled: bool,
    pub prompt_instruction_override: Option<String>,
    pub streaming: String, // "full" | "partial"
}

impl Default for TelegramSettings {
    fn default() -> Self {
        Self {
            bot_token: "".to_string(),
            allowed_user_ids: vec![],
            enabled: false,
            prompt_instruction_override: None,
            streaming: "full".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReminderSettings {
    pub enabled: bool,
    pub daily_reminder_time: String,
    pub quiet_hours_start: String,
    pub quiet_hours_end: String,
    pub weekend_quiet: bool,
}

impl Default for ReminderSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            daily_reminder_time: "09:00".to_string(),
            quiet_hours_start: "22:00".to_string(),
            quiet_hours_end: "08:00".to_string(),
            weekend_quiet: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchSettings {
    pub enabled: bool,
    pub provider: String,
    pub api_key: String,
}

impl Default for WebSearchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "tavily".to_string(),
            api_key: "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".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettings {
    pub default_font: String,
    pub default_font_size: String,
    pub default_line_spacing: String,
    pub default_export_format: String, // "docx" | "md" | "txt"
    pub export_subdir: String,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            default_font: "Microsoft JhengHei, sans-serif".to_string(),
            default_font_size: "12".to_string(),
            default_line_spacing: "1.5".to_string(),
            default_export_format: "md".to_string(),
            export_subdir: "exports".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundSynthesisSettings {
    pub enabled: bool,
    pub frequency_minutes: u32,
    pub max_chunks_per_batch: u32,
    pub force_content_processor_llm: bool,
}

impl Default for BackgroundSynthesisSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            frequency_minutes: 30,
            max_chunks_per_batch: 30,
            force_content_processor_llm: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AllSettings {
    pub general: GeneralSettings,
    pub ai_models: AIModelSettings,
    pub knowledge: KnowledgeSettings,
    pub hotkeys: HotkeySettings,
    pub auto_cleanup: AutoCleanupSettings,
    pub web_search: WebSearchSettings,
    pub telegram: TelegramSettings,
    pub reminders: ReminderSettings,
    #[serde(default)]
    pub editor: EditorSettings,
    pub bilibili_sessdata: Option<String>,
    #[serde(default)]
    pub chat_prompt_instruction: String,
    #[serde(default)]
    pub background_synthesis: BackgroundSynthesisSettings,
}

impl AllSettings {
    pub fn encrypt_all(&mut self) {
        if let Some(key) = &self.ai_models.chat_llm.api_key {
            self.ai_models.chat_llm.api_key = Some(encrypt(key));
        }
        if let Some(key) = &self.ai_models.content_processor_llm.api_key {
            self.ai_models.content_processor_llm.api_key = Some(encrypt(key));
        }
        if let Some(key) = &self.ai_models.vision_model.api_key {
            self.ai_models.vision_model.api_key = Some(encrypt(key));
        }
        if let Some(key) = &self.ai_models.embedding_model.api_key {
            self.ai_models.embedding_model.api_key = Some(encrypt(key));
        }
        if !self.web_search.api_key.is_empty() {
            self.web_search.api_key = encrypt(&self.web_search.api_key);
        }
        if !self.telegram.bot_token.is_empty() {
            self.telegram.bot_token = encrypt(&self.telegram.bot_token);
        }
        if let Some(sessdata) = &self.bilibili_sessdata {
            self.bilibili_sessdata = Some(encrypt(sessdata));
        }
        for profile in &mut self.ai_models.provider_profiles {
            if let Some(key) = &profile.api_key {
                if !key.is_empty() {
                    profile.api_key = Some(encrypt(key));
                }
            }
        }
    }

    pub fn decrypt_all(&mut self) {
        if let Some(key) = &self.ai_models.chat_llm.api_key {
            if let Ok(decrypted) = decrypt(key) {
                self.ai_models.chat_llm.api_key = Some(decrypted);
            }
        }
        if let Some(key) = &self.ai_models.content_processor_llm.api_key {
            if let Ok(decrypted) = decrypt(key) {
                self.ai_models.content_processor_llm.api_key = Some(decrypted);
            }
        }
        if let Some(key) = &self.ai_models.vision_model.api_key {
            if let Ok(decrypted) = decrypt(key) {
                self.ai_models.vision_model.api_key = Some(decrypted);
            }
        }
        if let Some(key) = &self.ai_models.embedding_model.api_key {
            if let Ok(decrypted) = decrypt(key) {
                self.ai_models.embedding_model.api_key = Some(decrypted);
            }
        }
        if !self.web_search.api_key.is_empty() {
            if let Ok(decrypted) = decrypt(&self.web_search.api_key) {
                self.web_search.api_key = decrypted;
            }
        }
        if !self.telegram.bot_token.is_empty() {
            if let Ok(decrypted) = decrypt(&self.telegram.bot_token) {
                self.telegram.bot_token = decrypted;
            }
        }
        if let Some(sessdata) = &self.bilibili_sessdata {
            if let Ok(decrypted) = decrypt(sessdata) {
                self.bilibili_sessdata = Some(decrypted);
            }
        }
        for profile in &mut self.ai_models.provider_profiles {
            if let Some(key) = &profile.api_key {
                if let Ok(decrypted) = decrypt(key) {
                    profile.api_key = Some(decrypted);
                }
            }
        }
    }

    pub fn resolve_profiles(&mut self) {
        let profiles = &self.ai_models.provider_profiles;

        let resolve_model = |model: &mut ModelSettings| {
            let needs_key = model.api_key.as_ref().map_or(true, |k| k.is_empty());
            let needs_url = model.base_url.as_ref().map_or(true, |u| u.is_empty());

            if needs_key || needs_url {
                if let Some(profile) = profiles.iter().find(|p| p.provider == model.provider) {
                    if needs_key {
                        model.api_key = profile.api_key.clone();
                    }
                    if needs_url {
                        model.base_url = profile.base_url.clone();
                    }
                } else if model.provider == "ollama" && needs_url {
                    model.base_url = Some("http://localhost:11434".to_string());
                }
            }
        };

        resolve_model(&mut self.ai_models.chat_llm);
        resolve_model(&mut self.ai_models.content_processor_llm);
        resolve_model(&mut self.ai_models.vision_model);
        resolve_model(&mut self.ai_models.embedding_model);
    }
}

fn is_path_safe(path: &str) -> bool {
    if path.is_empty() {
        return true;
    }

    if path.contains("..") {
        return false;
    }

    let p = Path::new(path);
    for component in p.components() {
        if let std::path::Component::ParentDir = component {
            return false;
        }
    }

    #[cfg(windows)]
    {
        let lower = path.to_lowercase();
        if lower.starts_with("c:\\windows") || lower.starts_with("c:\\users\\all users") {
            return false;
        }
    }

    true
}

pub async fn get_settings(pool: &SqlitePool) -> Result<AllSettings, sqlx::Error> {
    let mut settings = AllSettings::default();

    let rows = sqlx::query("SELECT key, value FROM settings")
        .fetch_all(pool)
        .await?;

    for row in rows {
        let key: String = row.get("key");
        let value: String = row.get("value");
        match key.as_str() {
            "general" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.general = val;
                }
            }
            "ai_models" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.ai_models = val;
                }
            }
            "knowledge" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.knowledge = val;
                }
            }
            "hotkeys" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.hotkeys = val;
                }
            }
            "auto_cleanup" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.auto_cleanup = val;
                }
            }
            "web_search" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.web_search = val;
                }
            }
            "telegram" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.telegram = val;
                }
            }
            "reminders" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.reminders = val;
                }
            }
            "bilibili" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.bilibili_sessdata = val;
                }
            }
            "editor" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.editor = val;
                }
            }
            "chat_prompt_instruction" => {
                settings.chat_prompt_instruction = value;
            }
            "background_synthesis" => {
                if let Ok(val) = serde_json::from_str(&value) {
                    settings.background_synthesis = val;
                }
            }
            _ => {}
        }
    }

    settings.decrypt_all();
    settings.resolve_profiles();

    Ok(settings)
}

pub async fn save_settings(
    pool: &SqlitePool,
    mut settings: AllSettings,
) -> Result<(), sqlx::Error> {
    if !is_path_safe(&settings.knowledge.kb_path) {
        return Err(sqlx::Error::Protocol(
            "Invalid knowledge base path: directory traversal is not allowed".to_string(),
        ));
    }

    let mut tx = pool.begin().await?;
    let now = chrono::Utc::now().to_rfc3339();

    settings.encrypt_all();

    let queries = vec![
        (
            "general",
            serde_json::to_string(&settings.general)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
        (
            "ai_models",
            serde_json::to_string(&settings.ai_models)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
        (
            "knowledge",
            serde_json::to_string(&settings.knowledge)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
        (
            "hotkeys",
            serde_json::to_string(&settings.hotkeys)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
        (
            "auto_cleanup",
            serde_json::to_string(&settings.auto_cleanup)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
        (
            "web_search",
            serde_json::to_string(&settings.web_search)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
        (
            "telegram",
            serde_json::to_string(&settings.telegram)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
        (
            "reminders",
            serde_json::to_string(&settings.reminders)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
        (
            "bilibili",
            serde_json::to_string(&settings.bilibili_sessdata)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
        (
            "editor",
            serde_json::to_string(&settings.editor)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
        (
            "chat_prompt_instruction",
            settings.chat_prompt_instruction.clone(),
        ),
        (
            "background_synthesis",
            serde_json::to_string(&settings.background_synthesis)
                .map_err(|e| sqlx::Error::Protocol(e.to_string()))?,
        ),
    ];

    for (key, value) in queries {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at"
        )
        .bind(key)
        .bind(value)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
