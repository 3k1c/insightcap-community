use tauri::AppHandle;

/// Phase 2 Stub: Space Recluster
/// 在新增太多資料或新 Space 建立時，負責背景更新向量集群
pub fn start_recluster_worker(_app: AppHandle) {
    // 預留邏輯：每隔一段時間重新掃描 captures 並以 Space 向量中心重新分配
    println!("[SpaceRecluster] Worker started (Stub)");
}
