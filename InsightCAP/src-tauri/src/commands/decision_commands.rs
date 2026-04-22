use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tauri::State;
use uuid::Uuid;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub id: String,
    pub project_id: String,
    pub conversation_id: String,
    pub variable_desc: String,
    pub options: Vec<String>,
    pub chosen_option: String,
    pub outcome_source: Option<String>,
    pub outcome_rating: Option<String>,
    pub outcome_note: Option<String>,
    pub status: String,
    pub trigger_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub async fn create_decision(
    pool: State<'_, SqlitePool>,
    project_id: String,
    conversation_id: String,
    variable_desc: String,
    options: Vec<String>,
    chosen_option: String,
) -> Result<Decision, String> {
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();
    let trigger_at = (Utc::now() + chrono::Duration::days(14)).to_rfc3339();
    let options_json = serde_json::to_string(&options).map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO decisions (id, project_id, conversation_id, variable_desc, options, chosen_option, status, trigger_at, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?)"
    )
    .bind(&id)
    .bind(&project_id)
    .bind(&conversation_id)
    .bind(&variable_desc)
    .bind(&options_json)
    .bind(&chosen_option)
    .bind(&trigger_at)
    .bind(&now)
    .bind(&now)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(Decision {
        id,
        project_id,
        conversation_id,
        variable_desc,
        options,
        chosen_option,
        outcome_source: None,
        outcome_rating: None,
        outcome_note: None,
        status: "pending".to_string(),
        trigger_at,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub async fn get_due_decisions(pool: State<'_, SqlitePool>) -> Result<Vec<Decision>, String> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "SELECT id, project_id, conversation_id, variable_desc, options, chosen_option, \
                outcome_source, outcome_rating, outcome_note, status, trigger_at, created_at, updated_at \
         FROM decisions \
         WHERE status = 'pending' AND trigger_at <= ? \
         ORDER BY trigger_at ASC LIMIT 10"
    )
    .bind(&now)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(|r| row_to_decision(&r)).collect())
}

#[tauri::command]
pub async fn get_project_decisions(
    pool: State<'_, SqlitePool>,
    project_id: String,
) -> Result<Vec<Decision>, String> {
    let rows = sqlx::query(
        "SELECT id, project_id, conversation_id, variable_desc, options, chosen_option, \
                outcome_source, outcome_rating, outcome_note, status, trigger_at, created_at, updated_at \
         FROM decisions \
         WHERE project_id = ? \
         ORDER BY created_at DESC"
    )
    .bind(&project_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows.into_iter().map(|r| row_to_decision(&r)).collect())
}

#[tauri::command]
pub async fn report_decision_outcome(
    pool: State<'_, SqlitePool>,
    decision_id: String,
    outcome_rating: String,
    outcome_note: Option<String>,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE decisions SET outcome_source = 'user_report', outcome_rating = ?, outcome_note = ?, \
         status = 'reported', updated_at = ? WHERE id = ?"
    )
    .bind(&outcome_rating)
    .bind(&outcome_note)
    .bind(&now)
    .bind(&decision_id)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn dismiss_decision(
    pool: State<'_, SqlitePool>,
    decision_id: String,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE decisions SET status = 'dismissed', updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&decision_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn row_to_decision(r: &sqlx::sqlite::SqliteRow) -> Decision {
    let options_json: String = r.try_get("options").unwrap_or_else(|_| "[]".to_string());
    let options: Vec<String> = serde_json::from_str(&options_json).unwrap_or_default();

    Decision {
        id: r.get("id"),
        project_id: r.get("project_id"),
        conversation_id: r.get("conversation_id"),
        variable_desc: r.try_get("variable_desc").unwrap_or_default(),
        options,
        chosen_option: r.try_get("chosen_option").unwrap_or_default(),
        outcome_source: r.try_get("outcome_source").ok(),
        outcome_rating: r.try_get("outcome_rating").ok(),
        outcome_note: r.try_get("outcome_note").ok(),
        status: r
            .try_get("status")
            .unwrap_or_else(|_| "pending".to_string()),
        trigger_at: r.try_get("trigger_at").unwrap_or_default(),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}
