use keyring::Entry;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::PathBuf;
use tauri::Manager;
use zeroize::Zeroize;

use crate::auth::key_derivation::generate_mnemonic;
use crate::auth::{
    derive_db_key, derive_recovery_key_new, derive_recovery_key_verify, load_login_guard,
    persist_login_guard, read_recovery_bin, write_recovery_bin,
};

const KEYCHAIN_SERVICE: &str = "insightcap";
const KEYCHAIN_AUTO_LOGIN: &str = "auto_login_key";
const KEYCHAIN_RECOVERY_PENDING: &str = "recovery_pending_v1";


#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub is_setup: bool,
    pub auto_login: bool,
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
    salt: String, // hex-encoded 32-byte Argon2id salt
}

fn load_auth_json(kb_path: &str) -> Option<AuthJson> {
    let content = std::fs::read_to_string(auth_json_path(kb_path)).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_auth_json(kb_path: &str, salt_hex: &str) -> Result<(), String> {
    let dir = auth_dir(kb_path);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let auth = AuthJson {
        version: 1,
        salt: salt_hex.to_string(),
    };
    let json = serde_json::to_string_pretty(&auth).map_err(|e| e.to_string())?;

    let tmp = auth_json_path(kb_path).with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, auth_json_path(kb_path)).map_err(|e| e.to_string())?;
    Ok(())
}


#[tauri::command]
pub async fn get_auth_status(kb_path: String) -> Result<AuthStatus, String> {
    let auth_exists = auth_json_path(&kb_path).exists();
    let recovery_exists = recovery_bin_path(&kb_path).exists();
    let db_exists = PathBuf::from(&kb_path)
        .join(".insightcap")
        .join("insightcap.db")
        .exists();

    let is_setup = auth_exists && recovery_exists;

    let keychain_ok = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .and_then(|e| e.get_password())
        .is_ok();

    let is_migrated = is_setup && db_exists && !keychain_ok;

    Ok(AuthStatus {
        is_setup,
        auto_login: keychain_ok,
        is_migrated,
    })
}

#[tauri::command]
pub async fn setup_auth(
    pool: tauri::State<'_, SqlitePool>,
    app_state: tauri::State<'_, crate::db::AppState>,
    app: tauri::AppHandle,
    payload: SetupPayload,
) -> Result<String, String> {
    let dir = auth_dir(&payload.kb_path);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut salt = [0u8; 32];
    rand::rng().fill_bytes(&mut salt);
    let mut db_key = derive_db_key(&payload.password, &salt).map_err(|e| e.to_string())?;
    let db_key_hex = hex::encode(&db_key);

    save_auth_json(&payload.kb_path, &hex::encode(&salt))?;

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

    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .map_err(|e| e.to_string())?
        .set_password(&db_key_hex)
        .map_err(|e| e.to_string())?;

    let _ = app_state.shutdown_tx.send(true);
    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
    pool.close().await;
    app_state.db.close().await;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let db_path = app_state.kb_path.join(".insightcap").join("insightcap.db");

    for _ in 0..10 {
        if std::fs::remove_file(&db_path).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e: tauri::Error| e.to_string())?;
    crate::db::connection::write_bootstrap(&app_data_dir, &payload.kb_path)?;

    db_key.zeroize();

    Ok(mnemonic)
}

#[tauri::command]
pub async fn try_auto_login(kb_path: String) -> Result<bool, String> {
    check_pending_recovery(&kb_path)?;

    let entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(_key_hex) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub async fn login(payload: LoginPayload) -> Result<(), String> {
    let guard_path = login_guard_path(&payload.kb_path);
    let mut guard = load_login_guard(&guard_path);

    if guard.is_permanently_locked() {
        return Err("PERMANENTLY_LOCKED".to_string());
    }
    if let Some(remaining) = guard.remaining_lock() {
        return Err(format!("LOCKED:{}", remaining.as_secs()));
    }

    let auth = load_auth_json(&payload.kb_path).ok_or_else(|| "AUTH_NOT_SETUP".to_string())?;
    let salt = hex::decode(&auth.salt).map_err(|e| e.to_string())?;
    let mut candidate_key = derive_db_key(&payload.password, &salt).map_err(|e| e.to_string())?;

    let rec_path = recovery_bin_path(&payload.kb_path);
    if rec_path.exists() {
        let bin_data = std::fs::read(&rec_path).map_err(|e| e.to_string())?;
        if bin_data.len() >= 77 {
        }
    }

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

#[tauri::command]
pub async fn change_password(
    pool: tauri::State<'_, SqlitePool>,
    payload: ChangePasswordPayload,
) -> Result<String, String> {
    let auth = load_auth_json(&payload.kb_path).ok_or_else(|| "AUTH_NOT_SETUP".to_string())?;
    let old_salt = hex::decode(&auth.salt).map_err(|e| e.to_string())?;
    let mut old_db_key =
        derive_db_key(&payload.old_password, &old_salt).map_err(|e| e.to_string())?;

    let mut new_salt = [0u8; 32];
    rand::rng().fill_bytes(&mut new_salt);
    let mut new_db_key =
        derive_db_key(&payload.new_password, &new_salt).map_err(|e| e.to_string())?;

    let new_key_hex = hex::encode(&new_db_key);
    let rekey_pragma = format!("PRAGMA rekey = \"x'{}'\";", new_key_hex);
    sqlx::query(&rekey_pragma)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("PRAGMA rekey failed: {}", e))?;

    save_auth_json(&payload.kb_path, &hex::encode(&new_salt))?;

    let new_mnemonic = generate_mnemonic();
    let (recovery_key, recovery_salt) =
        derive_recovery_key_new(&new_mnemonic).map_err(|e| e.to_string())?;

    let pending_entry =
        Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_RECOVERY_PENDING).map_err(|e| e.to_string())?;
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

    write_recovery_bin(
        &recovery_bin_path(&payload.kb_path),
        &new_db_key,
        &recovery_key,
        &recovery_salt,
    )
    .map_err(|e| e.to_string())?;

    if let Ok(entry) = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN) {
        if entry.get_password().is_ok() {
            entry
                .set_password(&hex::encode(&new_db_key))
                .map_err(|e| format!("Keychain      DB key          Keychain       {}", e))?;
        }
    }

    old_db_key.zeroize();
    new_db_key.zeroize();

    Ok(new_mnemonic)
}

#[tauri::command]
pub async fn confirm_new_recovery(_kb_path: String) -> Result<(), String> {
    let entry =
        Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_RECOVERY_PENDING).map_err(|e| e.to_string())?;
    let _ = entry.delete_credential();
    Ok(())
}

#[tauri::command]
pub async fn recover_with_mnemonic(
    pool: tauri::State<'_, SqlitePool>,
    payload: RecoveryPayload,
) -> Result<String, String> {
    let rec_path = recovery_bin_path(&payload.kb_path);
    let bin_data = std::fs::read(&rec_path).map_err(|e| e.to_string())?;
    if bin_data.len() < 17 {
        return Err("Invalid recovery.bin".to_string());
    }
    let mut stored_salt = [0u8; 16];
    stored_salt.copy_from_slice(&bin_data[1..17]);

    let recovery_key =
        derive_recovery_key_verify(&payload.mnemonic, &stored_salt).map_err(|e| e.to_string())?;
    let (mut db_key, _) =
        read_recovery_bin(&rec_path, &recovery_key).map_err(|_| "INVALID_MNEMONIC".to_string())?;

    let mut new_salt = [0u8; 32];
    rand::rng().fill_bytes(&mut new_salt);
    let mut new_db_key =
        derive_db_key(&payload.new_password, &new_salt).map_err(|e| e.to_string())?;

    let new_key_hex = hex::encode(&new_db_key);
    let rekey_pragma = format!("PRAGMA rekey = \"x'{}'\";", new_key_hex);
    sqlx::query(&rekey_pragma)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("PRAGMA rekey failed: {}", e))?;

    save_auth_json(&payload.kb_path, &hex::encode(&new_salt))?;

    let new_mnemonic = generate_mnemonic();
    let (new_recovery_key, new_recovery_salt) =
        derive_recovery_key_new(&new_mnemonic).map_err(|e| e.to_string())?;

    let pending_entry =
        Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_RECOVERY_PENDING).map_err(|e| e.to_string())?;
    let pending_json = serde_json::json!({ "mnemonic": new_mnemonic });
    pending_entry
        .set_password(&pending_json.to_string())
        .map_err(|e| format!("Keychain write failed: {}", e))?;

    write_recovery_bin(
        &rec_path,
        &new_db_key,
        &new_recovery_key,
        &new_recovery_salt,
    )
    .map_err(|e| e.to_string())?;

    db_key.zeroize();
    new_db_key.zeroize();

    Ok(new_mnemonic)
}

#[tauri::command]
pub async fn unlock_migrated_with_password(
    kb_path: String,
    password: String,
) -> Result<(), String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    let auth = load_auth_json(&kb_path).ok_or_else(|| "    auth.json".to_string())?;
    let salt_bytes = hex::decode(&auth.salt).map_err(|_| "auth.json salt     ".to_string())?;
    if salt_bytes.len() != 32 {
        return Err("auth.json salt     ".to_string());
    }
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&salt_bytes);

    let mut db_key = derive_db_key(&password, &salt).map_err(|e| format!("key     : {}", e))?;
    let db_key_hex = hex::encode(&db_key);

    let db_path = PathBuf::from(&kb_path)
        .join(".insightcap")
        .join("insightcap.db");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));
    let options = SqliteConnectOptions::from_str(&db_url)
        .map_err(|e| format!("DB URL     : {}", e))?
        .pragma("key", format!("\"x'{}' \"", db_key_hex))
        .create_if_missing(false);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| "WRONG_PASSWORD".to_string())?;

    pool.close().await;

    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .map_err(|e| format!("Keychain     : {}", e))?
        .set_password(&db_key_hex)
        .map_err(|e| format!("Keychain     : {}", e))?;

    db_key.zeroize();
    Ok(())
}

#[tauri::command]
pub async fn unlock_migrated_with_mnemonic(
    kb_path: String,
    mnemonic: String,
    new_password: String,
) -> Result<String, String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    let rec_path = recovery_bin_path(&kb_path);
    let bin_data = std::fs::read(&rec_path).map_err(|e| format!("   recovery.bin   : {}", e))?;
    if bin_data.len() < 17 {
        return Err("recovery.bin     ".to_string());
    }
    let mut stored_salt = [0u8; 16];
    stored_salt.copy_from_slice(&bin_data[1..17]);

    let recovery_key = derive_recovery_key_verify(&mnemonic, &stored_salt)
        .map_err(|_| "INVALID_MNEMONIC".to_string())?;
    let (mut db_key, _) =
        read_recovery_bin(&rec_path, &recovery_key).map_err(|_| "INVALID_MNEMONIC".to_string())?;
    let db_key_hex = hex::encode(&db_key);

    let db_path = PathBuf::from(&kb_path)
        .join(".insightcap")
        .join("insightcap.db");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));
    let options = SqliteConnectOptions::from_str(&db_url)
        .map_err(|e| format!("DB URL     : {}", e))?
        .pragma("key", format!("\"x'{}' \"", db_key_hex))
        .create_if_missing(false);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| "INVALID_MNEMONIC".to_string())?;

    let mut new_salt = [0u8; 32];
    rand::rng().fill_bytes(&mut new_salt);
    let mut new_db_key =
        derive_db_key(&new_password, &new_salt).map_err(|e| format!("    key     : {}", e))?;
    let new_key_hex = hex::encode(&new_db_key);

    sqlx::query(&format!("PRAGMA rekey = \"x'{}'\";", new_key_hex))
        .execute(&pool)
        .await
        .map_err(|e| format!("PRAGMA rekey   : {}", e))?;

    pool.close().await;

    save_auth_json(&kb_path, &hex::encode(&new_salt))?;

    let new_mnemonic = generate_mnemonic();
    let (new_recovery_key, new_recovery_salt) =
        derive_recovery_key_new(&new_mnemonic).map_err(|e| format!("        : {}", e))?;
    write_recovery_bin(
        &rec_path,
        &new_db_key,
        &new_recovery_key,
        &new_recovery_salt,
    )
    .map_err(|e| format!("   recovery.bin   : {}", e))?;

    Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_AUTO_LOGIN)
        .map_err(|e| format!("Keychain     : {}", e))?
        .set_password(&new_key_hex)
        .map_err(|e| format!("Keychain     : {}", e))?;

    db_key.zeroize();
    new_db_key.zeroize();

    Ok(new_mnemonic)
}

fn check_pending_recovery(kb_path: &str) -> Result<(), String> {
    let _ = kb_path;
    Ok(())
}

#[tauri::command]
pub async fn get_pending_recovery() -> Result<Option<String>, String> {
    let entry =
        Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_RECOVERY_PENDING).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(json_str) => {
            let val: serde_json::Value =
                serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
            Ok(val["mnemonic"].as_str().map(|s| s.to_string()))
        }
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub async fn reset_recovery_phrase(payload: LoginPayload) -> Result<String, String> {
    let auth = load_auth_json(&payload.kb_path).ok_or_else(|| "AUTH_NOT_SETUP".to_string())?;
    let salt = hex::decode(&auth.salt).map_err(|e| e.to_string())?;
    let mut db_key = derive_db_key(&payload.password, &salt).map_err(|e| e.to_string())?;

    let new_mnemonic = generate_mnemonic();
    let (recovery_key, recovery_salt) =
        derive_recovery_key_new(&new_mnemonic).map_err(|e| e.to_string())?;

    let pending_entry =
        Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_RECOVERY_PENDING).map_err(|e| e.to_string())?;
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

#[tauri::command]
pub async fn generate_recovery_phrase() -> Result<String, String> {
    Ok(generate_mnemonic())
}

#[tauri::command]
pub async fn verify_password(
    _pool: tauri::State<'_, SqlitePool>,
    payload: LoginPayload,
) -> Result<bool, String> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    let auth = load_auth_json(&payload.kb_path).ok_or_else(|| "AUTH_NOT_SETUP".to_string())?;
    let salt_bytes = hex::decode(&auth.salt).map_err(|e| e.to_string())?;
    if salt_bytes.len() != 32 {
        return Err("Invalid salt in auth.json".to_string());
    }
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&salt_bytes);

    let mut candidate_key = derive_db_key(&payload.password, &salt).map_err(|e| e.to_string())?;
    let key_hex = hex::encode(&candidate_key);

    let db_path = PathBuf::from(&payload.kb_path)
        .join(".insightcap")
        .join("insightcap.db");
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
            let res = sqlx::query("SELECT 1 FROM sqlite_master LIMIT 1")
                .fetch_optional(&p)
                .await;
            p.close().await;
            Ok(res.is_ok())
        }
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub async fn restart_app(app: tauri::AppHandle) -> Result<(), String> {
    app.restart();
}
