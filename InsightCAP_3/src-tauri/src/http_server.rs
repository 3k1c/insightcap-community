use axum::{routing::get, Router};
use tauri::AppHandle;
use tokio::net::TcpListener;

/// Phase 6: 開放本地 HTTP API (Axum) 以供未來的 React Native 手機版連入使用
pub async fn start_api_server(_app: AppHandle) {
    let app_router = Router::new()
        .route("/api/health", get(|| async { "InsightCAP Local API running" }));

    let addr = "127.0.0.1:3030";
    if let Ok(listener) = TcpListener::bind(addr).await {
        println!("[HTTP] API Server listening on {}", addr);
        let _ = axum::serve(listener, app_router).await;
    } else {
        eprintln!("[HTTP] Failed to bind API server to {}", addr);
    }
}
