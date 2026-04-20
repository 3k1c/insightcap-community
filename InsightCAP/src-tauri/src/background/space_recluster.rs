use crate::db::AppState;
use crate::services::space_engine::SpaceEngine;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Listener, Manager};

/// SpaceRecluster 背景工作程式
/// - 監聽 `space-created` event，觸發一次立即重聚類
/// - 每 30 分鐘定期重聚類，確保新增資料逐漸分配到正確 Space
pub fn start_recluster_worker(app: AppHandle) {
    // Worker 1：定時重聚類（每 30 分鐘）
    let app_timer = app.clone();
    let mut shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        println!("[SpaceRecluster] Worker 啟動");
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[SpaceRecluster] 收到停止訊號，退出。");
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(30 * 60)) => {
                    run_recluster(&app_timer).await;
                }
            }
        }
    });

    // Worker 2：監聽 space-created event，觸發即時重聚類
    let app_listener = app.clone();
    let mut shutdown_rx2 = app.state::<AppState>().shutdown_tx.subscribe();

    // 用 channel 接收 space-created event（避免閉包所有權問題）
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(4);
    let tx_clone = tx.clone();
    let _unlisten = app.listen("space-created", move |_| {
        let _ = tx_clone.try_send(());
    });

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx2.changed() => {
                    if *shutdown_rx2.borrow() { break; }
                }
                Some(_) = rx.recv() => {
                    // 延遲 3 秒，等待 DB 事務確認提交
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    run_recluster(&app_listener).await;
                }
            }
        }
    });
}

async fn run_recluster(app: &AppHandle) {
    let app_state = app.state::<AppState>();
    let pool = app_state.db.clone();
    let embedder = app_state.embedder.clone();
    let vector_store = app_state.vector_store.clone();

    let engine = SpaceEngine::new(pool, embedder, vector_store);
    match engine.recluster_all().await {
        Ok(count) if count > 0 => {
            println!("[SpaceRecluster] 重聚類完成，更新 {} 筆記錄", count);
            let _ = app.emit("space-reclustered", count);
        }
        Ok(_) => {}
        Err(e) => eprintln!("[SpaceRecluster] 重聚類失敗: {}", e),
    }
}
