use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use tauri::Manager;
use uuid::Uuid;

use crate::error::AppError;
use crate::knowledge_source::{
    KnowledgeError, KnowledgeSource, KnowledgeSourceType, QueryScope, ScoredChunk,
};
use crate::providers::embedding::fastembed::FastEmbedder;
use crate::providers::embedding::Embedder;
use crate::vector_store::multi_index::MultiIndexManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KbMetadata {
    pub kb_version: String,
    pub kb_type: String,
    pub embedding_model: String,
    pub embedding_dimension: u32,
    pub created_by: String,
    pub description: String,
    pub is_readonly: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalKnowledgeBase {
    pub id: String,
    pub name: String,
    pub db_path: String,
    pub kb_type: String,
    pub embedding_model: String,
    pub embedding_dimension: i64,
    pub created_by: String,
    pub description: String,
    pub status: String,
    pub last_checked: Option<String>,
    pub loaded_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalKbLoadResult {
    pub success: bool,
    pub reason: Option<String>,
    pub metadata: Option<KbMetadata>,
}

struct ExternalKbCompatibility {
    is_compatible: bool,
    reason: Option<String>,
    metadata: KbMetadata,
}

pub struct EnterpriseKnowledgeSource {
    pub ekb_id: String,
    pub db_path: String,
    pub pool: SqlitePool,
    pub multi_index: Arc<MultiIndexManager>,
}

impl EnterpriseKnowledgeSource {
    pub async fn new(
        ekb_id: String,
        db_path: String,
        multi_index: Arc<MultiIndexManager>,
    ) -> Result<Self, KnowledgeError> {
        let url = format!("sqlite:{}?mode=ro", db_path);
        let pool = SqlitePool::connect(&url).await.map_err(|e| {
            KnowledgeError::Database(format!("Failed to connect external DB: {}", e))
        })?;

        Ok(Self {
            ekb_id,
            db_path,
            pool,
            multi_index,
        })
    }
}

impl KnowledgeSource for EnterpriseKnowledgeSource {
    fn source_type(&self) -> KnowledgeSourceType {
        KnowledgeSourceType::EnterpriseExternal(self.ekb_id.clone())
    }

    fn semantic_search(
        &self,
        query_embedding: &[f32],
        _scope: &QueryScope,
        limit: usize,
    ) -> impl std::future::Future<Output = Result<Vec<ScoredChunk>, KnowledgeError>> + Send {
        let ekb_id = self.ekb_id.clone();
        let pool = self.pool.clone();
        let query_embedding = query_embedding.to_vec();
        let multi_index = self.multi_index.clone();

        async move {
            let store = multi_index.get(&ekb_id).await.ok_or_else(|| {
                KnowledgeError::VectorSearch("External index not loaded".to_string())
            })?;

            let results = store
                .search(&query_embedding, limit)
                .await
                .map_err(|e| KnowledgeError::VectorSearch(e))?;

            let mut chunks = Vec::new();

            for (vid, score) in results {
                if let Ok(row) = sqlx::query(
                    "SELECT c.id, c.clean_content, c.type, s.title 
                     FROM captures c 
                     LEFT JOIN sources s ON c.source_id = s.id 
                     WHERE c.rowid = ?",
                )
                .bind(vid as i64)
                .fetch_optional(&pool)
                .await
                {
                    if let Some(r) = row {
                        chunks.push(ScoredChunk {
                            id: r.try_get("id").unwrap_or_default(),
                            content: r.try_get("clean_content").unwrap_or_default(),
                            score,
                            knowledge_type: "data".to_string(), // external kb is primarily reference data
                            source_title: r.try_get("title").ok(),
                            tags: vec![],
                        });
                    }
                }
            }

            Ok(chunks)
        }
    }

    fn keyword_trigger(
        &self,
        query: &str,
        _scope: &QueryScope,
    ) -> impl std::future::Future<Output = Result<Vec<ScoredChunk>, KnowledgeError>> + Send {
        let pool = self.pool.clone();
        let pattern = format!("%{}%", query);
        async move {
            let rows = sqlx::query(
                "SELECT c.id, c.clean_content, c.type, s.title 
                 FROM captures c 
                 LEFT JOIN sources s ON c.source_id = s.id 
                 WHERE c.clean_content LIKE ? 
                 LIMIT 10",
            )
            .bind(&pattern)
            .fetch_all(&pool)
            .await
            .map_err(|e| KnowledgeError::Database(e.to_string()))?;

            let mut chunks = Vec::new();
            for r in rows {
                chunks.push(ScoredChunk {
                    id: r.try_get("id").unwrap_or_default(),
                    content: r.try_get("clean_content").unwrap_or_default(),
                    score: 0.8,
                    knowledge_type: "data".to_string(),
                    source_title: r.try_get("title").ok(),
                    tags: vec![],
                });
            }
            Ok(chunks)
        }
    }
}

async fn read_kb_metadata(conn: &SqlitePool) -> Result<KbMetadata, AppError> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM settings WHERE key LIKE 'kb_%'")
            .fetch_all(conn)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

    let mut map = std::collections::HashMap::new();
    for (k, v) in rows {
        map.insert(k, v);
    }

    let embedding_dimension: u32 = map
        .get("kb_embedding_dimension")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(384);

    Ok(KbMetadata {
        kb_version: map
            .get("kb_version")
            .cloned()
            .unwrap_or_else(|| "2".to_string()),
        kb_type: map
            .get("kb_type")
            .cloned()
            .unwrap_or_else(|| "general".to_string()),
        embedding_model: map
            .get("kb_embedding_model")
            .cloned()
            .unwrap_or_else(|| "fastembed-default".to_string()),
        embedding_dimension,
        created_by: map.get("kb_created_by").cloned().unwrap_or_default(),
        description: map.get("kb_description").cloned().unwrap_or_default(),
        is_readonly: map.get("kb_is_readonly").map(|v| v == "1").unwrap_or(false),
    })
}

async fn check_compatibility(
    db_path: &str,
    local_embedding_model: &str,
    local_embedding_dimension: u32,
) -> Result<ExternalKbCompatibility, AppError> {
    let url = format!("sqlite:{}?mode=ro", db_path);
    let conn = sqlx::SqlitePool::connect(&url)
        .await
        .map_err(|e| AppError::Database(format!("Failed to open external DB: {}", e)))?;

    let metadata = read_kb_metadata(&conn)
        .await
        .unwrap_or_else(|_| KbMetadata {
            kb_version: "2".to_string(),
            kb_type: "general".to_string(),
            embedding_model: local_embedding_model.to_string(),
            embedding_dimension: local_embedding_dimension,
            created_by: "".to_string(),
            description: "".to_string(),
            is_readonly: true,
        });

    conn.close().await;

    let kb_version: u32 = metadata.kb_version.parse().unwrap_or(0);
    if kb_version < 1 {
        return Ok(ExternalKbCompatibility {
            is_compatible: false,
            reason: Some("External KB version is too old".to_string()),
            metadata,
        });
    }

    Ok(ExternalKbCompatibility {
        is_compatible: true,
        reason: None,
        metadata,
    })
}

#[tauri::command]
pub async fn load_external_kb(
    app: tauri::AppHandle,
    state_pool: tauri::State<'_, SqlitePool>,
    db_path: String,
) -> Result<ExternalKbLoadResult, String> {
    let local_embedding_model = "fastembed-default";
    let local_embedding_dimension = 384;

    let db = state_pool.inner();

    let compat = check_compatibility(&db_path, local_embedding_model, local_embedding_dimension)
        .await
        .map_err(|e| e.to_string())?;

    if !compat.is_compatible {
        return Ok(ExternalKbLoadResult {
            success: false,
            reason: compat.reason,
            metadata: Some(compat.metadata),
        });
    }

    let meta = &compat.metadata;
    let ekb_id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query("DELETE FROM external_knowledge_bases WHERE db_path = ?")
        .bind(&db_path)
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;

    let name = if meta.description.is_empty() {
        std::path::Path::new(&db_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&db_path)
            .to_string()
    } else {
        meta.description.clone()
    };

    sqlx::query(
        "INSERT INTO external_knowledge_bases
         (id, name, uri, status, created_at, updated_at)
         VALUES (?, ?, ?, 'connected', ?, ?)",
    )
    .bind(&ekb_id)
    .bind(&name)
    .bind(&db_path)
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;

    let ekb_id_clone = ekb_id.clone();
    let db_path_clone = db_path.clone();
    tauri::async_runtime::spawn(async move {
        build_external_kb_index(&app, &ekb_id_clone, &db_path_clone).await;
    });

    Ok(ExternalKbLoadResult {
        success: true,
        reason: None,
        metadata: Some(compat.metadata),
    })
}

pub async fn build_external_kb_index(app: &tauri::AppHandle, ekb_id: &str, db_path: &str) {
    let multi_index: tauri::State<'_, MultiIndexManager> =
        match app.try_state::<MultiIndexManager>() {
            Some(s) => s,
            None => return,
        };

    let embedder: tauri::State<'_, FastEmbedder> = match app.try_state::<FastEmbedder>() {
        Some(e) => e,
        None => return,
    };

    let url = format!("sqlite:{}?mode=ro", db_path);
    let conn = match sqlx::SqlitePool::connect(&url).await {
        Ok(c) => c,
        Err(_) => return,
    };

    let chunks: Vec<(i64, String)> = match sqlx::query_as(
        "SELECT rowid, clean_content FROM captures WHERE status = 'inbox' OR status = 'processed' AND clean_content != '' LIMIT 10000"
    )
    .fetch_all(&conn)
    .await {
        Ok(c) => c,
        Err(_) => {
            let _ = conn.close().await;
            return;
        }
    };

    let _ = conn.close().await;

    if chunks.is_empty() {
        return;
    }

    let store = multi_index.load_or_create_external(ekb_id, 384).await;
    let batch_size = 32;

    for batch in chunks.chunks(batch_size) {
        let texts: Vec<&str> = batch.iter().map(|(_, t)| t.as_str()).collect();
        if let Ok(vectors) = embedder.embed_batch(&texts).await {
            for (item, vector) in batch.iter().zip(vectors.iter()) {
                let rowid = item.0;
                let _ = store.add_vector(rowid as u64, vector).await;
            }
        }
    }

    let _ = store.save().await;
    multi_index.insert(ekb_id.to_string(), store).await;
}

#[tauri::command]
pub async fn remove_external_kb(
    pool: tauri::State<'_, SqlitePool>,
    ekb_id: String,
) -> Result<(), String> {
    sqlx::query("DELETE FROM external_knowledge_bases WHERE id = ?")
        .bind(&ekb_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_external_kbs(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<ExternalKnowledgeBase>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, name, uri, status, created_at, updated_at
         FROM external_knowledge_bases
         ORDER BY created_at DESC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(
            |(id, name, uri, status, created_at, updated_at)| ExternalKnowledgeBase {
                id,
                name,
                db_path: uri,
                kb_type: "general".to_string(),
                embedding_model: "fastembed".to_string(),
                embedding_dimension: 384,
                created_by: "".to_string(),
                description: "".to_string(),
                status,
                last_checked: None,
                loaded_at: created_at,
                updated_at,
            },
        )
        .collect())
}
