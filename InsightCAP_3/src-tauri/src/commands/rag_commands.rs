use sqlx::SqlitePool;
use tauri::State;

// 佔位：後續 Phase 2 實作完整的 RAGEngine
#[tauri::command]
pub async fn rag_query(
    pool: State<'_, SqlitePool>,
    query: String,
    project_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let engine = crate::services::rag_engine::RagEngine::new(pool.inner().clone());
    engine.generate_answer(&query, project_id).await
}
