use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::time::{sleep, Duration};

/// 開啟 B站登入視窗，利用 Tauri v2 的原生 Cookie API 擷取 SESSDATA (支援 HttpOnly)
#[tauri::command]
pub async fn open_bilibili_login(app: AppHandle) -> Result<String, String> {
    // 1. 建立結果 channel
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let tx_arc = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let tx_clone = tx_arc.clone();

    // 2. 建立登入視窗
    WebviewWindowBuilder::new(
        &app,
        "bilibili-login",
        WebviewUrl::External("https://passport.bilibili.com/login".parse().unwrap()),
    )
    .title("登錄 B站帳號 (登入完成後視窗將自動關閉)")
    .inner_size(520.0, 700.0)
    .build()
    .map_err(|e| format!("無法開啟視窗: {}", e))?;

    println!("[BILI_AUTH] Login window opened. Using Native Cookie API.");

    // 3. 輪詢任務：監控 URL 跳轉並利用原生 API 讀取 Cookie
    let app_handle = app.clone();
    let poll_handle = tokio::spawn(async move {
        for _ in 0..120 {
            sleep(Duration::from_secs(1)).await;

            let target_win = match app_handle.get_webview_window("bilibili-login") {
                Some(w) => w,
                None => break, // 使用者手動關閉
            };

            // 檢查當前 URL
            let current_url = match target_win.url() {
                Ok(u) => u.to_string(),
                Err(_) => continue,
            };

            // 只要是在 bilibili.com 網域下，就嘗試抓取 Cookie (即便還在 passport 頁面，萬一它提早寫入呢)
            if current_url.contains("bilibili.com") {
                if let Ok(cookies) = target_win.cookies() {
                    let mut found_sessdata = None;
                    for cookie in cookies {
                        if cookie.name() == "SESSDATA" {
                            found_sessdata = Some(cookie.value().to_string());
                            break;
                        }
                    }

                    if let Some(sessdata) = found_sessdata {
                        println!(
                            "[BILI_AUTH] ✅ Success! Captured SESSDATA (len={})",
                            sessdata.len()
                        );
                        if let Ok(mut guard) = tx_clone.lock() {
                            if let Some(sender) = guard.take() {
                                let _ = sender.send(sessdata);
                            }
                        }
                        break;
                    }
                }
            }
        }
    });

    // 4. 等待結果 (最多 65 秒)
    let result = tokio::time::timeout(Duration::from_secs(65), rx).await;
    poll_handle.abort();

    // 5. 清理並關閉視窗，聚焦回主視窗
    if let Some(win) = app.get_webview_window("bilibili-login") {
        let _ = win.close();
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }

    match result {
        Ok(Ok(sessdata)) if !sessdata.is_empty() => {
            println!("[BILI_AUTH] Connection successful.");
            Ok(sessdata)
        }
        _ => Err(
            "未能成功取得登入資訊。請確認您已正確完成 B站登入程序。\n若視窗太快關閉，請嘗試重新點擊連接按鈕。"
                .to_string(),
        ),
    }
}
