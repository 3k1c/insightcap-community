use std::sync::Arc;

use chrono::Utc;
use serde::Deserialize;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::providers::embedding::Embedder;
use crate::providers::llm::openai::OpenAiProvider;
use crate::providers::llm::{LLMOptions, LLMProvider};
use crate::settings::store::get_settings;
use crate::vector_store::local::VectorStore;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TaggerResult {
    tags: Vec<String>,
    knowledge_type: String,
}

pub struct MemoryEngine {
    pool: SqlitePool,
    vector_store: VectorStore,
    embedder: Arc<dyn Embedder>,
}

impl MemoryEngine {
    pub fn new(pool: SqlitePool, vector_store: VectorStore, embedder: Arc<dyn Embedder>) -> Self {
        Self {
            pool,
            vector_store,
            embedder,
        }
    }

    pub async fn tag_capture(
        &self,
        capture_id: &str,
        content: &str,
    ) -> Result<Vec<String>, String> {
        let settings = get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let output_language = output_language_label(&settings.general.language);
        let cfg = settings.ai_models.content_processor_llm;
        let api_key = cfg.api_key.clone().unwrap_or_default();

        if api_key.is_empty() && cfg.provider != "ollama" {
            let fallback = vec!["untagged".to_string()];
            self.write_tags_to_capture(capture_id, &fallback).await?;
            return Ok(fallback);
        }

        let prompt = format!(
            "Analyze the following text and perform two tasks:\n\
             1. Extract 1 to 5 relevant tags (short keywords).\n\
             - Tags must be in {output_language}. Preserve dominant technical terms exactly as written.\n\
             2. Classify the knowledge type into one of: 'data', 'pattern', 'log'.\n\
             Output ONLY a valid JSON object with keys \"tags\" and \"knowledge_type\".\n\
             Example: {{\"tags\": [\"rust\", \"memory\"], \"knowledge_type\": \"data\"}}\n\
             Do not output any other text or markdown.\n\n\
             Text:\n{content}",
            content = content
        );

        let provider = OpenAiProvider::new(
            api_key,
            cfg.base_url,
            cfg.model.clone(),
            cfg.provider.clone(),
        );
        let opts = LLMOptions {
            temperature: 0.1,
            max_tokens: 200,
            stream: false,
            think_mode: None,
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            provider.complete_json(&prompt, opts),
        )
        .await;

        let tags = match result {
            Ok(Ok(json)) => parse_tags_from_json(&json),
            _ => {
                eprintln!(
                    "[MemoryEngine] tag_capture timeout/error for {}",
                    capture_id
                );
                vec!["untagged".to_string()]
            }
        };

        self.write_tags_to_capture(capture_id, &tags).await?;
        Ok(tags)
    }

    pub async fn process_conversation_summary(
        &self,
        conversation_id: &str,
        summary_text: &str,
        project_id: Option<&str>,
    ) -> Result<String, String> {
        let settings = get_settings(&self.pool).await.map_err(|e| e.to_string())?;
        let output_language = output_language_label(&settings.general.language);
        let cfg = settings.ai_models.content_processor_llm;
        let api_key = cfg.api_key.clone().unwrap_or_default();

        let (knowledge_type, tags, trigger_context, confidence) =
            if api_key.is_empty() && cfg.provider != "ollama" {
                (
                    "data".to_string(),
                    vec!["untagged".to_string()],
                    String::new(),
                    0.5_f32,
                )
            } else {
                self.deep_infer_knowledge_type(
                    summary_text,
                    &api_key,
                    cfg.base_url.unwrap_or_default(),
                    cfg.model,
                    output_language,
                )
                .await
            };

        let pending_confirm = if confidence < 0.75 { 1_i32 } else { 0_i32 };

        let vector_id = match self.embedder.embed(summary_text).await {
            Ok(vec) => {
                let chunk_id_hash = str_to_u64(conversation_id);
                if let Ok(()) = self.vector_store.add_vector(chunk_id_hash, &vec).await {
                    let vs = self.vector_store.clone();
                    tokio::spawn(async move {
                        let _ = vs.save().await;
                    });
                    Some(chunk_id_hash as i64)
                } else {
                    None
                }
            }
            Err(e) => {
                eprintln!("[MemoryEngine] embed failed: {}", e);
                None
            }
        };

        let now = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());

        let valid_project_id: Option<&str> = if let Some(pid) = project_id {
            let exists: bool =
                sqlx::query_scalar::<_, i32>("SELECT COUNT(*) FROM projects WHERE id = ?")
                    .bind(pid)
                    .fetch_one(&self.pool)
                    .await
                    .unwrap_or(0)
                    > 0;
            if exists {
                Some(pid)
            } else {
                None
            }
        } else {
            None
        };

        let existing_id: Option<String> =
            sqlx::query_scalar("SELECT id FROM memory_chunks WHERE conversation_id = ? LIMIT 1")
                .bind(conversation_id)
                .fetch_optional(&self.pool)
                .await
                .unwrap_or(None);

        let chunk_id = if let Some(eid) = existing_id {
            sqlx::query(
                "UPDATE memory_chunks SET \
                 project_id = ?, knowledge_type = ?, content = ?, tags = ?, \
                 trigger_context = ?, confidence = ?, pending_confirm = ?, \
                 vector_id = ?, updated_at = ? \
                 WHERE id = ?",
            )
            .bind(valid_project_id)
            .bind(&knowledge_type)
            .bind(summary_text)
            .bind(&tags_json)
            .bind(&trigger_context)
            .bind(confidence)
            .bind(pending_confirm)
            .bind(vector_id)
            .bind(&now)
            .bind(&eid)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
            eid
        } else {
            let chunk_id = Uuid::now_v7().to_string();
            sqlx::query(
                "INSERT INTO memory_chunks \
                 (id, conversation_id, project_id, knowledge_type, content, tags, \
                  trigger_context, confidence, pending_confirm, vector_id, placed_by, \
                  created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'ai', ?, ?)",
            )
            .bind(&chunk_id)
            .bind(conversation_id)
            .bind(valid_project_id)
            .bind(&knowledge_type)
            .bind(summary_text)
            .bind(&tags_json)
            .bind(&trigger_context)
            .bind(confidence)
            .bind(pending_confirm)
            .bind(vector_id)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
            chunk_id
        };

        println!(
            "[MemoryEngine] memory_chunk {} persisted: type={}, confidence={:.2}, pending={}",
            chunk_id, knowledge_type, confidence, pending_confirm
        );
        Ok(chunk_id)
    }

    pub async fn confirm_memory_chunk(&self, chunk_id: &str, accept: bool) -> Result<(), String> {
        if accept {
            sqlx::query(
                "UPDATE memory_chunks SET pending_confirm = 0, updated_at = ? WHERE id = ?",
            )
            .bind(Utc::now().to_rfc3339())
            .bind(chunk_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        } else {
            sqlx::query(
                "UPDATE memory_chunks SET knowledge_type = 'data', pending_confirm = 0, updated_at = ? WHERE id = ?"
            )
            .bind(Utc::now().to_rfc3339())
            .bind(chunk_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    async fn deep_infer_knowledge_type(
        &self,
        content: &str,
        api_key: &str,
        base_url: String,
        model: String,
        output_language: &'static str,
    ) -> (String, Vec<String>, String, f32) {
        let prompt = format!(
            "Analyze the following conversation summary and return a strict JSON object.\n\n\
             Tasks:\n\
             1. Extract 1 to 5 relevant short tags.\n\
             - Tags must be in {output_language}. Preserve dominant technical terms exactly as written.\n\
             2. Classify knowledge_type using strict rules:\n\
             - \"pattern\": confirmed reusable method/workflow/SOP/decision framework.\n\
             - \"log\": concrete failure, error, wrong direction, pitfall, or lesson learned.\n\
             - \"data\": all other cases. If uncertain, choose \"data\".\n\
             3. If type is \"log\", fill \"trigger_context\" with pipe-separated scenario keywords (example: \"competitor-analysis | crawler\"). Otherwise keep it empty.\n\
             4. Set \"confidence\" as a float between 0.0 and 1.0.\n\
             - data without clear signals: 0.85-0.95\n\
             - pattern/log with clear signals: 0.80-0.95\n\
             - pattern/log with weak signals: 0.50-0.75\n\n\
             Output valid JSON only, with keys: \"tags\", \"knowledge_type\", \"trigger_context\", \"confidence\".\n\
             Example: {{\"tags\": [\"rust\", \"async\"], \"knowledge_type\": \"pattern\", \"trigger_context\": \"\", \"confidence\": 0.88}}\n\
             Do not output any extra text or markdown.\n\n\
             Conversation summary:\n{content}",
            content = content
        );

        let provider =
            OpenAiProvider::new(api_key.to_string(), Some(base_url), model, String::new());
        let opts = LLMOptions {
            temperature: 0.1,
            max_tokens: 300,
            stream: false,
            think_mode: None,
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            provider.complete_json(&prompt, opts),
        )
        .await;

        match result {
            Ok(Ok(json)) => {
                let knowledge_type = json["knowledge_type"]
                    .as_str()
                    .filter(|s| matches!(*s, "data" | "pattern" | "log"))
                    .unwrap_or("data")
                    .to_string();

                let tags = parse_tags_from_json(&json);
                let trigger_context = json["trigger_context"].as_str().unwrap_or("").to_string();

                let confidence = json["confidence"]
                    .as_f64()
                    .map(|f| f.clamp(0.0, 1.0) as f32)
                    .unwrap_or(if knowledge_type == "data" { 0.9 } else { 0.7 });

                (knowledge_type, tags, trigger_context, confidence)
            }
            _ => {
                eprintln!("[MemoryEngine] deep_infer timeout/error, fallback to data");
                (
                    "data".to_string(),
                    vec!["untagged".to_string()],
                    String::new(),
                    0.5,
                )
            }
        }
    }

    async fn write_tags_to_capture(&self, capture_id: &str, tags: &[String]) -> Result<(), String> {
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
        sqlx::query("UPDATE captures SET tags = ?, updated_at = ? WHERE id = ?")
            .bind(&tags_json)
            .bind(Utc::now().to_rfc3339())
            .bind(capture_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn output_language_label(language: &str) -> &'static str {
    match language {
        "zh-CN" => "Simplified Chinese",
        "en" => "English",
        _ => "Traditional Chinese",
    }
}

fn parse_tags_from_json(json: &serde_json::Value) -> Vec<String> {
    let raw = json["tags"].as_array();
    match raw {
        Some(arr) => {
            let mut tags: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if tags.is_empty() {
                tags.push("untagged".to_string());
            }
            tags
        }
        None => vec!["untagged".to_string()],
    }
}

fn str_to_u64(s: &str) -> u64 {
    if let Ok(uuid) = s.parse::<uuid::Uuid>() {
        let bytes = uuid.as_bytes();
        u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    } else {
        let mut hash: u64 = 14695981039346656037;
        for byte in s.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
        hash
    }
}
