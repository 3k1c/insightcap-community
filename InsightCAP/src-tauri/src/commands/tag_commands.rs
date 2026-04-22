use sqlx::{Row, SqlitePool};
use tauri::State;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub source: String, // 'ai' or 'user'
    pub use_count: i64,
    pub recent_count: i64,
}

#[tauri::command]
pub async fn get_all_tags(pool: State<'_, SqlitePool>) -> Result<Vec<Tag>, String> {
    let rows = sqlx::query(
        "SELECT id, name, source, use_count, recent_count FROM tags ORDER BY recent_count DESC, use_count DESC LIMIT 100"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let tags = rows
        .into_iter()
        .map(|r| Tag {
            id: r.get("id"),
            name: r.get("name"),
            source: r.try_get("source").unwrap_or_else(|_| "ai".to_string()),
            use_count: r.try_get("use_count").unwrap_or(0),
            recent_count: r.try_get("recent_count").unwrap_or(0),
        })
        .collect();

    Ok(tags)
}

#[tauri::command]
pub async fn suggest_tags(
    pool: State<'_, SqlitePool>,
    query: String,
) -> Result<Vec<String>, String> {
    let pattern = format!("%{}%", query.trim());
    let rows =
        sqlx::query("SELECT name FROM tags WHERE name LIKE ? ORDER BY recent_count DESC LIMIT 10")
            .bind(pattern)
            .fetch_all(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let suggestions = rows.into_iter().map(|r| r.get("name")).collect();
    Ok(suggestions)
}

#[tauri::command]
pub async fn get_source_ids_by_tag(
    pool: State<'_, SqlitePool>,
    tag: String,
) -> Result<Vec<String>, String> {
    let pattern = format!("%\"{}%", tag);
    let rows = sqlx::query(
        "SELECT DISTINCT source_id FROM captures WHERE source_id IS NOT NULL AND tags LIKE ? LIMIT 500"
    )
    .bind(pattern)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let ids: Vec<String> = rows
        .into_iter()
        .filter_map(|r| r.try_get("source_id").ok())
        .collect();
    Ok(ids)
}
