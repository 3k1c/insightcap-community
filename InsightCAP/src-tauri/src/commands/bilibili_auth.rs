use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::time::{sleep, Duration};

#[tauri::command]
pub async fn open_bilibili_login(app: AppHandle) -> Result<String, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let tx_arc = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let tx_clone = tx_arc.clone();

    WebviewWindowBuilder::new(
        &app,
        "bilibili-login",
        WebviewUrl::External("https://passport.bilibili.com/login".parse().unwrap()),
    )
    .title("Login to Bilibili (complete login in this window)")
    .inner_size(520.0, 700.0)
    .build()
    .map_err(|e| format!("Failed to open login window: {}", e))?;

    println!("[BILI_AUTH] Login window opened. Using Native Cookie API.");

    let app_handle = app.clone();
    let poll_handle = tokio::spawn(async move {
        for _ in 0..120 {
            sleep(Duration::from_secs(1)).await;

            let target_win = match app_handle.get_webview_window("bilibili-login") {
                Some(w) => w,
                None => break,
            };

            let current_url = match target_win.url() {
                Ok(u) => u.to_string(),
                Err(_) => continue,
            };

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
                            "[BILI_AUTH] Success! Captured SESSDATA (len={})",
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

    let result = tokio::time::timeout(Duration::from_secs(65), rx).await;
    poll_handle.abort();

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
            "Could not retrieve SESSDATA. Please log in to Bilibili in the popup and try again."
                .to_string(),
        ),
    }
}
