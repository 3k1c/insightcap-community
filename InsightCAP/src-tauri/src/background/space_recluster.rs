use crate::db::AppState;
use crate::services::space_engine::SpaceEngine;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Listener, Manager};

pub fn start_recluster_worker(app: AppHandle) {
    let mut shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    let app_timer = app.clone();
    tauri::async_runtime::spawn(async move {
        println!("[SpaceRecluster] Worker started");
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[SpaceRecluster] Stop signal received, exiting.");
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(30 * 60)) => {
                    run_recluster(&app_timer).await;
                }
            }
        }
    });

    let app_listener = app.clone();
    let mut shutdown_rx2 = app.state::<AppState>().shutdown_tx.subscribe();

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
            println!(
                "[SpaceRecluster] Recluster completed: {} spaces updated",
                count
            );
            let _ = app.emit("space-reclustered", count);
        }
        Ok(_) => {}
        Err(e) => eprintln!("[SpaceRecluster] Recluster failed: {}", e),
    }
}
