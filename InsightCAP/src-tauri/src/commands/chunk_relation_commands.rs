use crate::db::AppState;
use crate::services::chunk_relation_engine::{ChunkRelationEngine, LinkedChunk};
use tauri::State;

#[tauri::command]
pub async fn get_chunk_relations(
    state: State<'_, AppState>,
    chunk_ids: Vec<String>,
) -> Result<Vec<LinkedChunk>, String> {
    let engine = ChunkRelationEngine::new(
        state.db.clone(),
        state.embedder.clone(),
        state.vector_store.clone(),
    );
    engine.fetch_linked_chunks(&chunk_ids).await
}
