use sqlx::{Row, SqlitePool};
use tauri::{Emitter, State};
use crate::db::AppState;

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

/// 取得來源列表（以 sources 為單位，可按 space_id 篩選）
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
             ORDER BY s.captured_at DESC LIMIT ?"
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
             ORDER BY captured_at DESC LIMIT ?"
        )
        .bind(max)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?
    };

    let sources = rows.into_iter().map(|r| SourceItem {
        id: r.try_get("id").unwrap_or_default(),
        title: r.get("title"),
        r#type: r.get("type"),
        url: r.try_get("url").unwrap_or(None),
        file_path: r.try_get("file_path").unwrap_or(None),
        captured_at: r.get("captured_at"),
        content_preview: r.try_get("preview").unwrap_or_default(),
    }).collect();

    Ok(sources)
}

/// 取得 captures 列表（可按 source_id 或 status 篩選）
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
             ORDER BY created_at DESC LIMIT ?"
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
             ORDER BY created_at DESC LIMIT ?"
        )
        .bind(s)
        .bind(max)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?
    };

    let captures = rows.into_iter().map(|r| CaptureItem {
        id: r.try_get("id").unwrap_or_default(),
        source_id: r.try_get("source_id").unwrap_or(None),
        clean_content: r.get("clean_content"),
        r#type: r.get("type"),
        status: r.try_get("status").unwrap_or_default(),
        created_at: r.get("created_at"),
    }).collect();

    Ok(captures)
}

// ── Timeline 查詢 ──────────────────────────────────────

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
    pub captured_at: String,
    pub content_preview: String,
    pub capture_count: i64,
    pub tags: Vec<String>,
}

/// 透過 title 取得來源預覽
#[tauri::command]
pub async fn get_source_preview_by_title(
    pool: State<'_, SqlitePool>,
    title: String,
) -> Result<String, String> {
    let preview: Option<String> = sqlx::query_scalar(
        "SELECT SUBSTR(clean_content, 1, 500) FROM sources WHERE title = ? LIMIT 1"
    )
    .bind(title)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(preview.unwrap_or_else(|| "無法取得來源預覽。".to_string()))
}

/// 取得 Timeline 來源列表（按日期排序，可按 category / media_type 篩選）
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

    // 先偵測 sources 表是否有 006 migration 的欄位
    let has_new_cols: bool = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*) FROM pragma_table_info('sources') WHERE name = 'source_category'"
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(0) > 0;

    let mut sql = if has_new_cols {
        String::from(
            "SELECT s.id, s.title, s.type, \
             COALESCE(s.source_category, 'captured') as source_category, \
             COALESCE(s.media_type, 'text') as media_type, \
             s.url, s.file_path, s.local_doc_path, s.captured_at, \
             SUBSTR(s.clean_content, 1, 120) as preview, \
             s.capture_count, \
             COALESCE((SELECT GROUP_CONCAT(DISTINCT t.value) FROM captures c, json_each(c.tags) t WHERE c.source_id = s.id AND t.value != ''), '') as agg_tags \
             FROM sources s WHERE 1=1"
        )
    } else {
        String::from(
            "SELECT s.id, s.title, s.type, \
             'captured' as source_category, \
             'text' as media_type, \
             s.url, s.file_path, NULL as local_doc_path, s.captured_at, \
             SUBSTR(s.clean_content, 1, 120) as preview, \
             s.capture_count, \
             COALESCE((SELECT GROUP_CONCAT(DISTINCT t.value) FROM captures c, json_each(c.tags) t WHERE c.source_id = s.id AND t.value != ''), '') as agg_tags \
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
        sql.push_str(" AND EXISTS (SELECT 1 FROM captures c2 WHERE c2.source_id = s.id AND c2.space_id = ?)");
        binds.push(sid.clone());
    }

    sql.push_str(" ORDER BY s.captured_at DESC LIMIT ? OFFSET ?");

    let mut query = sqlx::query(&sql);
    for b in &binds {
        query = query.bind(b);
    }
    query = query.bind(max).bind(off);

    let rows = query.fetch_all(pool.inner()).await.map_err(|e| e.to_string())?;

    let sources = rows.into_iter().map(|r| {
        let agg: String = r.try_get("agg_tags").unwrap_or_default();
        let tags: Vec<String> = if agg.is_empty() {
            Vec::new()
        } else {
            agg.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        };
        TimelineSourceItem {
            id: r.try_get("id").unwrap_or_default(),
            title: r.get("title"),
            r#type: r.get("type"),
            source_category: r.try_get("source_category").unwrap_or_else(|_| "captured".to_string()),
            media_type: r.try_get("media_type").unwrap_or_else(|_| "text".to_string()),
            url: r.try_get("url").unwrap_or(None),
            file_path: r.try_get("file_path").unwrap_or(None),
            local_doc_path: r.try_get("local_doc_path").unwrap_or(None),
            captured_at: r.get("captured_at"),
            content_preview: r.try_get("preview").unwrap_or_default(),
            capture_count: r.try_get("capture_count").unwrap_or(0),
            tags,
        }
    }).collect();

    Ok(sources)
}

// ── Editor Document CRUD ───────────────────────────────

/// 建立一份編輯器文件（在 sources 中建記錄 + 在磁碟建 .md）
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
        captured_at: now,
        content_preview: String::new(),
        capture_count: 0,
        tags: Vec::new(),
    })
}

/// 讀取編輯器文件內容（從磁碟）
#[tauri::command]
pub async fn read_editor_document(
    pool: State<'_, SqlitePool>,
    source_id: String,
) -> Result<String, String> {
    let row = sqlx::query("SELECT local_doc_path FROM sources WHERE id = ? AND source_category = 'editor_doc'")
        .bind(&source_id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Document not found".to_string())?;

    let path: String = row.try_get("local_doc_path")
        .map_err(|_| "No local_doc_path set".to_string())?;

    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// 儲存編輯器文件內容（寫回磁碟 + 更新 sources.updated_at）
#[tauri::command]
pub async fn save_editor_document(
    pool: State<'_, SqlitePool>,
    source_id: String,
    content: String,
) -> Result<(), String> {
    let row = sqlx::query("SELECT local_doc_path FROM sources WHERE id = ? AND source_category = 'editor_doc'")
        .bind(&source_id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Document not found".to_string())?;

    let path: String = row.try_get("local_doc_path")
        .map_err(|_| "No local_doc_path set".to_string())?;

    std::fs::write(&path, &content).map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE sources SET clean_content = SUBSTR(?, 1, 500), updated_at = ? WHERE id = ?")
        .bind(&content)
        .bind(&now)
        .bind(&source_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ── Source 刪除 ────────────────────────────────────────

/// 刪除 source（cascade 刪除 captures；若為 editor_doc 同時刪除磁碟檔案）
#[tauri::command]
pub async fn delete_source(
    pool: State<'_, SqlitePool>,
    source_id: String,
) -> Result<(), String> {
    // 偵測是否有 migration 006 欄位
    let has_new_cols: bool = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*) FROM pragma_table_info('sources') WHERE name = 'source_category'"
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(0) > 0;

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

// ── Capture（Chunk）CRUD ───────────────────────────────

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

/// 取得某 source 下的全部 captures（含 tags、space_id）
#[tauri::command]
pub async fn get_captures_detail(
    pool: State<'_, SqlitePool>,
    source_id: String,
) -> Result<Vec<CaptureDetail>, String> {
    let has_user_edited: bool = sqlx::query_scalar::<_, i32>(
        "SELECT COUNT(*) FROM pragma_table_info('captures') WHERE name = 'is_user_edited'"
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(0) > 0;

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

    let captures = rows.into_iter().map(|r| CaptureDetail {
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
    }).collect();

    Ok(captures)
}

/// 建立手動 capture（獨立 chunk，可不屬於任何 source）
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
        "SELECT COUNT(*) FROM pragma_table_info('captures') WHERE name = 'is_user_edited'"
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(0) > 0;

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

    // 更新 source capture_count
    if let Some(ref sid) = source_id {
        sqlx::query("UPDATE sources SET capture_count = capture_count + 1, updated_at = ? WHERE id = ?")
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

/// 更新 capture 內容、tags、space
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
        "SELECT COUNT(*) FROM pragma_table_info('captures') WHERE name = 'is_user_edited'"
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(0) > 0;

    let mut sets = if has_user_edited_col {
        vec!["updated_at = ?".to_string(), "is_user_edited = 1".to_string()]
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
    query.execute(pool.inner()).await.map_err(|e| e.to_string())?;

    Ok(())
}

/// 刪除 capture
#[tauri::command]
pub async fn delete_capture(
    pool: State<'_, SqlitePool>,
    capture_id: String,
) -> Result<(), String> {
    // 取得 source_id 以便更新 count
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

/// 處理源文件：將 source 的文件解析成 chunks，產生 embeddings，寫入 captures 表
#[tauri::command]
pub async fn process_source(
    state: State<'_, AppState>,
    source_id: String,
) -> Result<usize, String> {
    let db = &state.db;
    
    // 1. 從 sources 表取得 file_path 和 title
    let source_row = sqlx::query(
        "SELECT id, file_path, title FROM sources WHERE id = ?"
    )
    .bind(&source_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("Source not found: {}", source_id))?;
    
    let file_path: String = source_row.get("file_path");
    let _title: String = source_row.get("title");
    
    // 2. 取得 kb_path
    let kb_path = {
        let settings = crate::settings::store::get_settings(db).await.map_err(|e| e.to_string())?;
        settings.knowledge.kb_path
    };
    
    // 3. 解析文件
    let parsed = crate::capture::file_parser::parse_file(&kb_path, &file_path, None).await?;
    
    let mut chunk_count: usize = 0;
    let now = chrono::Utc::now().to_rfc3339();
    
    // 4. 對每個 chunk 進行段落切分（與 CaptureProcessor / create_temp_chunk 一致）
    for f_chunk in parsed.chunks {
        let paragraphs: Vec<String> = f_chunk.content
            .split("\n\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        let paragraphs = if paragraphs.is_empty() {
            if f_chunk.content.trim().is_empty() { continue; }
            vec![f_chunk.content.clone()]
        } else {
            paragraphs
        };
        
        // 5. 對每個段落生成 embedding 並寫入 captures
        for (idx, para) in paragraphs.into_iter().enumerate() {
            let chunk_id = uuid::Uuid::now_v7().to_string();
            
            let mut vector_id = 0i64;
            
            // 只有當 status 不是 pending_ocr 時，才產生 embedding 與標籤
            if f_chunk.status != "pending_ocr" {
                // 產生 embedding
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
                 capture_method, chunk_index, status, vector_id, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, 'source_import', ?, ?, ?, ?, ?)"
            )
            .bind(&chunk_id)
            .bind(&source_id)
            .bind(&f_chunk.chunk_type)
            .bind(&para)
            .bind(&para)
            .bind(idx as i64)
            .bind(&f_chunk.status)
            .bind(vector_id)
            .bind(&now)
            .bind(&now)
            .execute(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?;

            if f_chunk.status != "pending_ocr" {
                // Tagger：非同步提取標籤（不阻塞主流程）
                let tag_pool = db.clone();
                let tag_cid = chunk_id.clone();
                let tag_content = para.clone();
                tokio::spawn(async move {
                    let tag_engine = crate::services::tag_engine::TagEngine::new(tag_pool);
                    if let Err(e) = tag_engine.process_new_capture(&tag_cid, &tag_content).await {
                        eprintln!("[ProcessSource] Tag generation failed for {}: {}", &tag_cid[..8.min(tag_cid.len())], e);
                    }
                });
            }
            
            chunk_count += 1;
        }
    }
    
    // 6. 更新 source 的 capture_count
    sqlx::query("UPDATE sources SET capture_count = ?, updated_at = ? WHERE id = ?")
        .bind(chunk_count as i64)
        .bind(&now)
        .bind(&source_id)
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;
    
    // 7. 非同步儲存向量索引到磁碟
    let vs = state.vector_store.clone();
    tokio::spawn(async move { let _ = vs.save().await; });
    
    println!("[ProcessSource] {} chunks generated for source: {}", chunk_count, &source_id[..8.min(source_id.len())]);
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

/// 取得儲存庫統計數字
#[tauri::command]
pub async fn get_repository_stats(
    state: State<'_, AppState>,
) -> Result<RepositoryStats, String> {
    let db = &state.db;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let today_sources: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sources WHERE captured_at >= ?"
    )
    .bind(format!("{}T00:00:00", today))
    .fetch_one(db).await.unwrap_or(0);

    let total_chunks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM captures WHERE capture_method != 'temp_attachment' AND status != 'archived'"
    )
    .fetch_one(db).await.unwrap_or(0);

    let total_data: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memory_chunks WHERE knowledge_type = 'data'"
    )
    .fetch_one(db).await.unwrap_or(0);

    let total_patterns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memory_chunks WHERE knowledge_type = 'pattern'"
    )
    .fetch_one(db).await.unwrap_or(0);

    let total_logs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memory_chunks WHERE knowledge_type = 'log'"
    )
    .fetch_one(db).await.unwrap_or(0);

    Ok(RepositoryStats {
        today_sources,
        total_chunks,
        total_data,
        total_patterns,
        total_logs,
    })
}

// ─── KB Maintenance Commands ────────────────────────────────────────────────

/// 重建知識庫向量索引：清除現有向量 → 重新對所有 captures 產生 embedding
#[tauri::command]
pub async fn rebuild_kb_index(
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let db = &state.db;

    // 1. 清空向量儲存
    state.vector_store.clear().await?;
    println!("[RebuildIndex] Vector store cleared.");

    // 2. 取得所有需要重建的 captures
    let rows = sqlx::query(
        "SELECT id, clean_content FROM captures \
         WHERE status != 'archived' AND capture_method != 'temp_attachment' \
         ORDER BY created_at ASC"
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

        let vec = state.embedder.embed(&content).await.map_err(|e| e.to_string())?;
        let vector_id = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            id.hash(&mut hasher);
            hasher.finish()
        };

        state.vector_store.add_vector(vector_id, &vec).await
            .map_err(|e| format!("vector store error: {}", e))?;

        // 同步更新 captures 的 vector_id
        sqlx::query("UPDATE captures SET vector_id = ? WHERE id = ?")
            .bind(vector_id as i64)
            .bind(&id)
            .execute(db)
            .await
            .map_err(|e| e.to_string())?;

        count += 1;
    }

    // 3. 重建 memory_chunks 向量（三層記憶系統也使用同一個 VectorStore）
    let mc_rows = sqlx::query(
        "SELECT id, content, vector_id FROM memory_chunks \
         WHERE content IS NOT NULL AND content != '' \
         AND pending_confirm = 0 \
         ORDER BY created_at ASC"
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;

    println!("[RebuildIndex] Re-embedding {} memory_chunks...", mc_rows.len());

    for row in &mc_rows {
        let mc_id: String = row.get("id");
        let content: String = row.get("content");
        let existing_vid: Option<i64> = row.get("vector_id");

        if content.trim().is_empty() {
            continue;
        }

        let vec = state.embedder.embed(&content).await.map_err(|e| e.to_string())?;

        // 優先沿用原始 vector_id，無則用 hash(id) 產生新的
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

        state.vector_store.add_vector(vector_id, &vec).await
            .map_err(|e| format!("vector store error: {}", e))?;

        sqlx::query("UPDATE memory_chunks SET vector_id = ? WHERE id = ?")
            .bind(vector_id as i64)
            .bind(&mc_id)
            .execute(db)
            .await
            .map_err(|e| e.to_string())?;

        count += 1;
    }

    // 4. 儲存索引到磁碟
    state.vector_store.save().await?;
    println!("[RebuildIndex] Done. {} vectors rebuilt (captures + memory_chunks).", count);
    Ok(count)
}

/// 對所有歷史 sources 重新生成 Source 層級標籤（帶進度回報，串行）
/// 只處理 tags 為空或 '[]' 的 source，跳過已有標籤者
#[tauri::command]
pub async fn rebuild_source_tags(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<usize, String> {
    let db = &state.db;

    // 只取尚未有標籤的 source（tags 為 NULL、'[]' 或空字串）
    let rows = sqlx::query(
        "SELECT id, clean_content FROM sources \
         WHERE (tags IS NULL OR tags = '[]' OR tags = '') \
         AND (clean_content IS NOT NULL AND clean_content != '') \
         ORDER BY captured_at ASC"
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;

    let total = rows.len();
    println!("[RebuildSourceTags] 需要處理 {} 個 source（無標籤）", total);

    let _ = app.emit("rebuild-tags-progress", serde_json::json!({
        "current": 0, "total": total, "done": false
    }));

    if total == 0 {
        let _ = app.emit("rebuild-tags-progress", serde_json::json!({
            "current": 0, "total": 0, "done": true
        }));
        return Ok(0);
    }

    let mut count: usize = 0;
    for (i, row) in rows.iter().enumerate() {
        let id: String = row.get("id");
        let content: String = row.get("clean_content");

        let tag_engine = crate::services::tag_engine::TagEngine::new(db.clone());
        match tag_engine.process_source(&id, &content).await {
            Ok(_) => { count += 1; }
            Err(e) => {
                eprintln!("[RebuildSourceTags] source {} 失敗: {}", &id[..8.min(id.len())], e);
            }
        }

        let _ = app.emit("rebuild-tags-progress", serde_json::json!({
            "current": i + 1, "total": total, "done": false
        }));
    }

    let _ = app.emit("rebuild-tags-progress", serde_json::json!({
        "current": total, "total": total, "done": true
    }));

    println!("[RebuildSourceTags] 完成，成功 {}/{} 個 source", count, total);
    Ok(count)
}

/// 匯出知識庫：將整個 KB 目錄（.insightcap/ + files/ + notes/）壓縮為 .zip
/// 匯出知識庫：加密 DB 原樣打包，另附 backup_recovery.bin（用備份恢復碼加密的 db_key 快照）
/// mnemonic：前端生成並讓用戶確認保存的 24-word 備份專用恢復碼
#[tauri::command]
pub async fn export_kb(
    state: State<'_, AppState>,
    dest_path: String,
    mnemonic: String,
) -> Result<(), String> {
    use std::io::Write;
    use crate::auth::key_derivation::derive_recovery_key_new;
    use crate::auth::recovery::write_recovery_bin;

    // 1. WAL checkpoint 確保 DB 資料完整
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&state.db)
        .await
        .map_err(|e| format!("WAL checkpoint failed: {}", e))?;

    // 2. 從 Keyring 讀取當前 db_key
    let key_hex = keyring::Entry::new("insightcap", "auto_login_key")
        .map_err(|e| format!("Keyring 存取失敗: {}", e))?
        .get_password()
        .map_err(|_| "無法取得 db_key，請確認已登入自動登入模式".to_string())?;
    let key_bytes = hex::decode(&key_hex)
        .map_err(|_| "db_key 格式錯誤".to_string())?;
    if key_bytes.len() != 32 {
        return Err("db_key 長度錯誤".to_string());
    }
    let mut db_key = [0u8; 32];
    db_key.copy_from_slice(&key_bytes);

    // 3. 用備份恢復碼衍生 backup_recovery_key，生成 backup_recovery.bin bytes（in-memory）
    let (backup_recovery_key, salt) = derive_recovery_key_new(&mnemonic)
        .map_err(|e| format!("恢復碼處理失敗: {}", e))?;

    // 寫到臨時路徑再讀回（write_recovery_bin 使用原子寫入），完成後刪除
    let kb_root = &state.kb_path;
    let tmp_bin = kb_root.join(".insightcap").join("backup_recovery_tmp.bin");
    write_recovery_bin(&tmp_bin, &db_key, &backup_recovery_key, &salt)
        .map_err(|e| format!("生成 backup_recovery.bin 失敗: {}", e))?;
    let backup_bin_data = std::fs::read(&tmp_bin)
        .map_err(|e| format!("讀取 backup_recovery.bin 失敗: {}", e))?;
    let _ = std::fs::remove_file(&tmp_bin);

    // 4. 打包 zip：加密 DB 原樣 + auth.json + backup_recovery.bin（in-memory）+ files/ + notes/
    let zip_result = (|| -> Result<(), String> {
        let zip_file = std::fs::File::create(&dest_path)
            .map_err(|e| format!("無法建立匯出檔案: {}", e))?;
        let mut zip = zip::ZipWriter::new(zip_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        // .insightcap/ 目錄：加密 DB 原樣，跳過 WAL/SHM，注入 backup_recovery.bin
        let insightcap_dir = kb_root.join(".insightcap");
        let walker = walkdir::WalkDir::new(&insightcap_dir).follow_links(false);
        for entry in walker {
            let entry = entry.map_err(|e| format!("走訪目錄失敗: {}", e))?;
            let abs_path = entry.path();
            let rel_path = abs_path.strip_prefix(kb_root)
                .map_err(|_| "路徑前綴錯誤".to_string())?
                .to_string_lossy()
                .replace('\\', "/");

            if abs_path.is_dir() {
                zip.add_directory(&format!("{}/", rel_path), options)
                    .map_err(|e| format!("加入目錄失敗: {}", e))?;
                continue;
            }
            // 跳過 WAL/SHM 暫存檔、舊的 backup_recovery.bin（由 in-memory 版本取代）
            if let Some(name) = abs_path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with("-wal") || name.ends_with("-shm")
                    || name == "backup_recovery.bin" {
                    continue;
                }
            }
            zip.start_file(&rel_path, options)
                .map_err(|e| format!("加入檔案失敗: {}", e))?;
            let data = std::fs::read(abs_path)
                .map_err(|e| format!("讀取檔案失敗 {}: {}", rel_path, e))?;
            zip.write_all(&data)
                .map_err(|e| format!("寫入 zip 失敗: {}", e))?;
        }
        // 注入本次生成的 backup_recovery.bin
        zip.start_file(".insightcap/backup_recovery.bin", options)
            .map_err(|e| format!("加入 backup_recovery.bin 失敗: {}", e))?;
        zip.write_all(&backup_bin_data)
            .map_err(|e| format!("寫入 backup_recovery.bin 失敗: {}", e))?;

        // files/ 和 notes/ 原樣打包
        for dir_name in &["files", "notes"] {
            let dir_path = kb_root.join(dir_name);
            if !dir_path.exists() {
                continue;
            }
            let walker = walkdir::WalkDir::new(&dir_path).follow_links(false);
            for entry in walker {
                let entry = entry.map_err(|e| format!("走訪目錄失敗: {}", e))?;
                let abs_path = entry.path();
                let rel_path = abs_path.strip_prefix(kb_root)
                    .map_err(|_| "路徑前綴錯誤".to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");

                if abs_path.is_dir() {
                    zip.add_directory(&format!("{}/", rel_path), options)
                        .map_err(|e| format!("加入目錄失敗: {}", e))?;
                } else {
                    zip.start_file(&rel_path, options)
                        .map_err(|e| format!("加入檔案失敗: {}", e))?;
                    let data = std::fs::read(abs_path)
                        .map_err(|e| format!("讀取檔案失敗 {}: {}", rel_path, e))?;
                    zip.write_all(&data)
                        .map_err(|e| format!("寫入 zip 失敗: {}", e))?;
                }
            }
        }

        zip.finish().map_err(|e| format!("zip 完成失敗: {}", e))?;
        Ok(())
    })();

    zip_result?;
    println!("[ExportKB] Exported KB to: {}", dest_path);
    Ok(())
}

/// 匯入知識庫：
/// 1. 驗證 ZIP → 解壓到 kb_root
/// 2. 用備份恢復碼解密 backup_recovery.bin → 取得原始 db_key
/// 3. 用原始 db_key 開啟加密 DB，PRAGMA rekey 換成新密碼衍生的 new_db_key
/// 4. 寫新 auth.json、生成新日常 recovery.bin、更新 Keychain
/// 5. 重啟
#[tauri::command]
pub async fn import_kb(
    handle: tauri::AppHandle,
    state: State<'_, AppState>,
    src_path: String,
    mnemonic: String,
    new_password: String,
) -> Result<String, String> {
    use std::io::Read;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use zeroize::Zeroize;
    use rand::Rng;
    use crate::auth::key_derivation::{derive_db_key, derive_recovery_key_new, derive_recovery_key_verify, generate_mnemonic};
    use crate::auth::recovery::{read_recovery_bin, write_recovery_bin};
    use keyring::Entry;

    const KEYCHAIN_SERVICE: &str = "insightcap";
    const KEYCHAIN_AUTO_LOGIN: &str = "auto_login_key";

    let src = std::path::Path::new(&src_path);
    if !src.exists() {
        return Err("來源檔案不存在".to_string());
    }

    // 驗證是 ZIP 檔案（magic bytes: PK\x03\x04）
    let header = {
        let mut f = std::fs::File::open(src).map_err(|e| format!("讀取失敗: {}", e))?;
        let mut buf = [0u8; 4];
        use std::io::Read as _;
        f.read_exact(&mut buf).map_err(|_| "檔案太短".to_string())?;
        buf
    };
    if &header != b"PK\x03\x04" {
        return Err("無效的知識庫封包（非 .zip 格式）".to_string());
    }

    let kb_root = &state.kb_path;
    let db_path = kb_root.join(".insightcap").join("insightcap.db");
    let backup_bin_path = kb_root.join(".insightcap").join("backup_recovery.bin");

    // 步驟 0：關閉現有 DB 連線池，釋放檔案鎖定（Windows 必需，否則覆蓋 DB 會 Access Denied）
    state.db.close().await;

    // 步驟 1：備份現有 DB，解壓 zip 到 kb_root
    if db_path.exists() {
        let bak = db_path.with_extension("db.bak");
        std::fs::copy(&db_path, &bak)
            .map_err(|e| format!("備份現有 DB 失敗: {}", e))?;
    }

    let zip_file = std::fs::File::open(src)
        .map_err(|e| format!("開啟封包失敗: {}", e))?;
    let mut archive = zip::ZipArchive::new(zip_file)
        .map_err(|e| format!("解析封包失敗: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("讀取封包項目失敗: {}", e))?;

        // Zip Slip 防護：拒絕含路徑穿越或絕對路徑的條目
        let entry_name = file.name().to_string();
        if entry_name.contains("..") || entry_name.starts_with('/') || entry_name.starts_with('\\') {
            return Err(format!("不安全的封包路徑: {}", entry_name));
        }
        let out_path = kb_root.join(&entry_name);
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("建立目錄失敗: {}", e))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("建立父目錄失敗: {}", e))?;
            }
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .map_err(|e| format!("讀取封包內容失敗: {}", e))?;
            std::fs::write(&out_path, &buf)
                .map_err(|e| format!("寫入檔案失敗: {}", e))?;
        }
    }

    // 步驟 2：讀取 backup_recovery.bin，用備份恢復碼解密取得原始 db_key
    if !backup_bin_path.exists() {
        return Err("封包中缺少 backup_recovery.bin，無法還原".to_string());
    }
    let bin_data = std::fs::read(&backup_bin_path)
        .map_err(|e| format!("讀取 backup_recovery.bin 失敗: {}", e))?;
    if bin_data.len() < 17 {
        return Err("backup_recovery.bin 格式錯誤".to_string());
    }
    let mut stored_salt = [0u8; 16];
    stored_salt.copy_from_slice(&bin_data[1..17]);

    let backup_recovery_key = derive_recovery_key_verify(&mnemonic, &stored_salt)
        .map_err(|_| "INVALID_MNEMONIC".to_string())?;
    let (mut db_key, _) = read_recovery_bin(&backup_bin_path, &backup_recovery_key)
        .map_err(|_| "INVALID_MNEMONIC".to_string())?;

    // 步驟 3：用原始 db_key 開啟加密 DB，PRAGMA rekey 換成新密碼的 new_db_key
    let mut new_salt = [0u8; 32];
    rand::rng().fill_bytes(&mut new_salt);
    let mut new_db_key = derive_db_key(&new_password, &new_salt)
        .map_err(|e| format!("新密碼 key 衍生失敗: {}", e))?;
    let db_key_hex = hex::encode(&db_key);
    let new_key_hex = hex::encode(&new_db_key);

    let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));
    let options = SqliteConnectOptions::from_str(&db_url)
        .map_err(|e| format!("DB URL 解析失敗: {}", e))?
        .pragma("key", format!("\"x'{}'\"", db_key_hex))
        .create_if_missing(false);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|e| format!("無法建立連接池: {}", e))?;

    // 驗證金鑰：執行一個簡單查詢觸發解密
    sqlx::query("SELECT 1 FROM sqlite_master LIMIT 1")
        .fetch_optional(&pool)
        .await
        .map_err(|_| "INVALID_MNEMONIC".to_string())?; // 解密失敗表示恢復碼不符

    sqlx::query(&format!("PRAGMA rekey = \"x'{}'\";", new_key_hex))
        .execute(&pool)
        .await
        .map_err(|e| format!("PRAGMA rekey 失敗: {}", e))?;

    pool.close().await;

    // 步驟 4：寫新 auth.json（原子寫入）
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

    // 生成新日常 recovery.bin
    let new_mnemonic = generate_mnemonic();
    let (new_recovery_key, new_recovery_salt) = derive_recovery_key_new(&new_mnemonic)
        .map_err(|e| format!("生成新恢復碼失敗: {}", e))?;
    let rec_bin_path = kb_root.join(".insightcap").join("recovery.bin");
    write_recovery_bin(&rec_bin_path, &new_db_key, &new_recovery_key, &new_recovery_salt)
        .map_err(|e| format!("寫入 recovery.bin 失敗: {}", e))?;

    // 更新 Keychain（auto_login_key 設為新 db_key）
    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .map_err(|e| format!("Keychain 存取失敗: {}", e))?
        .set_password(&new_key_hex)
        .map_err(|e| format!("Keychain 寫入失敗: {}", e))?;

    // 清理已失效的 backup_recovery.bin（DB 已 rekey，該檔案不再有用且含敏感 nonce/salt）
    let _ = std::fs::remove_file(&backup_bin_path);

    db_key.zeroize();
    new_db_key.zeroize();

    println!("[ImportKB] Imported from: {}. New recovery mnemonic generated. Restarting...", src_path);

    // 步驟 5：重啟（重啟後 init_db 用 Keychain 的 new_db_key 開啟 DB）
    // 回傳新恢復碼前先重啟—前端需在重啟前顯示新恢復碼，因此改為回傳 mnemonic，由前端決定重啟時機
    // 步驟 5：回傳新恢復碼讓前端顯示，前端確認保存後呼叫 restart_app 重啟
    // 與 recover_with_mnemonic 一致的模式
    let _ = handle; // 重啟由前端呼叫 restart_app 完成
    Ok(new_mnemonic)
}

/// 刪除知識庫：清除所有 DB 資料、向量索引、以及磁碟上的 files/ notes/ 目錄
#[tauri::command]
pub async fn delete_kb(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = &state.db;

    // 1. 清空向量索引
    state.vector_store.clear().await?;

    // 2. 清空主要資料表（排除 settings 以保留使用者偏好）
    //    順序：先刪子表再刪父表，避免 FOREIGN KEY constraint 錯誤
    let tables = [
        "chunk_relations", "decisions",
        "messages", "conversation_summary_queue",
        "memory_chunks", "captures",
        "conversations", "sources", "spaces", "projects", "tags",
        "inbox", "external_knowledge_bases",
    ];
    for table in &tables {
        let sql = format!("DELETE FROM {}", table);
        if let Err(e) = sqlx::query(&sql).execute(db).await {
            if e.to_string().contains("no such table") {
                println!("[DeleteKB] Table {} does not exist, skipping.", table);
                continue;
            }
            return Err(format!("清除 {} 失敗: {}", table, e));
        }
    }

    // 3. VACUUM 回收空間
    sqlx::query("VACUUM")
        .execute(db)
        .await
        .map_err(|e| format!("VACUUM failed: {}", e))?;

    // 4. 刪除磁碟上的來源文件、筆記、編輯器文件目錄
    let kb_root = &state.kb_path;
    for dir_name in &["files", "notes", ".insightcap/documents"] {
        let dir_path = kb_root.join(dir_name);
        if dir_path.exists() {
            std::fs::remove_dir_all(&dir_path)
                .map_err(|e| format!("刪除 {} 目錄失敗: {}", dir_name, e))?;
            println!("[DeleteKB] Removed directory: {}", dir_path.display());
        }
    }

    println!("[DeleteKB] All knowledge data and files deleted.");
    Ok(())
}

/// 修復歷史資料：掃描所有 file_path 有效但 local_doc_path 為空的 sources，
/// 將原始檔案複製到 kb_path/files/ 並更新 local_doc_path
#[tauri::command]
pub async fn repair_missing_local_copies(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let db = &state.db;
    let kb_path = state.kb_path.to_string_lossy().to_string();

    // 查出所有 file_path 有值、local_doc_path 為空、且 type = 'file' 的 sources
    let rows = sqlx::query(
        "SELECT id, file_path FROM sources \
         WHERE type = 'file' AND file_path IS NOT NULL AND file_path != '' \
         AND (local_doc_path IS NULL OR local_doc_path = '')"
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("查詢失敗: {}", e))?;

    if rows.is_empty() {
        return Ok("無需修復的歷史資料".to_string());
    }

    let files_dir = std::path::Path::new(&kb_path).join("files");
    std::fs::create_dir_all(&files_dir)
        .map_err(|e| format!("無法建立 files 目錄: {}", e))?;

    let mut copied = 0usize;
    let mut skipped = 0usize;

    for row in &rows {
        let source_id: String = row.try_get("id").unwrap_or_default();
        let file_path: String = row.try_get("file_path").unwrap_or_default();
        let src = std::path::Path::new(&file_path);

        // 原始檔案已不存在，跳過
        if !src.exists() {
            skipped += 1;
            continue;
        }

        let ext = src.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let file_name = src.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.{}", &source_id[..8], ext));
        let dest = files_dir.join(format!("{}_{}", &source_id[..8], file_name));

        match std::fs::copy(src, &dest) {
            Ok(_) => {
                let local_path_str = dest.to_string_lossy().to_string();
                let now = chrono::Utc::now().to_rfc3339();
                let _ = sqlx::query(
                    "UPDATE sources SET local_doc_path = ?, updated_at = ? WHERE id = ?"
                )
                .bind(&local_path_str)
                .bind(&now)
                .bind(&source_id)
                .execute(db)
                .await;
                copied += 1;
                println!("[RepairCopies] ✅ {} → {}", file_path, dest.display());
            }
            Err(e) => {
                eprintln!("[RepairCopies] ❌ 複製失敗 {}: {}", file_path, e);
                skipped += 1;
            }
        }
    }

    let msg = format!("修復完成：已複製 {} 個檔案，跳過 {} 個（原始檔案不存在或複製失敗）", copied, skipped);
    println!("[RepairCopies] {}", msg);
    Ok(msg)
}
