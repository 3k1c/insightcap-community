use sqlx::SqlitePool;
use std::time::Duration;
use tauri::{AppHandle, Manager, Emitter};
use crate::services::pattern_engine::PatternEngine;

/// Pattern Promotion 背景工作程式
/// 定期喚醒檢視是否有多對話記憶重疊
pub fn start_pattern_promotion_worker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        println!("[PATTERN-PROMOTION] Started.");

        loop {
            tokio::time::sleep(Duration::from_secs(60 * 5)).await; // 每 5 分鐘運行一次

            let pool = match app.try_state::<SqlitePool>() {
                Some(p) => p.inner().clone(),
                None => continue,
            };

            let engine = PatternEngine::new(pool);
            match engine.detect_and_promote_patterns().await {
                Ok(count) if count > 0 => {
                    println!("[PATTERN-PROMOTION] 成功升格 {} 筆跨對話概念！", count);
                    // Emit event 給前端，顯示 Toast 提醒使用者有需要審閱的新知識
                    let _ = app.emit("pattern-promoted", count);
                }
                Ok(_) => { /* 無新升格 */ }
                Err(e) => eprintln!("[PATTERN-PROMOTION] 錯誤: {}", e),
            }
        }
    });
}
