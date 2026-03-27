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

#[tauri::command]
pub async fn get_all_spaces(pool: State<'_, SqlitePool>) -> Result<Vec<SpaceItem>, String> {
    let rows = sqlx::query(
        "SELECT id, name, description, chunk_count FROM spaces WHERE is_archived = 0 ORDER BY chunk_count DESC, created_at DESC"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let spaces = rows.into_iter().map(|r| SpaceItem {
        id: r.get("id"),
        name: r.get("name"),
        description: r.try_get("description").unwrap_or_default(),
        chunk_count: r.try_get("chunk_count").unwrap_or(0),
    }).collect();

    Ok(spaces)
}
