use crate::db::AppState;
use chrono::Utc;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn clear_seed_data(state: State<'_, AppState>) -> Result<String, String> {
    let pool = &state.db;
    sqlx::query("DELETE FROM memory_chunks WHERE id LIKE 'seed-mc-%'")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM captures WHERE id LIKE 'seed-cap-%'")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM sources WHERE id LIKE 'seed-src-%'")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM spaces WHERE id LIKE 'seed-space-%'")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok("Cleared all seed test data".to_string())
}

#[tauri::command]
pub async fn seed_test_data(state: State<'_, AppState>) -> Result<String, String> {
    let pool = &state.db;
    let now = Utc::now().to_rfc3339();

    let space1_id = format!("seed-space-{}", Uuid::new_v4());
    let space2_id = format!("seed-space-{}", Uuid::new_v4());

    sqlx::query(
        "INSERT OR IGNORE INTO spaces (id, name, description, chunk_count, created_by, is_archived, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'ai', 0, ?, ?)"
    )
    .bind(&space1_id).bind("Frontend Development").bind("React, TypeScript, and Tauri knowledge")
    .bind(4).bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT OR IGNORE INTO spaces (id, name, description, chunk_count, created_by, is_archived, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'ai', 0, ?, ?)"
    )
    .bind(&space2_id).bind("Product Design").bind("UX design, user research, and design systems")
    .bind(3).bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    let src1_id = format!("seed-src-{}", Uuid::new_v4());
    let src2_id = format!("seed-src-{}", Uuid::new_v4());

    sqlx::query(
        "INSERT OR IGNORE INTO sources (id, type, title, clean_content, capture_count, captured_at, updated_at)
         VALUES (?, 'file', ?, ?, 2, ?, ?)"
    )
    .bind(&src1_id).bind("react-performance-notes.md")
    .bind("Analysis of when to use React.memo, useMemo, and useCallback. Avoiding unnecessary re-renders is key to frontend performance optimization.")
    .bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT OR IGNORE INTO sources (id, type, title, clean_content, capture_count, captured_at, updated_at)
         VALUES (?, 'editor', ?, ?, 1, ?, ?)"
    )
    .bind(&src2_id).bind("design-system-spec-v1.0")
    .bind("Color tokens, spacing system, and typography rules. Semantic names like surface-card and text-primary ensure cross-theme consistency.")
    .bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    let cap1_id = format!("seed-cap-{}", Uuid::new_v4());
    let cap2_id = format!("seed-cap-{}", Uuid::new_v4());
    let cap3_id = format!("seed-cap-{}", Uuid::new_v4());

    sqlx::query(
        "INSERT OR IGNORE INTO captures (id, source_id, space_id, type, raw_content, clean_content, capture_method, tags, status, created_at, updated_at)
         VALUES (?, ?, ?, 'text', ?, ?, 'import', ?, 'processed', ?, ?)"
    )
    .bind(&cap1_id).bind(&src1_id).bind(&space1_id)
    .bind("useMemo is valuable only when computation is expensive; overuse can add overhead.")
    .bind("useMemo is valuable only when computation is expensive; overuse can add overhead.")
    .bind(r#"["React","performance","useMemo"]"#)
    .bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT OR IGNORE INTO captures (id, source_id, space_id, type, raw_content, clean_content, capture_method, tags, status, created_at, updated_at)
         VALUES (?, ?, ?, 'text', ?, ?, 'import', ?, 'processed', ?, ?)"
    )
    .bind(&cap2_id).bind(&src1_id).bind(&space1_id)
    .bind("Tauri invoke is asynchronous. Frontend calls to backend commands should use await with proper try/catch handling.")
    .bind("Tauri invoke is asynchronous. Frontend calls to backend commands should use await with proper try/catch handling.")
    .bind(r#"["Tauri","async","error-handling"]"#)
    .bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT OR IGNORE INTO captures (id, source_id, space_id, type, raw_content, clean_content, capture_method, tags, status, created_at, updated_at)
         VALUES (?, ?, ?, 'text', ?, ?, 'import', ?, 'processed', ?, ?)"
    )
    .bind(&cap3_id).bind(&src2_id).bind(&space2_id)
    .bind("Design-system token naming should be semantic (surface-card) instead of raw color values (gray-100) for easier theme switching.")
    .bind("Design-system token naming should be semantic (surface-card) instead of raw color values (gray-100) for easier theme switching.")
    .bind(r#"["design-system","token","theme"]"#)
    .bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    let chunks: Vec<(&str, &str, &str, &str, f64, i64)> = vec![
        (&space1_id, "pattern",
         "React performance SOP: use React DevTools Profiler first, then apply memo/useMemo selectively. Avoid premature optimization.",
         r#"["React","performance","SOP"]"#, 0.92, 0),
        (&space1_id, "pattern",
         "Tauri command workflow: 1) write Rust fn + #[tauri::command], 2) prefix unused params with _, 3) register in lib.rs generate_handler![], 4) call from frontend invoke.",
         r#"["Tauri","development-flow","SOP"]"#, 0.88, 0),
        (&space1_id, "log",
         "Pitfall: sqlite3 CLI cannot open SQLCipher-encrypted databases and returns 'file is not a database'. Use app-native access flow.",
         r#"["SQLCipher","pitfall","database"]"#, 0.95, 0),
        (&space2_id, "log",
         "BubbleMenu positioning issue: Tiptap BubbleMenu repositions repeatedly during selection drag and jitters. Manual fixed positioning + debounce resolves it.",
         r#"["Tiptap","BubbleMenu","positioning"]"#, 0.90, 0),
        (&space1_id, "data",
         "In TypeScript, convert Set to Array using Array.from(set) or [...set]. The latter may require downlevelIteration under some tsconfig settings.",
         r#"["TypeScript","Set","Array"]"#, 0.55, 1),
        (&space2_id, "data",
         "Figma Auto Layout spacing should map to spacing tokens, for example spacing-3 = 12px.",
         r#"["Figma","design-system","spacing"]"#, 0.50, 1),
    ];

    for (space_id, ktype, content, tags, confidence, pending) in &chunks {
        let chunk_id = format!("seed-mc-{}", Uuid::new_v4());
        sqlx::query(
            "INSERT OR IGNORE INTO memory_chunks
             (id, space_id, knowledge_type, content, tags, confidence, pending_confirm, placed_by, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, 'ai', ?, ?)"
        )
        .bind(&chunk_id).bind(space_id).bind(ktype).bind(content)
        .bind(tags).bind(confidence).bind(pending)
        .bind(&now).bind(&now)
        .execute(pool).await.map_err(|e| e.to_string())?;
    }

    for space_id in [&space1_id, &space2_id] {
        sqlx::query(
            "UPDATE spaces SET chunk_count = (
                SELECT COUNT(*) FROM memory_chunks WHERE space_id = ? AND pending_confirm = 0
             ) + (
                SELECT COUNT(*) FROM captures WHERE space_id = ?
             ) WHERE id = ?",
        )
        .bind(space_id)
        .bind(space_id)
        .bind(space_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(format!(
        "Seeded: 2 spaces, 2 sources, 3 captures, 6 memory_chunks (2 pending_confirm)"
    ))
}
