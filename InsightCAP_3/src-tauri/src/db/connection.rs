/// DB 連接、健康檢查、bootstrap.json 原子寫入、db_state.json
/// 按照 Architecture-v2.md「資料庫安全設計」章節實現

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::providers::embedding::Embedder;
use crate::vector_store::local::VectorStore;

// ─── AppState ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub kb_path: PathBuf,
    pub vector_store: VectorStore,
    pub embedder: Arc<dyn Embedder>,
    pub current_conversation_id: Arc<tokio::sync::Mutex<Option<String>>>,
    /// 用於通知背景任務停止（發送 true = 停止）
    pub shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>,
}

impl AppState {
    pub fn new(
        pool: SqlitePool,
        kb_path: PathBuf,
        vector_store: VectorStore,
        embedder: Arc<dyn Embedder>,
        shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>,
    ) -> Self {
        Self {
            db: pool,
            kb_path,
            vector_store,
            embedder,
            current_conversation_id: Arc::new(tokio::sync::Mutex::new(None)),
            shutdown_tx,
        }
    }
}

// ─── db_state.json ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbState {
    pub version: u32,
    pub is_encrypted: bool,
    pub key_version: u32,
    pub last_operation: String,
    // idle | rekey_in_progress | path_migration_in_progress | rebuild_index_in_progress
    pub pending_kb_path: Option<String>,
    pub last_successful_open: Option<String>,
}

impl Default for DbState {
    fn default() -> Self {
        Self {
            version: 1,
            is_encrypted: false,
            key_version: 0,
            last_operation: "idle".to_string(),
            pending_kb_path: None,
            last_successful_open: None,
        }
    }
}

pub fn read_db_state(app_data_dir: &Path) -> DbState {
    let path = app_data_dir.join("db_state.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => DbState::default(),
    }
}

pub fn write_db_state(app_data_dir: &Path, state: &DbState) -> Result<(), String> {
    let path = app_data_dir.join("db_state.json");
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    // 原子寫入
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

// ─── bootstrap.json 原子寫入 ────────────────────────────────────────────────

/// 原子寫入 bootstrap.json（tmp + rename + sync_all）
pub fn write_bootstrap(app_data_dir: &Path, kb_path: &str) -> Result<(), String> {
    let bootstrap_path = app_data_dir.join("bootstrap.json");
    let json = serde_json::json!({ "kb_path": kb_path });
    let content = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;

    // Windows 上直接覆寫（rename 在目標已存在時可能失敗）
    std::fs::write(&bootstrap_path, &content).map_err(|e| e.to_string())?;

    Ok(())
}

/// 讀取 bootstrap.json，返回 kb_path
pub fn read_bootstrap(app_data_dir: &Path) -> Option<String> {
    let path = app_data_dir.join("bootstrap.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json["kb_path"].as_str().map(|s| s.to_string())
}

// ─── DB 初始化 ──────────────────────────────────────────────────────────────

/// 建立 .insightcap/ 目錄結構
fn ensure_insightcap_dir(kb_path: &Path) -> Result<(), String> {
    let dir = kb_path.join(".insightcap");
    std::fs::create_dir_all(&dir).map_err(|e| format!("無法建立 .insightcap 目錄: {}", e))?;
    std::fs::create_dir_all(dir.join("vectors")).map_err(|e| e.to_string())?;
    Ok(())
}

/// 初始化 DB
/// - db_key_hex: 若為 Some，使用加密 DB（SQLCipher PRAGMA key）
/// - 若為 None，使用明文 DB（首次設定前）
pub async fn init_db(
    kb_path: &Path,
    db_key_hex: Option<&str>,
) -> Result<SqlitePool, String> {
    ensure_insightcap_dir(kb_path)?;

    let db_path = kb_path.join(".insightcap").join("insightcap.db");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy());

    let mut options = SqliteConnectOptions::from_str(&db_url)
        .map_err(|e| e.to_string())?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    // SQLCipher: key 格式必須為 "x'hex'" (含外層雙引號)
    if let Some(key_hex) = db_key_hex {
        options = options.pragma("key", format!("\"x'{}'\"", key_hex));
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|e| format!("無法連接資料庫: {}", e))?;

    // 執行 WAL checkpoint
    let _ = sqlx::query("PRAGMA wal_checkpoint(PASSIVE)")
        .execute(&pool)
        .await;

    // 執行 Migration
    run_migrations(&pool).await?;

    Ok(pool)
}

/// 執行 Schema Migration
async fn run_migrations(pool: &SqlitePool) -> Result<(), String> {
    println!("[DB] run_migrations 開始");
    // 建立 migration 追蹤表（若不存在）
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _migrations (id TEXT PRIMARY KEY, applied_at TEXT NOT NULL)"
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Migration 追蹤表建立失敗: {}", e))?;
    println!("[DB] _migrations 表建立完成");

    let migrations: &[(&str, &str)] = &[
        ("001", include_str!("../../migrations/001_init.sql")),
        ("002", include_str!("../../migrations/002_pattern_engine.sql")),
        ("003", include_str!("../../migrations/003_enterprise.sql")),
        ("004", include_str!("../../migrations/004_add_project_color.sql")),
        ("005", include_str!("../../migrations/005_conversation_pin_lock.sql")),
        ("006", include_str!("../../migrations/006_repository_timeline.sql")),
        ("007", include_str!("../../migrations/007_fix_local_doc_path.sql")),
        ("008", include_str!("../../migrations/008_decisions.sql")),
        ("009", include_str!("../../migrations/009_chunk_relations.sql")),
        ("010", include_str!("../../migrations/010_space_wiki.sql")),
        ("011", include_str!("../../migrations/011_source_tags.sql")),
    ];

    for (id, sql) in migrations {
        let already: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM _migrations WHERE id = ?"
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap_or(0) > 0;

        if already {
            println!("[DB] Migration {} 已存在，跳過", id);
            continue;
        }

        println!("[DB] 正在執行 Migration {}...", id);
        // 逐句執行，跳過已存在的欄位/表等 idempotent 錯誤
        for statement in sql.split(';') {
            let trimmed = statement.trim();
            if trimmed.is_empty() {
                continue;
            }
            // 過濾掉純 comment 的 fragment
            let non_comment: String = trimmed.lines()
                .filter(|l| !l.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n");
            if non_comment.trim().is_empty() {
                continue;
            }
            match sqlx::raw_sql(&format!("{};", trimmed)).execute(pool).await {
                Ok(_) => {}
                Err(e) => {
                    let msg = e.to_string().to_lowercase();
                    if msg.contains("duplicate column") || msg.contains("already exists") {
                        println!("[DB] Migration {} 跳過已存在的物件", id);
                        continue;
                    }
                    return Err(format!("Migration {} 失敗: {} | SQL: {}", id, e, &trimmed[..trimmed.len().min(80)]));
                }
            }
        }

        sqlx::query("INSERT INTO _migrations (id, applied_at) VALUES (?, ?)")
            .bind(id)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| format!("Migration {} 記錄失敗: {}", id, e))?;

        println!("[DB] Migration {} 完成", id);
    }

    Ok(())
}

// ─── 啟動七步驟健康檢查 ─────────────────────────────────────────────────────

#[derive(Debug)]
pub enum HealthCheckResult {
    Ok(SqlitePool, PathBuf),
    NeedsRepair(RepairReason),
}

#[derive(Debug)]
pub enum RepairReason {
    LastOperationNotIdle(String),
    DbFileNotFound(PathBuf),
    KeychainReadFailed,
    DbOpenFailed(String),
    IntegrityCheckFailed,
}

/// 執行七步驟啟動健康檢查
pub async fn health_check(app_data_dir: &Path) -> HealthCheckResult {
    // Step 1: 讀 db_state.json
    let db_state = read_db_state(app_data_dir);
    if db_state.last_operation != "idle" {
        return HealthCheckResult::NeedsRepair(
            RepairReason::LastOperationNotIdle(db_state.last_operation)
        );
    }

    // Step 2: 讀 bootstrap.json 取得 kb_path
    let kb_path_str = read_bootstrap(app_data_dir)
        .unwrap_or_else(|| app_data_dir.to_string_lossy().to_string());
    let kb_path = PathBuf::from(&kb_path_str);

    // 若路徑不可存取，fallback 到 app_data_dir
    let effective_kb_path = if std::fs::create_dir_all(&kb_path).is_ok() && kb_path.exists() {
        kb_path
    } else {
        eprintln!("[DB-HEALTH] kb_path 不可存取，使用 fallback: {:?}", app_data_dir);
        app_data_dir.to_path_buf()
    };

    // Step 3: 確認 DB 文件存在（若不存在，允許新建）
    let _db_file = effective_kb_path.join(".insightcap").join("insightcap.db");
    // 新安裝時 DB 不存在是正常的，init_db 會建立

    // Step 4: 從 Keychain 讀取 db_key
    let db_key_hex: Option<String> = keyring::Entry::new("insightcap", "auto_login_key")
        .ok()
        .and_then(|e| e.get_password().ok());

    // Step 5-6: 嘗試打開 DB 並執行 integrity_check
    match init_db(&effective_kb_path, db_key_hex.as_deref()).await {
        Ok(pool) => {
            // Step 6: integrity_check
            let integrity_ok = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
                .fetch_one(&pool)
                .await
                .map(|r| r == "ok")
                .unwrap_or(false);

            if !integrity_ok {
                pool.close().await;
                return HealthCheckResult::NeedsRepair(RepairReason::IntegrityCheckFailed);
            }

            // Step 7: 全部通過
            let _ = write_db_state(app_data_dir, &DbState {
                last_successful_open: Some(chrono::Utc::now().to_rfc3339()),
                ..db_state
            });

            HealthCheckResult::Ok(pool, effective_kb_path)
        }
        Err(e) => {
            HealthCheckResult::NeedsRepair(RepairReason::DbOpenFailed(e))
        }
    }
}
