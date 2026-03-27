use std::path::PathBuf;
use tauri::AppHandle;
use std::time::Duration;

pub fn start_cloud_sync_watcher(_app: AppHandle, _kb_path: PathBuf) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            // TODO: 偵測 SQLite 檔案的 modified time 是否與內部 cache 有異 (例如被 Dropbox 覆蓋)
        }
    });
}
