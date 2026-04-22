use crate::db::AppState;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub fn start_cloud_sync_watcher(app: AppHandle, _kb_path: PathBuf) {
    let mut shutdown_rx = app.state::<AppState>().shutdown_tx.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        println!("[CloudSyncWatcher] Stop signal received, exiting.");
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                }
            }
        }
    });
}
