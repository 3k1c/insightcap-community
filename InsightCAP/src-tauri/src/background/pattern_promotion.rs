use std::time::Duration;
use tauri::{AppHandle, Manager, Emitter};
use crate::db::AppState;
use crate::services::pattern_engine::PatternEngine;

/// Pattern Promotion 背景工作程式
/// 定期喚醒檢視是否有多對話記憶重疊
pub fn start_pattern_promotion_worker(app: AppHandle) {
    let mut shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        println!("[PATTERN-PROMOTION] Started.");

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[PATTERN-PROMOTION] 收到停止訊號，退出。");
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(60 * 5)) => {
                    let app_state = app.state::<AppState>();
                    let pool = app_state.db.clone();
                    let embedder = app_state.embedder.clone();
                    let engine = PatternEngine::new(pool, embedder);
                    match engine.detect_and_promote_patterns().await {
                        Ok(count) if count > 0 => {
                            println!("[PATTERN-PROMOTION] 成功升格 {} 筆跨對話概念！", count);
                            let _ = app.emit("pattern-promoted", count);
                        }
                        Ok(_) => {}
                        Err(e) => eprintln!("[PATTERN-PROMOTION] 錯誤: {}", e),
                    }
                }
            }
        }
    });
}
