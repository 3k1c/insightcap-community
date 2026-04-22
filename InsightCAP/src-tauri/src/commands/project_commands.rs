use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tauri::State;
use uuid::Uuid;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub default_tags: Vec<String>,
    pub color: Option<String>,
    pub is_pinned: bool,
    pub is_archived: bool,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub color: Option<String>,
    pub is_pinned: Option<bool>,
    pub is_archived: Option<bool>,
}

async fn ensure_project_color_column(pool: &SqlitePool) {
    let _ = sqlx::query("ALTER TABLE projects ADD COLUMN color TEXT")
        .execute(pool)
        .await;
}

#[tauri::command]
pub async fn get_projects(pool: State<'_, SqlitePool>) -> Result<Vec<Project>, String> {
    ensure_project_color_column(pool.inner()).await;

    let rows = sqlx::query(
        "SELECT id, name, default_tags, color, is_pinned, is_archived, sort_order, created_at, updated_at 
         FROM projects 
         WHERE is_archived = 0 
         ORDER BY sort_order ASC, created_at DESC"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let projects = rows
        .into_iter()
        .map(|r| {
            let tags_json: String = r.try_get("default_tags").unwrap_or_default();
            let default_tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            Project {
                id: r.get("id"),
                name: r.get("name"),
                default_tags,
                color: r.try_get("color").ok(),
                is_pinned: r.get::<i32, _>("is_pinned") != 0,
                is_archived: r.get::<i32, _>("is_archived") != 0,
                sort_order: r.get("sort_order"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            }
        })
        .collect();

    Ok(projects)
}

#[tauri::command]
pub async fn get_project_conversations(
    pool: State<'_, SqlitePool>,
    project_id: String,
) -> Result<Vec<crate::commands::conversation_commands::Conversation>, String> {
    let rows = sqlx::query(
        "SELECT id, title, summary, project_id, is_pinned, is_locked, created_at, updated_at FROM conversations
         WHERE project_id = ? AND is_archived = 0
         ORDER BY is_pinned DESC, updated_at DESC"
    )
    .bind(&project_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let conversations = rows
        .into_iter()
        .map(|r| crate::commands::conversation_commands::Conversation {
            id: r.get("id"),
            title: r.try_get("title").unwrap_or_default(),
            summary: r.try_get("summary").unwrap_or_default(),
            project_id: r.try_get("project_id").ok(),
            is_pinned: r.try_get::<i32, _>("is_pinned").unwrap_or(0) != 0,
            is_locked: r.try_get::<i32, _>("is_locked").unwrap_or(0) != 0,
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

    Ok(conversations)
}

#[tauri::command]
pub async fn create_project(
    pool: State<'_, SqlitePool>,
    name: String,
    color: Option<String>,
) -> Result<Project, String> {
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let default_tags: String =
        serde_json::to_string(&Vec::<String>::new()).map_err(|e| e.to_string())?;

    ensure_project_color_column(pool.inner()).await;

    sqlx::query(
        "INSERT INTO projects (id, name, default_tags, color, created_at, updated_at) 
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&name)
    .bind(&default_tags)
    .bind(&color)
    .bind(&now)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(Project {
        id,
        name,
        default_tags: vec![],
        color,
        is_pinned: false,
        is_archived: false,
        sort_order: 0,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub async fn update_project(
    pool: State<'_, SqlitePool>,
    project_id: String,
    name: Option<String>,
    color: Option<String>,
    is_pinned: Option<bool>,
    is_archived: Option<bool>,
) -> Result<Project, String> {
    ensure_project_color_column(pool.inner()).await;

    let now = Utc::now().to_rfc3339();

    let mut query_str = "UPDATE projects SET updated_at = ?".to_string();
    let mut bindings: Vec<String> = vec![now.clone()];

    if name.is_some() {
        query_str.push_str(", name = ?");
        bindings.push(name.clone().unwrap_or_default());
    }
    if color.is_some() {
        query_str.push_str(", color = ?");
        bindings.push(color.clone().unwrap_or_default());
    }
    if is_pinned.is_some() {
        query_str.push_str(", is_pinned = ?");
        bindings.push(if is_pinned.unwrap() {
            "1".to_string()
        } else {
            "0".to_string()
        });
    }
    if is_archived.is_some() {
        query_str.push_str(", is_archived = ?");
        bindings.push(if is_archived.unwrap() {
            "1".to_string()
        } else {
            "0".to_string()
        });
    }

    query_str.push_str(" WHERE id = ?");
    bindings.push(project_id.clone());

    let mut query = sqlx::query(&query_str);
    for binding in bindings {
        query = query.bind(binding);
    }

    query
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    get_project_by_id(pool, project_id).await
}

#[tauri::command]
pub async fn delete_project(pool: State<'_, SqlitePool>, project_id: String) -> Result<(), String> {
    sqlx::query("UPDATE conversations SET project_id = NULL WHERE project_id = ?")
        .bind(&project_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM projects WHERE id = ?")
        .bind(&project_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn update_project_sort_order(
    pool: State<'_, SqlitePool>,
    project_id: String,
    sort_order: i32,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();

    sqlx::query("UPDATE projects SET sort_order = ?, updated_at = ? WHERE id = ?")
        .bind(sort_order)
        .bind(&now)
        .bind(&project_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn move_conversation_to_project(
    pool: State<'_, SqlitePool>,
    conversation_id: String,
    project_id: Option<String>,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();

    sqlx::query("UPDATE conversations SET project_id = ?, updated_at = ? WHERE id = ?")
        .bind(&project_id)
        .bind(&now)
        .bind(&conversation_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

async fn get_project_by_id(
    pool: State<'_, SqlitePool>,
    project_id: String,
) -> Result<Project, String> {
    ensure_project_color_column(pool.inner()).await;

    let row = sqlx::query(
        "SELECT id, name, default_tags, color, is_pinned, is_archived, sort_order, created_at, updated_at 
         FROM projects 
         WHERE id = ?"
    )
    .bind(&project_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let tags_json: String = row.try_get("default_tags").unwrap_or_default();
    let default_tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

    Ok(Project {
        id: row.get("id"),
        name: row.get("name"),
        default_tags,
        color: row.try_get("color").ok(),
        is_pinned: row.get::<i32, _>("is_pinned") != 0,
        is_archived: row.get::<i32, _>("is_archived") != 0,
        sort_order: row.get("sort_order"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
