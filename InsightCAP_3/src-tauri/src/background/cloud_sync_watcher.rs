use std::path::PathBuf;
use tauri::AppHandle;

pub struct FileWatcher {}

impl FileWatcher {
    pub fn new(_app: AppHandle, _attachments_path: PathBuf) -> Result<Self, String> {
        println!("[WATCHER] Stubbed temporarily due to missing VectorStore in Phase 3.");
        Ok(Self {})
    }
}
