use tauri::AppHandle;

/// Phase 2 Stub: 對話總結排程
/// 用於在對話結束或切換時，自動啟動總結任務並提取跨對話關聯記憶
pub fn start_scheduler(_app: AppHandle) {
    println!("[ConversationScheduler] Worker started (Stub)");
}
