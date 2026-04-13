/// 測試用資料注入指令 — 用完請移除此檔案及相關登記
use tauri::State;
use crate::db::AppState;
use chrono::Utc;
use uuid::Uuid;

#[tauri::command]
pub async fn clear_seed_data(state: State<'_, AppState>) -> Result<String, String> {
    let pool = &state.db;
    // 依照 id 前綴刪除所有 seed 資料
    sqlx::query("DELETE FROM memory_chunks WHERE id LIKE 'seed-mc-%'").execute(pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM captures WHERE id LIKE 'seed-cap-%'").execute(pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM sources WHERE id LIKE 'seed-src-%'").execute(pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM spaces WHERE id LIKE 'seed-space-%'").execute(pool).await.map_err(|e| e.to_string())?;
    Ok("已清除所有測試資料".to_string())
}

#[tauri::command]
pub async fn seed_test_data(state: State<'_, AppState>) -> Result<String, String> {
    let pool = &state.db;
    let now = Utc::now().to_rfc3339();

    // ── 1. Spaces ────────────────────────────────────────────────────────────
    let space1_id = format!("seed-space-{}", Uuid::new_v4());
    let space2_id = format!("seed-space-{}", Uuid::new_v4());

    sqlx::query(
        "INSERT OR IGNORE INTO spaces (id, name, description, chunk_count, created_by, is_archived, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'ai', 0, ?, ?)"
    )
    .bind(&space1_id).bind("前端開發").bind("React、TypeScript、Tauri 相關知識")
    .bind(4).bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT OR IGNORE INTO spaces (id, name, description, chunk_count, created_by, is_archived, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'ai', 0, ?, ?)"
    )
    .bind(&space2_id).bind("產品設計").bind("UX 設計、用戶研究、設計系統")
    .bind(3).bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    // ── 2. Sources ───────────────────────────────────────────────────────────
    let src1_id = format!("seed-src-{}", Uuid::new_v4());
    let src2_id = format!("seed-src-{}", Uuid::new_v4());

    sqlx::query(
        "INSERT OR IGNORE INTO sources (id, type, title, clean_content, capture_count, captured_at, updated_at)
         VALUES (?, 'file', ?, ?, 2, ?, ?)"
    )
    .bind(&src1_id).bind("React 效能優化筆記.md")
    .bind("React.memo、useMemo、useCallback 的使用時機與效能影響分析。避免不必要的重新渲染是前端效能優化的核心。")
    .bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT OR IGNORE INTO sources (id, type, title, clean_content, capture_count, captured_at, updated_at)
         VALUES (?, 'editor', ?, ?, 1, ?, ?)"
    )
    .bind(&src2_id).bind("設計系統規範 v1.0")
    .bind("色彩 token、間距系統、字型規範。surface-card、text-primary 等語義化命名確保跨主題一致性。")
    .bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    // ── 3. Captures ──────────────────────────────────────────────────────────
    let cap1_id = format!("seed-cap-{}", Uuid::new_v4());
    let cap2_id = format!("seed-cap-{}", Uuid::new_v4());
    let cap3_id = format!("seed-cap-{}", Uuid::new_v4());

    sqlx::query(
        "INSERT OR IGNORE INTO captures (id, source_id, space_id, type, raw_content, clean_content, capture_method, tags, status, created_at, updated_at)
         VALUES (?, ?, ?, 'text', ?, ?, 'import', ?, 'processed', ?, ?)"
    )
    .bind(&cap1_id).bind(&src1_id).bind(&space1_id)
    .bind("useMemo 只在計算成本高時才值得使用，過度使用反而增加 overhead")
    .bind("useMemo 只在計算成本高時才值得使用，過度使用反而增加 overhead")
    .bind(r#"["React","效能優化","useMemo"]"#)
    .bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT OR IGNORE INTO captures (id, source_id, space_id, type, raw_content, clean_content, capture_method, tags, status, created_at, updated_at)
         VALUES (?, ?, ?, 'text', ?, ?, 'import', ?, 'processed', ?, ?)"
    )
    .bind(&cap2_id).bind(&src1_id).bind(&space1_id)
    .bind("Tauri invoke 是非同步的，前端呼叫後端 command 需要 await，錯誤處理要用 try/catch")
    .bind("Tauri invoke 是非同步的，前端呼叫後端 command 需要 await，錯誤處理要用 try/catch")
    .bind(r#"["Tauri","async","錯誤處理"]"#)
    .bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT OR IGNORE INTO captures (id, source_id, space_id, type, raw_content, clean_content, capture_method, tags, status, created_at, updated_at)
         VALUES (?, ?, ?, 'text', ?, ?, 'import', ?, 'processed', ?, ?)"
    )
    .bind(&cap3_id).bind(&src2_id).bind(&space2_id)
    .bind("設計系統的 token 命名應以語義為主（surface-card）而非顏色值（gray-100），方便主題切換")
    .bind("設計系統的 token 命名應以語義為主（surface-card）而非顏色值（gray-100），方便主題切換")
    .bind(r#"["設計系統","token","主題"]"#)
    .bind(&now).bind(&now)
    .execute(pool).await.map_err(|e| e.to_string())?;

    // ── 4. Memory Chunks ─────────────────────────────────────────────────────
    let chunks: Vec<(&str, &str, &str, &str, f64, i64)> = vec![
        // (space_id, type, content, tags, confidence, pending_confirm)
        (&space1_id, "pattern",
         "React 元件效能優化 SOP：先用 React DevTools Profiler 定位熱點，再針對性加 memo/useMemo，不要提前優化",
         r#"["React","效能優化","SOP"]"#, 0.92, 0),
        (&space1_id, "pattern",
         "Tauri command 新增流程：①寫 Rust fn + #[tauri::command] ②加 _前綴未使用參數 ③在 lib.rs generate_handler![] 登記 ④前端 invoke 呼叫",
         r#"["Tauri","開發流程","SOP"]"#, 0.88, 0),
        (&space1_id, "log",
         "踩坑：直接用 sqlite3 CLI 無法開啟 SQLCipher 加密的 DB，會報 'file is not a database' 錯誤，需要透過應用程式本身操作",
         r#"["SQLCipher","踩坑","資料庫"]"#, 0.95, 0),
        (&space2_id, "log",
         "BubbleMenu 定位問題：Tiptap BubbleMenu 在 selection 拖曳時會不斷重新定位導致跳動，改用手動 fixed 定位 + debounce 解決",
         r#"["Tiptap","BubbleMenu","定位問題"]"#, 0.90, 0),
        (&space1_id, "data",
         "TypeScript 中 Set 轉 Array 可用 Array.from(set) 或 [...set]，後者在某些 tsconfig 配置下需要確認 downlevelIteration",
         r#"["TypeScript","Set","Array"]"#, 0.55, 1),
        (&space2_id, "data",
         "Figma Auto Layout 的 spacing 設定建議對應到設計 token 的間距系統，例如 spacing-3 = 12px",
         r#"["Figma","設計系統","間距"]"#, 0.50, 1),
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

    // ── 5. 更新 spaces.chunk_count ───────────────────────────────────────────
    for space_id in [&space1_id, &space2_id] {
        sqlx::query(
            "UPDATE spaces SET chunk_count = (
                SELECT COUNT(*) FROM memory_chunks WHERE space_id = ? AND pending_confirm = 0
             ) + (
                SELECT COUNT(*) FROM captures WHERE space_id = ?
             ) WHERE id = ?"
        )
        .bind(space_id).bind(space_id).bind(space_id)
        .execute(pool).await.map_err(|e| e.to_string())?;
    }

    Ok(format!(
        "已注入：2 個 Space、2 個 Source、3 個 Capture、6 個 memory_chunk（其中 2 筆 pending_confirm）"
    ))
}
