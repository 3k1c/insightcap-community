use crate::db::AppState;
use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tauri::State;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub chunk_count: i64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceInsight {
    pub space_id: String,
    pub space_name: String,
    pub total_chunks: i64,
    pub data_count: i64,
    pub pattern_count: i64,
    pub log_count: i64,
    pub capture_count: i64,
    pub top_tags: Vec<TagStat>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagStat {
    pub tag: String,
    pub count: i64,
}

#[tauri::command]
pub async fn get_all_spaces(pool: State<'_, SqlitePool>) -> Result<Vec<SpaceItem>, String> {
    let rows = sqlx::query(
        "SELECT id, name, description, chunk_count FROM spaces WHERE is_archived = 0 ORDER BY chunk_count DESC, created_at DESC"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let spaces = rows
        .into_iter()
        .map(|r| SpaceItem {
            id: r.get("id"),
            name: r.get("name"),
            description: r.try_get("description").unwrap_or_default(),
            chunk_count: r.try_get("chunk_count").unwrap_or(0),
        })
        .collect();

    Ok(spaces)
}

/// 取得指定 Space 的知識可用性洞察：memory_chunks 按 knowledge_type 分佈 + 標籤統計
#[tauri::command]
pub async fn get_space_insight(
    pool: State<'_, SqlitePool>,
    space_id: String,
) -> Result<SpaceInsight, String> {
    // Space 基本資訊
    let space_row = sqlx::query("SELECT name FROM spaces WHERE id = ?")
        .bind(&space_id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Space not found".to_string())?;
    let space_name: String = space_row.get("name");

    // memory_chunks 按 knowledge_type 統計
    let mc_rows = sqlx::query(
        "SELECT knowledge_type, COUNT(*) as cnt FROM memory_chunks WHERE space_id = ? GROUP BY knowledge_type"
    )
    .bind(&space_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let mut data_count: i64 = 0;
    let mut pattern_count: i64 = 0;
    let mut log_count: i64 = 0;
    for r in &mc_rows {
        let kt: String = r.try_get("knowledge_type").unwrap_or_default();
        let cnt: i64 = r.try_get("cnt").unwrap_or(0);
        match kt.as_str() {
            "data" => data_count = cnt,
            "pattern" => pattern_count = cnt,
            "log" => log_count = cnt,
            _ => {}
        }
    }

    // captures 統計（屬於此 Space 的擷取）
    let cap_row = sqlx::query("SELECT COUNT(*) as cnt FROM captures WHERE space_id = ?")
        .bind(&space_id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    let capture_count: i64 = cap_row.try_get("cnt").unwrap_or(0);

    // 標籤統計：合併 memory_chunks.tags + captures.tags，取 Top-10
    let tag_rows = sqlx::query(
        "SELECT tags FROM memory_chunks WHERE space_id = ? AND tags IS NOT NULL AND tags != '[]'
         UNION ALL
         SELECT tags FROM captures WHERE space_id = ? AND tags IS NOT NULL AND tags != '[]'",
    )
    .bind(&space_id)
    .bind(&space_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let mut tag_freq: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for r in &tag_rows {
        let tags_json: String = r.try_get("tags").unwrap_or_else(|_| "[]".to_string());
        if let Ok(arr) = serde_json::from_str::<Vec<String>>(&tags_json) {
            for tag in arr {
                let t = tag.trim().to_string();
                if !t.is_empty() {
                    *tag_freq.entry(t).or_insert(0) += 1;
                }
            }
        }
    }

    let mut top_tags: Vec<TagStat> = tag_freq
        .into_iter()
        .map(|(tag, count)| TagStat { tag, count })
        .collect();
    top_tags.sort_by(|a, b| b.count.cmp(&a.count));
    top_tags.truncate(10);

    let total_chunks = data_count + pattern_count + log_count + capture_count;

    Ok(SpaceInsight {
        space_id,
        space_name,
        total_chunks,
        data_count,
        pattern_count,
        log_count,
        capture_count,
        top_tags,
    })
}

/// 手動觸發 Space 全量重聚類（新 Space 建立後或前端主動呼叫）
#[tauri::command]
pub async fn trigger_space_recluster(state: State<'_, AppState>) -> Result<usize, String> {
    let engine = crate::services::space_engine::SpaceEngine::new(
        state.db.clone(),
        state.embedder.clone(),
        state.vector_store.clone(),
    );
    engine.recluster_all().await
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceWiki {
    pub space_id: String,
    pub wiki_content: String,
    pub wiki_updated_at: String,
}

/// 取得指定 Space 的 Wiki 內容
#[tauri::command]
pub async fn get_space_wiki(
    pool: State<'_, SqlitePool>,
    space_id: String,
) -> Result<SpaceWiki, String> {
    let row = sqlx::query("SELECT id, wiki_content, wiki_updated_at FROM spaces WHERE id = ?")
        .bind(&space_id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Space not found".to_string())?;

    Ok(SpaceWiki {
        space_id,
        wiki_content: row.try_get("wiki_content").unwrap_or_default(),
        wiki_updated_at: row.try_get("wiki_updated_at").unwrap_or_default(),
    })
}

/// 用戶手動儲存編輯後的 Wiki 內容
#[tauri::command]
pub async fn save_space_wiki(
    pool: State<'_, SqlitePool>,
    space_id: String,
    wiki_content: String,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE spaces SET wiki_content = ?, wiki_updated_at = ? WHERE id = ?")
        .bind(&wiki_content)
        .bind(&now)
        .bind(&space_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 手動觸發指定 Space 的 Wiki 重新生成
#[tauri::command]
pub async fn regenerate_space_wiki(
    pool: State<'_, SqlitePool>,
    space_id: String,
) -> Result<String, String> {
    let engine = crate::services::space_wiki_engine::SpaceWikiEngine::new(pool.inner().clone());
    engine.update_wiki_for_space(&space_id).await
}
