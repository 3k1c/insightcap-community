use crate::db::AppState;
use crate::services::pattern_engine::PatternEngine;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub fn start_pattern_promotion_worker(app: AppHandle) {
    let mut shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        println!("[PATTERN-PROMOTION] Started.");

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[PATTERN-PROMOTION] Stop signal received, exiting.");
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
                            println!("[PATTERN-PROMOTION] Promoted {} patterns", count);
                            let _ = app.emit("pattern-promoted", count);
                        }
                        Ok(_) => {}
                        Err(e) => eprintln!("[PATTERN-PROMOTION] Error: {}", e),
                    }
                }
            }
        }
    });
}
