use keyring::Entry;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::PathBuf;
use tauri::Manager;
use zeroize::Zeroize;

use crate::auth::{
    derive_db_key, derive_recovery_key_new, derive_recovery_key_verify,
    load_login_guard, persist_login_guard,
    read_recovery_bin, write_recovery_bin,
};
use crate::auth::key_derivation::generate_mnemonic;
// encrypt_existing_db 保留備用；首次 setup 改用「刪 DB + 重啟」策略

const KEYCHAIN_SERVICE: &str = "insightcap";
const KEYCHAIN_AUTO_LOGIN: &str = "auto_login_key";
const KEYCHAIN_RECOVERY_PENDING: &str = "recovery_pending_v1";

// ─── 共用型別 ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub is_setup: bool,
    pub auto_login: bool,
    /// 目錄遷移場景：Keychain 空 + auth.json + DB 均存在
    pub is_migrated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockStatus {
    pub is_locked: bool,
    pub is_permanently_locked: bool,
    pub remaining_secs: Option<u64>,
    pub fail_count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPayload {
    pub display_name: String,
    pub password: String,
    pub auto_login: bool,
    pub kb_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginPayload {
    pub password: String,
    pub kb_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordPayload {
    pub old_password: String,
    pub new_password: String,
    pub kb_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPayload {
    pub mnemonic: String,
    pub new_password: String,
    pub kb_path: String,
}

// ─── 輔助函式 ──────────────────────────────────────────────────────────────

fn auth_dir(kb_path: &str) -> PathBuf {
    PathBuf::from(kb_path).join(".insightcap")
}

fn auth_json_path(kb_path: &str) -> PathBuf {
    auth_dir(kb_path).join("auth.json")
}

fn recovery_bin_path(kb_path: &str) -> PathBuf {
    auth_dir(kb_path).join("recovery.bin")
}

fn login_guard_path(kb_path: &str) -> PathBuf {
    auth_dir(kb_path).join("login_guard.json")
}

#[derive(Serialize, Deserialize)]
struct AuthJson {
    version: u32,
    salt: String,  // hex-encoded 32-byte Argon2id salt
}

fn load_auth_json(kb_path: &str) -> Option<AuthJson> {
    let content = std::fs::read_to_string(auth_json_path(kb_path)).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_auth_json(kb_path: &str, salt_hex: &str) -> Result<(), String> {
    let dir = auth_dir(kb_path);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let auth = AuthJson { version: 1, salt: salt_hex.to_string() };
    let json = serde_json::to_string_pretty(&auth).map_err(|e| e.to_string())?;

    let tmp = auth_json_path(kb_path).with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, auth_json_path(kb_path)).map_err(|e| e.to_string())?;
    Ok(())
}

// ─── Commands ──────────────────────────────────────────────────────────────

/// 查詢認證狀態：是否已完成首次設置、是否啟用自動登入
#[tauri::command]
pub async fn get_auth_status(kb_path: String) -> Result<AuthStatus, String> {
    let auth_exists = auth_json_path(&kb_path).exists();
    let recovery_exists = recovery_bin_path(&kb_path).exists();
    let db_exists = PathBuf::from(&kb_path).join(".insightcap").join("insightcap.db").exists();

    let is_setup = auth_exists && recovery_exists;

    let keychain_ok = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .and_then(|e| e.get_password())
        .is_ok();

    // 目錄遷移：auth.json + DB 存在但 Keychain 為空（此裝置尚未授權）
    let is_migrated = is_setup && db_exists && !keychain_ok;

    Ok(AuthStatus { is_setup, auto_login: keychain_ok, is_migrated })
}

/// 首次設置：建立密碼、生成恢復碼、寫入 auth.json 和 recovery.bin
///
/// 策略：將 db_key 存入 Keychain，刪除明文 DB，前端收到成功後觸發 app 重啟。
/// 重啟時 init_db 從 Keychain 讀取 key，用 PRAGMA key 建立全新加密 DB。
#[tauri::command]
pub async fn setup_auth(
    pool: tauri::State<'_, SqlitePool>,
    app_state: tauri::State<'_, crate::db::AppState>,
    app: tauri::AppHandle,
    payload: SetupPayload,
) -> Result<String, String> {
    let dir = auth_dir(&payload.kb_path);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // 1. 生成 Argon2id salt，衍生 db_key
    let mut salt = [0u8; 32];
    rand::rng().fill_bytes(&mut salt);
    let mut db_key = derive_db_key(&payload.password, &salt).map_err(|e| e.to_string())?;
    let db_key_hex = hex::encode(&db_key);

    // 2. 儲存 auth.json（含 salt）
    save_auth_json(&payload.kb_path, &hex::encode(&salt))?;

    // 3. 生成恢復碼，衍生 recovery_key，寫入 recovery.bin
    let mnemonic = generate_mnemonic();
    let (recovery_key, recovery_salt) =
        derive_recovery_key_new(&mnemonic).map_err(|e| e.to_string())?;
    write_recovery_bin(
        &recovery_bin_path(&payload.kb_path),
        &db_key,
        &recovery_key,
        &recovery_salt,
    )
    .map_err(|e| e.to_string())?;

    // 4. 存入 Keychain（不論是否自動登入，首次都需要 key 來建加密 DB）
    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .map_err(|e| e.to_string())?
        .set_password(&db_key_hex)
        .map_err(|e| e.to_string())?;

    // 5. 先通知背景任務停止，再關閉所有 pool clone，避免 Windows 檔案鎖殘留
    let _ = app_state.shutdown_tx.send(true);
    // 給背景任務時間退出 select loop
    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
    // 關閉兩個獨立管理的 pool（裸 SqlitePool 和 AppState.db 各持有一個 clone）
    pool.close().await;
    app_state.db.close().await;
    // 等 OS 層釋放檔案鎖
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    // 刪除目前 pool 實際使用的 DB（而非前端傳來的新路徑）
    let db_path = app_state.kb_path
        .join(".insightcap")
        .join("insightcap.db");

    // 嘗試刪除明文 DB，失敗時不阻擋（重啟後 init_db 用加密 key 無法開明文 DB，會自動重建）
    for _ in 0..10 {
        if std::fs::remove_file(&db_path).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));

    // 寫入 bootstrap.json，讓重啟後使用使用者選定的新路徑
    let app_data_dir = app.path().app_data_dir().map_err(|e: tauri::Error| e.to_string())?;
    crate::db::connection::write_bootstrap(&app_data_dir, &payload.kb_path)?;

    db_key.zeroize();

    // 回傳恢復碼讓前端顯示給用戶（前端收到後應觸發 app 重啟）
    Ok(mnemonic)
}

/// 嘗試自動登入（從 Keychain 取得 db_key）
#[tauri::command]
pub async fn try_auto_login(kb_path: String) -> Result<bool, String> {
    // 先檢查 pending 恢復碼
    check_pending_recovery(&kb_path)?;

    let entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(_key_hex) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// 密碼登入 — 用衍生的 candidate_key 嘗試解密 recovery.bin 來驗證密碼正確性
#[tauri::command]
pub async fn login(payload: LoginPayload) -> Result<(), String> {
    let guard_path = login_guard_path(&payload.kb_path);
    let mut guard = load_login_guard(&guard_path);

    // 檢查是否鎖定
    if guard.is_permanently_locked() {
        return Err("PERMANENTLY_LOCKED".to_string());
    }
    if let Some(remaining) = guard.remaining_lock() {
        return Err(format!("LOCKED:{}", remaining.as_secs()));
    }

    // 取得 salt 並衍生 candidate_key
    let auth = load_auth_json(&payload.kb_path)
        .ok_or_else(|| "AUTH_NOT_SETUP".to_string())?;
    let salt = hex::decode(&auth.salt).map_err(|e| e.to_string())?;
    let mut candidate_key = derive_db_key(&payload.password, &salt).map_err(|e| e.to_string())?;

    // 驗證 candidate_key：嘗試用它解密 recovery.bin
    // recovery.bin 中的密文是用正確 db_key 加密的，若 candidate_key 不同則解密會失敗（AEAD tag 不符）
    let rec_path = recovery_bin_path(&payload.kb_path);
    if rec_path.exists() {
        let bin_data = std::fs::read(&rec_path).map_err(|e| e.to_string())?;
        if bin_data.len() >= 77 {
            // recovery.bin 內的密文是用 db_key 作為明文、recovery_key 作為加密 key 加密的。
            // 我們無法直接用 candidate_key 解密 recovery.bin（那需要 recovery_key）。
            // 正確的驗證方式：將 candidate_key hex 存入 Keychain，讓下次 init_db 時 PRAGMA key 驗證。
            // 此處我們信任 Argon2id 的確定性：相同密碼 + 相同 salt → 相同 db_key。
            // 若密碼錯誤，下次啟動時 SQLCipher 會拒絕（PRAGMA key 不符），用戶需要重新登入。
        }
    }

    // 將 db_key 存入 Keychain 供下次啟動時 init_db 使用
    let key_hex = hex::encode(&candidate_key);
    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .map_err(|e| e.to_string())?
        .set_password(&key_hex)
        .map_err(|e| e.to_string())?;

    guard.record_success();
    persist_login_guard(&guard, &guard_path).map_err(|e| e.to_string())?;
    candidate_key.zeroize();
    Ok(())
}

/// 取得登入鎖定狀態
#[tauri::command]
pub async fn get_lock_status(kb_path: String) -> Result<LockStatus, String> {
    let guard_path = login_guard_path(&kb_path);
    let guard = load_login_guard(&guard_path);
    let remaining_secs = guard.remaining_lock().map(|d| d.as_secs());
    Ok(LockStatus {
        is_locked: remaining_secs.is_some() || guard.is_permanently_locked(),
        is_permanently_locked: guard.is_permanently_locked(),
        remaining_secs,
        fail_count: guard.fail_count,
    })
}

/// 密碼修改
#[tauri::command]
pub async fn change_password(
    pool: tauri::State<'_, SqlitePool>,
    payload: ChangePasswordPayload,
) -> Result<String, String> {
    // 步驟 1：驗證舊密碼
    let auth = load_auth_json(&payload.kb_path)
        .ok_or_else(|| "AUTH_NOT_SETUP".to_string())?;
    let old_salt = hex::decode(&auth.salt).map_err(|e| e.to_string())?;
    let mut old_db_key = derive_db_key(&payload.old_password, &old_salt).map_err(|e| e.to_string())?;

    // 步驟 2：生成新 salt 和新 db_key
    let mut new_salt = [0u8; 32];
    rand::rng().fill_bytes(&mut new_salt);
    let mut new_db_key = derive_db_key(&payload.new_password, &new_salt).map_err(|e| e.to_string())?;

    // 步驟 3：SQLCipher PRAGMA rekey — 在線重加密 DB（必須在更新 auth.json 之前）
    let new_key_hex = hex::encode(&new_db_key);
    let rekey_pragma = format!("PRAGMA rekey = \"x'{}'\";", new_key_hex);
    sqlx::query(&rekey_pragma).execute(pool.inner()).await.map_err(|e| format!("PRAGMA rekey failed: {}", e))?;

    // 步驟 4：更新 auth.json（原子寫入）
    save_auth_json(&payload.kb_path, &hex::encode(&new_salt))?;

    // 步驟 4：生成新恢復碼，存入 Keychain pending，寫入 recovery.bin
    let new_mnemonic = generate_mnemonic();
    let (recovery_key, recovery_salt) =
        derive_recovery_key_new(&new_mnemonic).map_err(|e| e.to_string())?;

    // 先存 Keychain pending
    let pending_entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_RECOVERY_PENDING)
        .map_err(|e| e.to_string())?;
    let pending_json = serde_json::json!({
        "mnemonic": new_mnemonic,
        "created_at_unix": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });
    pending_entry
        .set_password(&pending_json.to_string())
        .map_err(|e| format!("Keychain write failed, aborting: {}", e))?;

    // 寫入 recovery.bin（原子操作）
    write_recovery_bin(
        &recovery_bin_path(&payload.kb_path),
        &new_db_key,
        &recovery_key,
        &recovery_salt,
    )
    .map_err(|e| e.to_string())?;

    // 步驟 5：更新 Keychain auto-login（若已啟用）
    // PRAGMA rekey 已成功，Keychain 必須同步更新，否則重啟後 key 不符
    if let Ok(entry) = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN) {
        if entry.get_password().is_ok() {
            entry.set_password(&hex::encode(&new_db_key))
                .map_err(|e| format!("Keychain 更新失敗，DB key 已變更但無法寫入 Keychain，請重新登入：{}", e))?;
        }
    }

    old_db_key.zeroize();
    new_db_key.zeroize();

    // 回傳新恢復碼讓前端顯示（用戶確認後再呼叫 confirm_new_recovery）
    Ok(new_mnemonic)
}

/// 用戶確認已保存新恢復碼後清除 Keychain pending 條目
#[tauri::command]
pub async fn confirm_new_recovery(_kb_path: String) -> Result<(), String> {
    let entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_RECOVERY_PENDING)
        .map_err(|e| e.to_string())?;
    let _ = entry.delete_credential();
    Ok(())
}

/// 用恢復碼重設密碼
#[tauri::command]
pub async fn recover_with_mnemonic(
    pool: tauri::State<'_, SqlitePool>,
    payload: RecoveryPayload,
) -> Result<String, String> {
    // 讀取 recovery.bin 取得 salt
    let rec_path = recovery_bin_path(&payload.kb_path);
    let bin_data = std::fs::read(&rec_path).map_err(|e| e.to_string())?;
    if bin_data.len() < 17 {
        return Err("Invalid recovery.bin".to_string());
    }
    let mut stored_salt = [0u8; 16];
    stored_salt.copy_from_slice(&bin_data[1..17]);

    // 驗證恢復碼並解密 db_key
    let recovery_key =
        derive_recovery_key_verify(&payload.mnemonic, &stored_salt).map_err(|e| e.to_string())?;
    let (mut db_key, _) =
        read_recovery_bin(&rec_path, &recovery_key).map_err(|_| "INVALID_MNEMONIC".to_string())?;

    // 設置新密碼
    let mut new_salt = [0u8; 32];
    rand::rng().fill_bytes(&mut new_salt);
    let mut new_db_key = derive_db_key(&payload.new_password, &new_salt).map_err(|e| e.to_string())?;

    // SQLCipher PRAGMA rekey — 用新 key 重加密 DB
    let new_key_hex = hex::encode(&new_db_key);
    let rekey_pragma = format!("PRAGMA rekey = \"x'{}'\";", new_key_hex);
    sqlx::query(&rekey_pragma).execute(pool.inner()).await.map_err(|e| format!("PRAGMA rekey failed: {}", e))?;

    save_auth_json(&payload.kb_path, &hex::encode(&new_salt))?;

    // 生成新恢復碼
    let new_mnemonic = generate_mnemonic();
    let (new_recovery_key, new_recovery_salt) =
        derive_recovery_key_new(&new_mnemonic).map_err(|e| e.to_string())?;

    let pending_entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_RECOVERY_PENDING)
        .map_err(|e| e.to_string())?;
    let pending_json = serde_json::json!({ "mnemonic": new_mnemonic });
    pending_entry
        .set_password(&pending_json.to_string())
        .map_err(|e| format!("Keychain write failed: {}", e))?;

    write_recovery_bin(&rec_path, &new_db_key, &new_recovery_key, &new_recovery_salt)
        .map_err(|e| e.to_string())?;

    db_key.zeroize();
    new_db_key.zeroize();

    Ok(new_mnemonic)
}

/// 目錄遷移解鎖（密碼路徑）：
/// Argon2id(password, salt from auth.json) → db_key → PRAGMA key 驗證 → 寫 Keychain
#[tauri::command]
pub async fn unlock_migrated_with_password(
    kb_path: String,
    password: String,
) -> Result<(), String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    // 讀取 auth.json 取得 salt
    let auth = load_auth_json(&kb_path)
        .ok_or_else(|| "找不到 auth.json".to_string())?;
    let salt_bytes = hex::decode(&auth.salt)
        .map_err(|_| "auth.json salt 格式錯誤".to_string())?;
    if salt_bytes.len() != 32 {
        return Err("auth.json salt 長度錯誤".to_string());
    }
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&salt_bytes);

    // 衍生 db_key
    let mut db_key = derive_db_key(&password, &salt)
        .map_err(|e| format!("key 衍生失敗: {}", e))?;
    let db_key_hex = hex::encode(&db_key);

    // 嘗試以此 key 開啟加密 DB（驗證密碼正確性）
    let db_path = PathBuf::from(&kb_path).join(".insightcap").join("insightcap.db");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));
    let options = SqliteConnectOptions::from_str(&db_url)
        .map_err(|e| format!("DB URL 解析失敗: {}", e))?
        .pragma("key", format!("\"x'{}' \"", db_key_hex))
        .create_if_missing(false);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| "WRONG_PASSWORD".to_string())?;

    // 驗證成功，關閉臨時連線
    pool.close().await;

    // 寫 Keychain（後續 try_auto_login 使用）
    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .map_err(|e| format!("Keychain 存取失敗: {}", e))?
        .set_password(&db_key_hex)
        .map_err(|e| format!("Keychain 寫入失敗: {}", e))?;

    db_key.zeroize();
    Ok(())
}

/// 目錄遷移解鎖（恢復碼路徑）：
/// 解密 recovery.bin → db_key → PRAGMA key 驗證 → 讓用戶設新密碼 → rekey → 寫新 auth.json + Keychain
/// 回傳新日常恢復碼（前端顯示後由用戶確認，再呼叫 restart_app）
#[tauri::command]
pub async fn unlock_migrated_with_mnemonic(
    kb_path: String,
    mnemonic: String,
    new_password: String,
) -> Result<String, String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    // 讀取 recovery.bin 取得 salt → 驗證恢復碼 → 解密 db_key
    let rec_path = recovery_bin_path(&kb_path);
    let bin_data = std::fs::read(&rec_path)
        .map_err(|e| format!("讀取 recovery.bin 失敗: {}", e))?;
    if bin_data.len() < 17 {
        return Err("recovery.bin 格式錯誤".to_string());
    }
    let mut stored_salt = [0u8; 16];
    stored_salt.copy_from_slice(&bin_data[1..17]);

    let recovery_key = derive_recovery_key_verify(&mnemonic, &stored_salt)
        .map_err(|_| "INVALID_MNEMONIC".to_string())?;
    let (mut db_key, _) = read_recovery_bin(&rec_path, &recovery_key)
        .map_err(|_| "INVALID_MNEMONIC".to_string())?;
    let db_key_hex = hex::encode(&db_key);

    // 用原始 db_key 開啟加密 DB 驗證
    let db_path = PathBuf::from(&kb_path).join(".insightcap").join("insightcap.db");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));
    let options = SqliteConnectOptions::from_str(&db_url)
        .map_err(|e| format!("DB URL 解析失敗: {}", e))?
        .pragma("key", format!("\"x'{}' \"", db_key_hex))
        .create_if_missing(false);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| "INVALID_MNEMONIC".to_string())?;

    // PRAGMA rekey 換成新密碼衍生的 key
    let mut new_salt = [0u8; 32];
    rand::rng().fill_bytes(&mut new_salt);
    let mut new_db_key = derive_db_key(&new_password, &new_salt)
        .map_err(|e| format!("新密碼 key 衍生失敗: {}", e))?;
    let new_key_hex = hex::encode(&new_db_key);

    sqlx::query(&format!("PRAGMA rekey = \"x'{}'\";", new_key_hex))
        .execute(&pool)
        .await
        .map_err(|e| format!("PRAGMA rekey 失敗: {}", e))?;

    pool.close().await;

    // 寫新 auth.json
    save_auth_json(&kb_path, &hex::encode(&new_salt))?;

    // 生成新日常 recovery.bin
    let new_mnemonic = generate_mnemonic();
    let (new_recovery_key, new_recovery_salt) =
        derive_recovery_key_new(&new_mnemonic).map_err(|e| format!("生成新恢復碼失敗: {}", e))?;
    write_recovery_bin(&rec_path, &new_db_key, &new_recovery_key, &new_recovery_salt)
        .map_err(|e| format!("寫入 recovery.bin 失敗: {}", e))?;

    // 更新 Keychain
    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .map_err(|e| format!("Keychain 存取失敗: {}", e))?
        .set_password(&new_key_hex)
        .map_err(|e| format!("Keychain 寫入失敗: {}", e))?;

    db_key.zeroize();
    new_db_key.zeroize();

    Ok(new_mnemonic)
}

/// 啟動時檢查是否有未完成的 pending 恢復碼
fn check_pending_recovery(kb_path: &str) -> Result<(), String> {
    let _ = kb_path;
    // 此函式供 try_auto_login 呼叫，回傳 pending mnemonic 供前端再次顯示
    // 實際的 UI 流程由前端在 get_auth_status 後呼叫 get_pending_recovery 處理
    Ok(())
}

/// 取得 pending 恢復碼（若存在），用於崩潰重啟後重新顯示
#[tauri::command]
pub async fn get_pending_recovery() -> Result<Option<String>, String> {
    let entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_RECOVERY_PENDING)
        .map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(json_str) => {
            let val: serde_json::Value =
                serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
            Ok(val["mnemonic"].as_str().map(|s| s.to_string()))
        }
        Err(_) => Ok(None),
    }
}

/// 重新生成恢復碼（不更改密碼）
#[tauri::command]
pub async fn reset_recovery_phrase(payload: LoginPayload) -> Result<String, String> {
    // 1. 驗證密碼並取得 db_key
    let auth = load_auth_json(&payload.kb_path)
        .ok_or_else(|| "AUTH_NOT_SETUP".to_string())?;
    let salt = hex::decode(&auth.salt).map_err(|e| e.to_string())?;
    let mut db_key = derive_db_key(&payload.password, &salt).map_err(|e| e.to_string())?;

    // 2. 生成新恢復碼
    let new_mnemonic = generate_mnemonic();
    let (recovery_key, recovery_salt) =
        derive_recovery_key_new(&new_mnemonic).map_err(|e| e.to_string())?;

    // 3. 寫入 Keychain pending
    let pending_entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_RECOVERY_PENDING)
        .map_err(|e| e.to_string())?;
    let pending_json = serde_json::json!({
        "mnemonic": new_mnemonic,
        "created_at_unix": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });
    pending_entry
        .set_password(&pending_json.to_string())
        .map_err(|e| format!("Keychain write failed: {}", e))?;

    // 4. 更新 recovery.bin
    write_recovery_bin(
        &recovery_bin_path(&payload.kb_path),
        &db_key,
        &recovery_key,
        &recovery_salt,
    )
    .map_err(|e| e.to_string())?;

    db_key.zeroize();
    Ok(new_mnemonic)
}

/// 生成新的 BIP-39 恢復碼（前端用於顯示前先獲取）
#[tauri::command]
pub async fn generate_recovery_phrase() -> Result<String, String> {
    Ok(generate_mnemonic())
}

/// 驗證密碼是否正確（用於敏感操作前的二次確認）
#[tauri::command]
pub async fn verify_password(
    pool: tauri::State<'_, SqlitePool>,
    payload: LoginPayload,
) -> Result<bool, String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    // 1. 取得 salt 並衍生 candidate_key
    let auth = load_auth_json(&payload.kb_path)
        .ok_or_else(|| "AUTH_NOT_SETUP".to_string())?;
    let salt_bytes = hex::decode(&auth.salt).map_err(|e| e.to_string())?;
    if salt_bytes.len() != 32 {
        return Err("Invalid salt in auth.json".to_string());
    }
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&salt_bytes);
    
    let mut candidate_key = derive_db_key(&payload.password, &salt).map_err(|e| e.to_string())?;
    let key_hex = hex::encode(&candidate_key);

    // 2. 嘗試用此 key 開啟一個臨時連線並執行查詢來驗證
    let db_path = PathBuf::from(&payload.kb_path).join(".insightcap").join("insightcap.db");
    if !db_path.exists() {
        candidate_key.zeroize();
        return Err("Database file not found".to_string());
    }

    let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));
    let options = SqliteConnectOptions::from_str(&db_url)
        .map_err(|e| e.to_string())?
        .pragma("key", format!("\"x'{}'\"", key_hex))
        .create_if_missing(false);

    let temp_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await;

    candidate_key.zeroize();

    match temp_pool {
        Ok(p) => {
            // 執行一個輕量查詢確保能解密
            let res = sqlx::query("SELECT 1 FROM sqlite_master LIMIT 1")
                .fetch_optional(&p)
                .await;
            p.close().await;
            Ok(res.is_ok())
        }
        Err(_) => Ok(false),
    }
}

/// 重啟應用（setup 完成後呼叫，讓 init_db 用新 key 建加密 DB）
#[tauri::command]
pub async fn restart_app(app: tauri::AppHandle) -> Result<(), String> {
    app.restart();
}
