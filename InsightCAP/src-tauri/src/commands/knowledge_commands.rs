use crate::db::AppState;
use sqlx::{Row, SqlitePool};
use tauri::{Emitter, State, Runtime};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceItem {
    pub id: String,
    pub title: String,
    pub r#type: String,
    pub url: Option<String>,
    pub file_path: Option<String>,
    pub captured_at: String,
    pub content_preview: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureItem {
    pub id: String,
    pub source_id: Option<String>,
    pub clean_content: String,
    pub r#type: String,
    pub status: String,
    pub created_at: String,
}

#[tauri::command]
pub async fn get_sources(
    pool: State<'_, SqlitePool>,
    space_id: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<SourceItem>, String> {
    let max = limit.unwrap_or(50);

    let rows = if let Some(sid) = space_id {
        sqlx::query(
            "SELECT DISTINCT s.id, s.title, s.type, s.url, s.file_path, s.captured_at, \
             SUBSTR(s.clean_content, 1, 120) as preview \
             FROM sources s \
             JOIN captures c ON s.id = c.source_id \
             WHERE c.space_id = ? \
             ORDER BY s.captured_at DESC LIMIT ?",
        )
        .bind(sid)
        .bind(max)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query(
            "SELECT id, title, type, url, file_path, captured_at, \
             SUBSTR(clean_content, 1, 120) as preview \
             FROM sources \
             ORDER BY captured_at DESC LIMIT ?",
        )
        .bind(max)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?
    };

    let sources = rows
        .into_iter()
        .map(|r| SourceItem {
            id: r.try_get("id").unwrap_or_default(),
            title: r.get("title"),
            r#type: r.get("type"),
            url: r.try_get("url").unwrap_or(None),
            file_path: r.try_get("file_path").unwrap_or(None),
            captured_at: r.get("captured_at"),
            content_preview: r.try_get("preview").unwrap_or_default(),
        })
        .collect();

    Ok(sources)
}

#[tauri::command]
pub async fn get_captures(
    pool: State<'_, SqlitePool>,
    status: Option<String>,
    source_id: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<CaptureItem>, String> {
    let max = limit.unwrap_or(100);

    let rows = if let Some(sid) = source_id {
        let s = status.unwrap_or_else(|| "processed".to_string());
        sqlx::query(
            "SELECT id, source_id, type, clean_content, status, created_at \
             FROM captures WHERE status = ? AND source_id = ? \
             ORDER BY created_at DESC LIMIT ?",
        )
        .bind(s)
        .bind(sid)
        .bind(max)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?
    } else {
        let s = status.unwrap_or_else(|| "inbox".to_string());
        sqlx::query(
            "SELECT id, source_id, type, clean_content, status, created_at \
             FROM captures WHERE status = ? \
             ORDER BY created_at DESC LIMIT ?",
        )
        .bind(s)
        .bind(max)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?
    };

    let captures = rows
        .into_iter()
        .map(|r| CaptureItem {
            id: r.try_get("id").unwrap_or_default(),
            source_id: r.try_get("source_id").unwrap_or(None),
            clean_content: r.get("clean_content"),
            r#type: r.get("type"),
            status: r.try_get("status").unwrap_or_default(),
            created_at: r.get("created_at"),
        })
        .collect();

    Ok(captures)
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSourceItem {
    pub id: String,
    pub title: String,
    pub r#type: String,
    pub source_category: String,
    pub media_type: String,
    pub url: Option<String>,
    pub file_path: Option<String>,
    pub local_doc_path: Option<String>,
    pub thumbnail: Option<String>,
    pub captured_at: String,
    pub content_preview: String,
    pub capture_count: i64,
    pub tags: Vec<String>,
}

#[tauri::command]
pub async fn get_source_preview_by_title(
    pool: State<'_, SqlitePool>,
    title: String,
) -> Result<String, String> {
    let preview: Option<String> = sqlx::query_scalar(
        "SELECT SUBSTR(clean_content, 1, 500) FROM sources WHERE title = ? LIMIT 1",
    )
    .bind(title)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(preview.unwrap_or_else(|| "Source preview is unavailable.".to_string()))
}

#[tauri::command]
pub async fn get_sources_timeline(
    pool: State<'_, SqlitePool>,
    category: Option<String>,
    media_type: Option<String>,
    search_query: Option<String>,
    space_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<TimelineSourceItem>, String> {
    let max = limit.unwrap_or(50);
    let off = offset.unwrap_or(0);

    let has_new_cols: bool = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*) FROM pragma_table_info('sources') WHERE name = 'source_category'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(0)
        > 0;

    let mut sql = if has_new_cols {
        String::from(
            "SELECT s.id, s.title, s.type, \
             COALESCE(s.source_category, 'captured') as source_category, \
             COALESCE(s.media_type, 'text') as media_type, \
             s.url, s.file_path, s.local_doc_path, s.thumbnail, s.captured_at, \
             SUBSTR(s.clean_content, 1, 120) as preview, \
             s.capture_count, \
             COALESCE((SELECT GROUP_CONCAT(DISTINCT t.value) FROM (SELECT tags FROM captures WHERE source_id = s.id AND tags IS NOT NULL UNION ALL SELECT tags FROM sources WHERE id = s.id AND tags IS NOT NULL) raw, json_each(raw.tags) t WHERE t.value != '' AND t.value != 'untagged'), '') as agg_tags \
             FROM sources s WHERE 1=1"
        )
    } else {
        String::from(
            "SELECT s.id, s.title, s.type, \
             'captured' as source_category, \
             'text' as media_type, \
             s.url, s.file_path, NULL as local_doc_path, s.thumbnail, s.captured_at, \
             SUBSTR(s.clean_content, 1, 120) as preview, \
             s.capture_count, \
             COALESCE((SELECT GROUP_CONCAT(DISTINCT t.value) FROM (SELECT tags FROM captures WHERE source_id = s.id AND tags IS NOT NULL UNION ALL SELECT tags FROM sources WHERE id = s.id AND tags IS NOT NULL) raw, json_each(raw.tags) t WHERE t.value != '' AND t.value != 'untagged'), '') as agg_tags \
             FROM sources s WHERE 1=1"
        )
    };
    let mut binds: Vec<String> = Vec::new();

    if let Some(ref cat) = category {
        if has_new_cols {
            sql.push_str(" AND COALESCE(s.source_category, 'captured') = ?");
            binds.push(cat.clone());
        }
    }
    if let Some(ref mt) = media_type {
        if has_new_cols {
            sql.push_str(" AND COALESCE(s.media_type, 'text') = ?");
            binds.push(mt.clone());
        }
    }
    if let Some(ref q) = search_query {
        sql.push_str(" AND (s.title LIKE ? OR s.clean_content LIKE ?)");
        let like = format!("%{}%", q);
        binds.push(like.clone());
        binds.push(like);
    }
    if let Some(ref sid) = space_id {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM captures c2 WHERE c2.source_id = s.id AND c2.space_id = ?)",
        );
        binds.push(sid.clone());
    }

    sql.push_str(" ORDER BY s.captured_at DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query(&sql);
    for b in &binds {
        query = query.bind(b);
    }
    query = query.bind(max).bind(off);

    let rows = query
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let sources = rows
        .into_iter()
        .map(|r| {
            let agg: String = r.try_get("agg_tags").unwrap_or_default();
            let tags: Vec<String> = if agg.is_empty() {
                Vec::new()
            } else {
                agg.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            };
            TimelineSourceItem {
                id: r.try_get("id").unwrap_or_default(),
                title: r.get("title"),
                r#type: r.get("type"),
                source_category: r
                    .try_get("source_category")
                    .unwrap_or_else(|_| "captured".to_string()),
                media_type: r
                    .try_get("media_type")
                    .unwrap_or_else(|_| "text".to_string()),
                url: r.try_get("url").unwrap_or(None),
                file_path: r.try_get("file_path").unwrap_or(None),
                local_doc_path: r.try_get("local_doc_path").unwrap_or(None),
                thumbnail: r.try_get("thumbnail").unwrap_or(None),
                captured_at: r.get("captured_at"),
                content_preview: r.try_get("preview").unwrap_or_default(),
                capture_count: r.try_get("capture_count").unwrap_or(0),
                tags,
            }
        })
        .collect();

    Ok(sources)
}

#[tauri::command]
pub async fn create_editor_document(
    state: State<'_, AppState>,
    title: String,
) -> Result<TimelineSourceItem, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let id = uuid::Uuid::now_v7().to_string();

    let doc_dir = state.kb_path.join(".insightcap").join("documents");
    std::fs::create_dir_all(&doc_dir).map_err(|e| e.to_string())?;

    let safe_name = title.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let file_name = format!("{}_{}.md", safe_name, &id[..8]);
    let doc_path = doc_dir.join(&file_name);

    std::fs::write(&doc_path, format!("# {}\n", title)).map_err(|e| e.to_string())?;

    let local_path_str = doc_path.to_string_lossy().to_string();

    sqlx::query(
        "INSERT INTO sources (id, type, title, clean_content, content_hash, captured_at, updated_at, \
         source_category, media_type, local_doc_path, capture_count) \
         VALUES (?, 'editor', ?, '', '', ?, ?, 'editor_doc', 'text', ?, 0)"
    )
    .bind(&id)
    .bind(&title)
    .bind(&now)
    .bind(&now)
    .bind(&local_path_str)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(TimelineSourceItem {
        id,
        title,
        r#type: "editor".to_string(),
        source_category: "editor_doc".to_string(),
        media_type: "text".to_string(),
        url: None,
        file_path: None,
        local_doc_path: Some(local_path_str),
        thumbnail: None,
        captured_at: now,
        content_preview: String::new(),
        capture_count: 0,
        tags: Vec::new(),
    })
}

#[tauri::command]
pub async fn read_editor_document(
    pool: State<'_, SqlitePool>,
    source_id: String,
) -> Result<String, String> {
    let row = sqlx::query(
        "SELECT local_doc_path FROM sources WHERE id = ? AND source_category = 'editor_doc'",
    )
    .bind(&source_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Document not found".to_string())?;

    let path: String = row
        .try_get("local_doc_path")
        .map_err(|_| "No local_doc_path set".to_string())?;

    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_editor_document(
    pool: State<'_, SqlitePool>,
    source_id: String,
    content: String,
) -> Result<(), String> {
    let row = sqlx::query(
        "SELECT local_doc_path FROM sources WHERE id = ? AND source_category = 'editor_doc'",
    )
    .bind(&source_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Document not found".to_string())?;

    let path: String = row
        .try_get("local_doc_path")
        .map_err(|_| "No local_doc_path set".to_string())?;

    std::fs::write(&path, &content).map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE sources SET clean_content = SUBSTR(?, 1, 500), updated_at = ? WHERE id = ?",
    )
    .bind(&content)
    .bind(&now)
    .bind(&source_id)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_source(pool: State<'_, SqlitePool>, source_id: String) -> Result<(), String> {
    let has_new_cols: bool = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*) FROM pragma_table_info('sources') WHERE name = 'source_category'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(0)
        > 0;

    if has_new_cols {
        let row = sqlx::query("SELECT source_category, local_doc_path FROM sources WHERE id = ?")
            .bind(&source_id)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

        if let Some(r) = &row {
            let cat: String = r.try_get("source_category").unwrap_or_default();
            if cat == "editor_doc" {
                if let Ok(path) = r.try_get::<String, _>("local_doc_path") {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    sqlx::query("DELETE FROM sources WHERE id = ?")
        .bind(&source_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDetail {
    pub id: String,
    pub source_id: Option<String>,
    pub space_id: Option<String>,
    pub clean_content: String,
    pub r#type: String,
    pub tags: String,
    pub status: String,
    pub is_user_edited: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub async fn get_captures_detail(
    pool: State<'_, SqlitePool>,
    source_id: String,
) -> Result<Vec<CaptureDetail>, String> {
    let has_user_edited: bool = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*) FROM pragma_table_info('captures') WHERE name = 'is_user_edited'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(0)
        > 0;

    let detail_sql = if has_user_edited {
        "SELECT id, source_id, space_id, clean_content, type, \
         COALESCE(tags, '[]') as tags, status, \
         COALESCE(is_user_edited, 0) as is_user_edited, \
         created_at, updated_at \
         FROM captures WHERE source_id = ? ORDER BY chunk_index ASC, created_at ASC"
    } else {
        "SELECT id, source_id, space_id, clean_content, type, \
         COALESCE(tags, '[]') as tags, status, \
         0 as is_user_edited, \
         created_at, updated_at \
         FROM captures WHERE source_id = ? ORDER BY chunk_index ASC, created_at ASC"
    };

    let rows = sqlx::query(detail_sql)
        .bind(&source_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let captures = rows
        .into_iter()
        .map(|r| CaptureDetail {
            id: r.try_get("id").unwrap_or_default(),
            source_id: r.try_get("source_id").unwrap_or(None),
            space_id: r.try_get("space_id").unwrap_or(None),
            clean_content: r.get("clean_content"),
            r#type: r.get("type"),
            tags: r.try_get("tags").unwrap_or_else(|_| "[]".to_string()),
            status: r.try_get("status").unwrap_or_default(),
            is_user_edited: r.try_get::<i32, _>("is_user_edited").unwrap_or(0) != 0,
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

    Ok(captures)
}

#[tauri::command]
pub async fn create_manual_capture(
    pool: State<'_, SqlitePool>,
    source_id: Option<String>,
    content: String,
    tags: Option<String>,
    space_id: Option<String>,
) -> Result<CaptureDetail, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let id = uuid::Uuid::now_v7().to_string();
    let tag_str = tags.unwrap_or_else(|| "[]".to_string());

    let has_user_edited_col: bool = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*) FROM pragma_table_info('captures') WHERE name = 'is_user_edited'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(0)
        > 0;

    let insert_sql = if has_user_edited_col {
        "INSERT INTO captures (id, source_id, space_id, type, raw_content, clean_content, \
         capture_method, tags, status, is_user_edited, created_at, updated_at) \
         VALUES (?, ?, ?, 'text', ?, ?, 'manual', ?, 'processed', 1, ?, ?)"
    } else {
        "INSERT INTO captures (id, source_id, space_id, type, raw_content, clean_content, \
         capture_method, tags, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'text', ?, ?, 'manual', ?, 'processed', ?, ?)"
    };

    sqlx::query(insert_sql)
        .bind(&id)
        .bind(&source_id)
        .bind(&space_id)
        .bind(&content)
        .bind(&content)
        .bind(&tag_str)
        .bind(&now)
        .bind(&now)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    if let Some(ref sid) = source_id {
        sqlx::query(
            "UPDATE sources SET capture_count = capture_count + 1, updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(sid)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(CaptureDetail {
        id,
        source_id,
        space_id,
        clean_content: content,
        r#type: "text".to_string(),
        tags: tag_str,
        status: "processed".to_string(),
        is_user_edited: true,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub async fn update_capture(
    pool: State<'_, SqlitePool>,
    capture_id: String,
    content: Option<String>,
    tags: Option<String>,
    space_id: Option<String>,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();

    let has_user_edited_col: bool = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*) FROM pragma_table_info('captures') WHERE name = 'is_user_edited'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(0)
        > 0;

    let mut sets = if has_user_edited_col {
        vec![
            "updated_at = ?".to_string(),
            "is_user_edited = 1".to_string(),
        ]
    } else {
        vec!["updated_at = ?".to_string()]
    };
    let mut binds: Vec<String> = vec![now];

    if let Some(ref c) = content {
        sets.push("clean_content = ?".to_string());
        binds.push(c.clone());
    }
    if let Some(ref t) = tags {
        sets.push("tags = ?".to_string());
        binds.push(t.clone());
    }
    if let Some(ref s) = space_id {
        sets.push("space_id = ?".to_string());
        binds.push(s.clone());
    }

    let sql = format!("UPDATE captures SET {} WHERE id = ?", sets.join(", "));
    binds.push(capture_id);

    let mut query = sqlx::query(&sql);
    for b in &binds {
        query = query.bind(b);
    }
    query
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_capture(pool: State<'_, SqlitePool>, capture_id: String) -> Result<(), String> {
    let row = sqlx::query("SELECT source_id FROM captures WHERE id = ?")
        .bind(&capture_id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM captures WHERE id = ?")
        .bind(&capture_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    if let Some(r) = row {
        if let Ok(sid) = r.try_get::<String, _>("source_id") {
            let now = chrono::Utc::now().to_rfc3339();
            let _ = sqlx::query(
                "UPDATE sources SET capture_count = MAX(capture_count - 1, 0), updated_at = ? WHERE id = ?"
            )
            .bind(&now)
            .bind(&sid)
            .execute(pool.inner())
            .await;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn process_source(
    state: State<'_, AppState>,
    source_id: String,
) -> Result<usize, String> {
    let db = &state.db;

    let source_row = sqlx::query("SELECT id, file_path, title FROM sources WHERE id = ?")
        .bind(&source_id)
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Source not found: {}", source_id))?;

    let file_path: String = source_row.get("file_path");
    let title: String = source_row.get("title");

    let kb_path = {
        let settings = crate::settings::store::get_settings(db)
            .await
            .map_err(|e| e.to_string())?;
        settings.knowledge.kb_path
    };

    let parsed = crate::capture::file_parser::parse_file(&kb_path, &file_path, None).await?;
    let parsed_full_content = parsed
        .chunks
        .iter()
        .map(|c| c.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let source_identity =
        crate::capture::source_group::identity_for_file(&file_path, &parsed_full_content);
    let source_group_id =
        crate::capture::source_group::get_or_create_source_group(db, &source_identity, &title)
            .await?;

    let mut chunk_count: usize = 0;
    let now = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query("UPDATE sources SET source_group_id = ?, content_hash = ?, clean_content = ?, updated_at = ? WHERE id = ?")
        .bind(&source_group_id)
        .bind(&source_identity.content_hash)
        .bind(&parsed_full_content)
        .bind(&now)
        .bind(&source_id)
        .execute(db)
        .await;

    for f_chunk in parsed.chunks {
        let routed_chunks = crate::capture::chunking::chunks_for_file_chunk(&f_chunk);

        for routed in routed_chunks {
            let chunk_id = uuid::Uuid::now_v7().to_string();
            let para = routed.content;

            let mut vector_id = 0i64;

            if f_chunk.status != "pending_ocr" {
                match state.embedder.embed(&para).await {
                    Ok(vec) => {
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};
                        let mut hasher = DefaultHasher::new();
                        chunk_id.hash(&mut hasher);
                        vector_id = hasher.finish() as i64;
                        let _ = state.vector_store.add_vector(vector_id as u64, &vec).await;
                    }
                    Err(e) => eprintln!("[ProcessSource] vector store error: {}", e),
                }
            }

            sqlx::query(
                "INSERT INTO captures (id, source_id, type, raw_content, clean_content, \
                 capture_method, chunk_index, status, vector_id, content_type, knowledge_type, chunk_strategy, chunk_metadata, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, 'source_import', ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&chunk_id)
            .bind(&source_id)
            .bind(&f_chunk.chunk_type)
            .bind(&para)
            .bind(&para)
            .bind(chunk_count as i64)
            .bind(&f_chunk.status)
            .bind(vector_id)
            .bind(&routed.content_type)
            .bind(&routed.knowledge_type)
            .bind(&routed.chunk_strategy)
            .bind(&routed.metadata_json)
            .bind(&now)
            .bind(&now)
            .execute(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

            if f_chunk.status != "pending_ocr" {
                let tag_pool = db.clone();
                let tag_cid = chunk_id.clone();
                let tag_content = para.clone();
                tokio::spawn(async move {
                    let tag_engine = crate::services::tag_engine::TagEngine::new(tag_pool);
                    if let Err(e) = tag_engine.process_new_capture(&tag_cid, &tag_content).await {
                        eprintln!(
                            "[ProcessSource] Tag generation failed for {}: {}",
                            &tag_cid[..8.min(tag_cid.len())],
                            e
                        );
                    }
                });
            }

            chunk_count += 1;
        }
    }

    sqlx::query("UPDATE sources SET capture_count = ?, updated_at = ? WHERE id = ?")
        .bind(chunk_count as i64)
        .bind(&now)
        .bind(&source_id)
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;

    let vs = state.vector_store.clone();
    tokio::spawn(async move {
        let _ = vs.save().await;
    });

    println!(
        "[ProcessSource] {} chunks generated for source: {}",
        chunk_count,
        &source_id[..8.min(source_id.len())]
    );
    Ok(chunk_count)
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryStats {
    pub today_sources: i64,
    pub total_chunks: i64,
    pub total_data: i64,
    pub total_patterns: i64,
    pub total_logs: i64,
}

#[tauri::command]
pub async fn get_repository_stats(state: State<'_, AppState>) -> Result<RepositoryStats, String> {
    let db = &state.db;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let today_sources: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sources WHERE captured_at >= ?")
            .bind(format!("{}T00:00:00", today))
            .fetch_one(db)
            .await
            .unwrap_or(0);

    let total_chunks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM captures WHERE capture_method != 'temp_attachment' AND status != 'archived'"
    )
    .fetch_one(db).await.unwrap_or(0);

    let total_data: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM memory_chunks WHERE knowledge_type = 'data'")
            .fetch_one(db)
            .await
            .unwrap_or(0);

    let total_patterns: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM memory_chunks WHERE knowledge_type = 'pattern'")
            .fetch_one(db)
            .await
            .unwrap_or(0);

    let total_logs: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM memory_chunks WHERE knowledge_type = 'log'")
            .fetch_one(db)
            .await
            .unwrap_or(0);

    Ok(RepositoryStats {
        today_sources,
        total_chunks,
        total_data,
        total_patterns,
        total_logs,
    })
}

#[tauri::command]
pub async fn rebuild_kb_index(state: State<'_, AppState>) -> Result<usize, String> {
    let db = &state.db;

    state.vector_store.clear().await?;
    println!("[RebuildIndex] Vector store cleared.");

    let rows = sqlx::query(
        "SELECT id, clean_content FROM captures \
         WHERE status != 'archived' AND capture_method != 'temp_attachment' \
         ORDER BY created_at ASC",
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;

    let total = rows.len();
    println!("[RebuildIndex] Re-embedding {} captures...", total);

    let mut count: usize = 0;
    for row in &rows {
        let id: String = row.get("id");
        let content: String = row.get("clean_content");

        if content.trim().is_empty() {
            continue;
        }

        let vec = state
            .embedder
            .embed(&content)
            .await
            .map_err(|e| e.to_string())?;
        let vector_id = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            id.hash(&mut hasher);
            hasher.finish()
        };

        state
            .vector_store
            .add_vector(vector_id, &vec)
            .await
            .map_err(|e| format!("vector store error: {}", e))?;

        sqlx::query("UPDATE captures SET vector_id = ? WHERE id = ?")
            .bind(vector_id as i64)
            .bind(&id)
            .execute(db)
            .await
            .map_err(|e| e.to_string())?;

        count += 1;
    }

    let mc_rows = sqlx::query(
        "SELECT id, content, vector_id FROM memory_chunks \
         WHERE content IS NOT NULL AND content != '' \
         AND pending_confirm = 0 \
         ORDER BY created_at ASC",
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;

    println!(
        "[RebuildIndex] Re-embedding {} memory_chunks...",
        mc_rows.len()
    );

    for row in &mc_rows {
        let mc_id: String = row.get("id");
        let content: String = row.get("content");
        let existing_vid: Option<i64> = row.get("vector_id");

        if content.trim().is_empty() {
            continue;
        }

        let vec = state
            .embedder
            .embed(&content)
            .await
            .map_err(|e| e.to_string())?;

        let vector_id = match existing_vid {
            Some(vid) if vid != 0 => vid as u64,
            _ => {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                mc_id.hash(&mut hasher);
                hasher.finish()
            }
        };

        state
            .vector_store
            .add_vector(vector_id, &vec)
            .await
            .map_err(|e| format!("vector store error: {}", e))?;

        sqlx::query("UPDATE memory_chunks SET vector_id = ? WHERE id = ?")
            .bind(vector_id as i64)
            .bind(&mc_id)
            .execute(db)
            .await
            .map_err(|e| e.to_string())?;

        count += 1;
    }

    state.vector_store.save().await?;
    println!(
        "[RebuildIndex] Done. {} vectors rebuilt (captures + memory_chunks).",
        count
    );
    Ok(count)
}

#[tauri::command]
pub async fn rebuild_source_tags<R: Runtime>(
    state: State<'_, AppState>,
    app: tauri::AppHandle<R>,
) -> Result<usize, String> {
    let db = &state.db;

    let rows = sqlx::query(
        "SELECT id, clean_content FROM sources \
         WHERE (tags IS NULL OR tags = '[]' OR tags = '') \
         AND (clean_content IS NOT NULL AND clean_content != '') \
         ORDER BY captured_at ASC",
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;

    let total = rows.len();
    println!(
        "[RebuildSourceTags] Processing {} sources without tags",
        total
    );

    let _ = app.emit(
        "rebuild-tags-progress",
        serde_json::json!({
            "current": 0, "total": total, "done": false
        }),
    );

    if total == 0 {
        let _ = app.emit(
            "rebuild-tags-progress",
            serde_json::json!({
                "current": 0, "total": 0, "done": true
            }),
        );
        return Ok(0);
    }

    let mut count: usize = 0;
    for (i, row) in rows.iter().enumerate() {
        let id: String = row.get("id");
        let content: String = row.get("clean_content");

        let tag_engine = crate::services::tag_engine::TagEngine::new(db.clone());
        match tag_engine.process_source(&id, &content).await {
            Ok(_) => {
                count += 1;
            }
            Err(e) => {
                eprintln!(
                    "[RebuildSourceTags] source {} failed: {}",
                    &id[..8.min(id.len())],
                    e
                );
            }
        }

        let _ = app.emit(
            "rebuild-tags-progress",
            serde_json::json!({
                "current": i + 1, "total": total, "done": false
            }),
        );
    }

    let _ = app.emit(
        "rebuild-tags-progress",
        serde_json::json!({
            "current": total, "total": total, "done": true
        }),
    );

    println!(
        "[RebuildSourceTags] Finished, succeeded {}/{} sources",
        count, total
    );
    Ok(count)
}

const KB_EXPORT_SCHEMA_VERSION: u32 = 17;
const KB_MANIFEST_PATH: &str = ".insightcap/manifest.json";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct KbExportManifest {
    schema_version: u32,
    exported_at: String,
    includes_reminders: bool,
}

fn parse_manifest_schema_version(value: &str) -> Option<u32> {
    value.parse::<u32>().ok()
}
#[tauri::command]
pub async fn export_kb(
    state: State<'_, AppState>,
    dest_path: String,
    mnemonic: String,
) -> Result<(), String> {
    use crate::auth::key_derivation::derive_recovery_key_new;
    use crate::auth::recovery::write_recovery_bin;
    use std::io::Write;

    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&state.db)
        .await
        .map_err(|e| format!("WAL checkpoint failed: {}", e))?;

    let key_hex = keyring::Entry::new("insightcap", "auto_login_key")
        .map_err(|e| format!("Keyring access failed: {}", e))?
        .get_password()
        .map_err(|_| "Failed to read db_key; make sure auto-login mode is available".to_string())?;
    let key_bytes = hex::decode(&key_hex).map_err(|_| "Invalid db_key hex".to_string())?;
    if key_bytes.len() != 32 {
        return Err("Invalid db_key length".to_string());
    }
    let mut db_key = [0u8; 32];
    db_key.copy_from_slice(&key_bytes);

    let (backup_recovery_key, salt) = derive_recovery_key_new(&mnemonic)
        .map_err(|e| format!("Recovery mnemonic handling failed: {}", e))?;

    let schema_version =
        sqlx::query_scalar::<_, String>("SELECT id FROM _migrations ORDER BY id DESC LIMIT 1")
            .fetch_optional(&state.db)
            .await
            .map_err(|e| format!("Failed to read schema version: {}", e))?
            .and_then(|id| parse_manifest_schema_version(&id))
            .unwrap_or(KB_EXPORT_SCHEMA_VERSION);
    let manifest = KbExportManifest {
        schema_version,
        exported_at: chrono::Utc::now().to_rfc3339(),
        includes_reminders: true,
    };

    let kb_root = &state.kb_path;
    let tmp_bin = kb_root.join(".insightcap").join("backup_recovery_tmp.bin");
    write_recovery_bin(&tmp_bin, &db_key, &backup_recovery_key, &salt)
        .map_err(|e| format!("Failed to generate backup_recovery.bin: {}", e))?;
    let backup_bin_data = std::fs::read(&tmp_bin)
        .map_err(|e| format!("Failed to read backup_recovery.bin: {}", e))?;
    let _ = std::fs::remove_file(&tmp_bin);

    let zip_result = (|| -> Result<(), String> {
        let zip_file = std::fs::File::create(&dest_path)
            .map_err(|e| format!("Failed to create export file: {}", e))?;
        let mut zip = zip::ZipWriter::new(zip_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        let insightcap_dir = kb_root.join(".insightcap");
        let walker = walkdir::WalkDir::new(&insightcap_dir).follow_links(false);
        for entry in walker {
            let entry = entry.map_err(|e| format!("Failed to walk directory: {}", e))?;
            let abs_path = entry.path();
            let rel_path = abs_path
                .strip_prefix(kb_root)
                .map_err(|_| "Invalid path prefix".to_string())?
                .to_string_lossy()
                .replace('\\', "/");

            if abs_path.is_dir() {
                zip.add_directory(&format!("{}/", rel_path), options)
                    .map_err(|e| format!("Failed to add directory: {}", e))?;
                continue;
            }
            if let Some(name) = abs_path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with("-wal") || name.ends_with("-shm") || name == "backup_recovery.bin"
                {
                    continue;
                }
            }
            zip.start_file(&rel_path, options)
                .map_err(|e| format!("Failed to add file: {}", e))?;
            let data = std::fs::read(abs_path)
                .map_err(|e| format!("Failed to read file {}: {}", rel_path, e))?;
            zip.write_all(&data)
                .map_err(|e| format!("Failed to write zip entry: {}", e))?;
        }
        zip.start_file(".insightcap/backup_recovery.bin", options)
            .map_err(|e| format!("Failed to add backup_recovery.bin: {}", e))?;
        zip.write_all(&backup_bin_data)
            .map_err(|e| format!("Failed to write backup_recovery.bin: {}", e))?;

        zip.start_file(KB_MANIFEST_PATH, options)
            .map_err(|e| format!("Failed to add manifest: {}", e))?;
        let manifest_json = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
        zip.write_all(&manifest_json)
            .map_err(|e| format!("Failed to write manifest: {}", e))?;

        for dir_name in &["files", "notes"] {
            let dir_path = kb_root.join(dir_name);
            if !dir_path.exists() {
                continue;
            }
            let walker = walkdir::WalkDir::new(&dir_path).follow_links(false);
            for entry in walker {
                let entry = entry.map_err(|e| format!("Failed to walk directory: {}", e))?;
                let abs_path = entry.path();
                let rel_path = abs_path
                    .strip_prefix(kb_root)
                    .map_err(|_| "Invalid path prefix".to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");

                if abs_path.is_dir() {
                    zip.add_directory(&format!("{}/", rel_path), options)
                        .map_err(|e| format!("Failed to add directory: {}", e))?;
                } else {
                    zip.start_file(&rel_path, options)
                        .map_err(|e| format!("Failed to add file: {}", e))?;
                    let data = std::fs::read(abs_path)
                        .map_err(|e| format!("Failed to read file {}: {}", rel_path, e))?;
                    zip.write_all(&data)
                        .map_err(|e| format!("Failed to write zip entry: {}", e))?;
                }
            }
        }

        zip.finish()
            .map_err(|e| format!("Failed to finalize zip: {}", e))?;
        Ok(())
    })();

    zip_result?;
    println!("[ExportKB] Exported KB to: {}", dest_path);
    Ok(())
}
#[tauri::command]
pub async fn import_kb(
    handle: tauri::AppHandle,
    state: State<'_, AppState>,
    src_path: String,
    mnemonic: String,
    new_password: String,
) -> Result<String, String> {
    use crate::auth::key_derivation::{
        derive_db_key, derive_recovery_key_new, derive_recovery_key_verify, generate_mnemonic,
    };
    use crate::auth::recovery::{read_recovery_bin, write_recovery_bin};
    use keyring::Entry;
    use rand::Rng;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::io::Read;
    use std::str::FromStr;
    use zeroize::Zeroize;

    const KEYCHAIN_SERVICE: &str = "insightcap";
    const KEYCHAIN_AUTO_LOGIN: &str = "auto_login_key";

    let src = std::path::Path::new(&src_path);
    if !src.exists() {
        return Err("Source file does not exist".to_string());
    }

    let header = {
        let mut f =
            std::fs::File::open(src).map_err(|e| format!("Failed to read source file: {}", e))?;
        let mut buf = [0u8; 4];
        use std::io::Read as _;
        f.read_exact(&mut buf)
            .map_err(|_| "File is too short".to_string())?;
        buf
    };
    if &header != b"PK\x03\x04" {
        return Err("Invalid knowledge base package: expected .zip format".to_string());
    }

    {
        let zip_file =
            std::fs::File::open(src).map_err(|e| format!("Failed to open package: {}", e))?;
        let mut archive = zip::ZipArchive::new(zip_file)
            .map_err(|e| format!("Failed to parse package: {}", e))?;
        match archive.by_name(KB_MANIFEST_PATH) {
            Ok(mut manifest_file) => {
                let mut manifest_buf = String::new();
                manifest_file
                    .read_to_string(&mut manifest_buf)
                    .map_err(|e| format!("Failed to read manifest: {}", e))?;
                let manifest: KbExportManifest = serde_json::from_str(&manifest_buf)
                    .map_err(|e| format!("Invalid manifest format: {}", e))?;
                if manifest.schema_version > KB_EXPORT_SCHEMA_VERSION {
                    return Err(format!(
                        "Package schema_version={} is newer than supported version {}; please upgrade the app before importing.",
                        manifest.schema_version, KB_EXPORT_SCHEMA_VERSION
                    ));
                }
            }
            Err(zip::result::ZipError::FileNotFound) => {
                println!("[ImportKB] manifest not found, continue with legacy package");
            }
            Err(e) => return Err(format!("Failed to read manifest: {}", e)),
        };
    }

    let kb_root = &state.kb_path;
    let db_path = kb_root.join(".insightcap").join("insightcap.db");
    let backup_bin_path = kb_root.join(".insightcap").join("backup_recovery.bin");

    state.db.close().await;

    if db_path.exists() {
        let bak = db_path.with_extension("db.bak");
        std::fs::copy(&db_path, &bak)
            .map_err(|e| format!("Failed to back up existing DB: {}", e))?;
    }

    let zip_file =
        std::fs::File::open(src).map_err(|e| format!("Failed to open package: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(zip_file).map_err(|e| format!("Failed to parse package: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read package entry: {}", e))?;

        let entry_name = file.name().to_string();
        if entry_name.contains("..") || entry_name.starts_with('/') || entry_name.starts_with('\\')
        {
            return Err(format!("Unsafe package path: {}", entry_name));
        }
        let out_path = kb_root.join(&entry_name);
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directory: {}", e))?;
            }
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read package content: {}", e))?;
            std::fs::write(&out_path, &buf).map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }

    if !backup_bin_path.exists() {
        return Err("Package is missing backup_recovery.bin; cannot restore".to_string());
    }
    let bin_data = std::fs::read(&backup_bin_path)
        .map_err(|e| format!("Failed to read backup_recovery.bin: {}", e))?;
    if bin_data.len() < 17 {
        return Err("Invalid backup_recovery.bin format".to_string());
    }
    let mut stored_salt = [0u8; 16];
    stored_salt.copy_from_slice(&bin_data[1..17]);

    let backup_recovery_key = derive_recovery_key_verify(&mnemonic, &stored_salt)
        .map_err(|_| "INVALID_MNEMONIC".to_string())?;
    let (mut db_key, _) = read_recovery_bin(&backup_bin_path, &backup_recovery_key)
        .map_err(|_| "INVALID_MNEMONIC".to_string())?;

    let mut new_salt = [0u8; 32];
    rand::rng().fill_bytes(&mut new_salt);
    let mut new_db_key = derive_db_key(&new_password, &new_salt)
        .map_err(|e| format!("Failed to derive new DB key: {}", e))?;
    let db_key_hex = hex::encode(&db_key);
    let new_key_hex = hex::encode(&new_db_key);

    let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));
    let options = SqliteConnectOptions::from_str(&db_url)
        .map_err(|e| format!("Failed to parse DB URL: {}", e))?
        .pragma("key", format!("\"x'{}'\"", db_key_hex))
        .create_if_missing(false);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|e| format!("Failed to create database pool: {}", e))?;

    sqlx::query("SELECT 1 FROM sqlite_master LIMIT 1")
        .fetch_optional(&pool)
        .await
        .map_err(|_| "INVALID_MNEMONIC".to_string())?;

    sqlx::query(&format!("PRAGMA rekey = \"x'{}'\";", new_key_hex))
        .execute(&pool)
        .await
        .map_err(|e| format!("PRAGMA rekey failed: {}", e))?;

    pool.close().await;

    let kb_path_str = kb_root.to_string_lossy().to_string();
    {
        use std::path::PathBuf;
        let auth_dir = PathBuf::from(&kb_path_str).join(".insightcap");
        std::fs::create_dir_all(&auth_dir).map_err(|e| e.to_string())?;
        let auth_json = serde_json::json!({ "version": 1, "salt": hex::encode(&new_salt) });
        let auth_json_path = auth_dir.join("auth.json");
        let tmp = auth_json_path.with_extension("json.tmp");
        std::fs::write(&tmp, auth_json.to_string()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &auth_json_path).map_err(|e| e.to_string())?;
    }

    let new_mnemonic = generate_mnemonic();
    let (new_recovery_key, new_recovery_salt) = derive_recovery_key_new(&new_mnemonic)
        .map_err(|e| format!("Failed to create new recovery mnemonic: {}", e))?;
    let rec_bin_path = kb_root.join(".insightcap").join("recovery.bin");
    write_recovery_bin(
        &rec_bin_path,
        &new_db_key,
        &new_recovery_key,
        &new_recovery_salt,
    )
    .map_err(|e| format!("Failed to write recovery.bin: {}", e))?;

    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .map_err(|e| format!("Keychain access failed: {}", e))?
        .set_password(&new_key_hex)
        .map_err(|e| format!("Keychain write failed: {}", e))?;

    let _ = std::fs::remove_file(&backup_bin_path);

    db_key.zeroize();
    new_db_key.zeroize();

    println!(
        "[ImportKB] Imported from: {}. New recovery mnemonic generated. Restarting...",
        src_path
    );

    let _ = handle;
    Ok(new_mnemonic)
}

#[tauri::command]
pub async fn delete_kb(state: State<'_, AppState>) -> Result<(), String> {
    let db = &state.db;

    state.vector_store.clear().await?;

    let tables = [
        "chunk_relations",
        "decisions",
        "messages",
        "conversation_summary_queue",
        "memory_chunks",
        "captures",
        "conversations",
        "sources",
        "spaces",
        "projects",
        "tags",
        "inbox",
        "external_knowledge_bases",
    ];
    for table in &tables {
        let sql = format!("DELETE FROM {}", table);
        if let Err(e) = sqlx::query(&sql).execute(db).await {
            if e.to_string().contains("no such table") {
                println!("[DeleteKB] Table {} does not exist, skipping.", table);
                continue;
            }
            return Err(format!("Failed to clear table {}: {}", table, e));
        }
    }

    sqlx::query("VACUUM")
        .execute(db)
        .await
        .map_err(|e| format!("VACUUM failed: {}", e))?;

    let kb_root = &state.kb_path;
    for dir_name in &["files", "notes", ".insightcap/documents"] {
        let dir_path = kb_root.join(dir_name);
        if dir_path.exists() {
            std::fs::remove_dir_all(&dir_path)
                .map_err(|e| format!("Failed to delete {} directory: {}", dir_name, e))?;
            println!("[DeleteKB] Removed directory: {}", dir_path.display());
        }
    }

    println!("[DeleteKB] All knowledge data and files deleted.");
    Ok(())
}

#[tauri::command]
pub async fn repair_missing_local_copies(state: State<'_, AppState>) -> Result<String, String> {
    let db = &state.db;
    let kb_path = state.kb_path.to_string_lossy().to_string();

    let rows = sqlx::query(
        "SELECT id, file_path FROM sources \
         WHERE type = 'file' AND file_path IS NOT NULL AND file_path != '' \
         AND (local_doc_path IS NULL OR local_doc_path = '')",
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Query failed: {}", e))?;

    if rows.is_empty() {
        return Ok("No missing local copies need repair".to_string());
    }

    let files_dir = std::path::Path::new(&kb_path).join("files");
    std::fs::create_dir_all(&files_dir)
        .map_err(|e| format!("Failed to create files directory: {}", e))?;

    let mut copied = 0usize;
    let mut skipped = 0usize;

    for row in &rows {
        let source_id: String = row.try_get("id").unwrap_or_default();
        let file_path: String = row.try_get("file_path").unwrap_or_default();
        let src = std::path::Path::new(&file_path);

        if !src.exists() {
            skipped += 1;
            continue;
        }

        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let file_name = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.{}", &source_id[..8], ext));
        let dest = files_dir.join(format!("{}_{}", &source_id[..8], file_name));

        match std::fs::copy(src, &dest) {
            Ok(_) => {
                let local_path_str = dest.to_string_lossy().to_string();
                let now = chrono::Utc::now().to_rfc3339();
                let _ = sqlx::query(
                    "UPDATE sources SET local_doc_path = ?, updated_at = ? WHERE id = ?",
                )
                .bind(&local_path_str)
                .bind(&now)
                .bind(&source_id)
                .execute(db)
                .await;
                copied += 1;
                println!("[RepairCopies] copied {} -> {}", file_path, dest.display());
            }
            Err(e) => {
                eprintln!("[RepairCopies] copy failed {}: {}", file_path, e);
                skipped += 1;
            }
        }
    }

    let msg = format!(
        "Repair completed: copied {} files, skipped {} files",
        copied, skipped
    );
    println!("[RepairCopies] {}", msg);
    Ok(msg)
}

#[tauri::command]
pub async fn run_knowledge_stress_test<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let pool = &state.db;
    let now_utc = chrono::Utc::now();
    let now = now_utc.to_rfc3339();

    println!("[STRESS-TEST] Starting Knowledge Base Stress Test...");

    // 1. Data Injection: 1000 Sources, each with 5 chunks
    println!("[STRESS-TEST] Phase 1: Injecting 1,000 sources and 5,000 chunks...");
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    for i in 0..1000 {
        let source_id = uuid::Uuid::now_v7().to_string();
        let title = format!("Stress Test Source Entity #{}", i);
        let content = format!("This is a long synthetic content for stress testing source {}. It contains multiple paragraphs to simulate real world data distribution patterns in the RAG engine.", i);
        
        sqlx::query(
            "INSERT INTO sources (id, type, title, clean_content, captured_at, updated_at, capture_count, source_category, media_type) \
             VALUES (?, 'text', ?, ?, ?, ?, 5, 'captured', 'text')"
        )
        .bind(&source_id)
        .bind(&title)
        .bind(&content)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        for j in 0..5 {
            let chunk_id = uuid::Uuid::now_v7().to_string();
            let chunk_content = format!("Synthetic chunk {} for source {}. Vector search relevance testing data.", j, i);
            sqlx::query(
                "INSERT INTO captures (id, source_id, type, raw_content, clean_content, status, capture_method, chunk_index, created_at, updated_at) \
                 VALUES (?, ?, 'text', ?, ?, 'processed', 'source_import', ?, ?, ?)"
            )
            .bind(&chunk_id)
            .bind(&source_id)
            .bind(&chunk_content)
            .bind(&chunk_content)
            .bind(j as i64)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }

        if i % 200 == 0 {
            println!("[STRESS-TEST] Injected {}/1000 sources...", i);
        }
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    println!("[STRESS-TEST] Injection complete.");

    // 2. Rebuild Tags
    println!("[STRESS-TEST] Phase 2: Rebuilding Source Tags...");
    let tags_count = rebuild_source_tags(state.clone(), app.clone()).await?;
    println!("[STRESS-TEST] Tagged {} sources.", tags_count);

    // 3. Rebuild Index
    println!("[STRESS-TEST] Phase 3: Rebuilding Vector Index...");
    let index_count = rebuild_kb_index(state.clone()).await?;
    println!("[STRESS-TEST] Rebuilt {} vectors.", index_count);

    // 4. Verification Check
    let total_chunks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM captures WHERE source_id IN (SELECT id FROM sources WHERE title LIKE 'Stress Test Source%')")
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    let result = format!(
        "STRESS TEST SUCCESSFUL\n- Injected Chunks: {}\n- Tags Rebuilt: {}\n- Vectors Rebuilt: {}\n- Verified in DB: {}",
        5000, tags_count, index_count, total_chunks
    );
    println!("[STRESS-TEST] Done.");
    Ok(result)
}
