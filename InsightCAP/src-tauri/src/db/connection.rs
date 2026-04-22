use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use crate::providers::embedding::Embedder;
use crate::vector_store::local::VectorStore;


#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub kb_path: PathBuf,
    pub vector_store: VectorStore,
    pub embedder: Arc<dyn Embedder>,
    pub current_conversation_id: Arc<tokio::sync::Mutex<Option<String>>>,
    pub shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>,
    pub reminder_loop_count: Arc<std::sync::atomic::AtomicU64>,
    pub summary_wakeup_tx: Arc<tokio::sync::Notify>,
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
            reminder_loop_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            summary_wakeup_tx: Arc::new(tokio::sync::Notify::new()),
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbState {
    pub version: u32,
    pub is_encrypted: bool,
    pub key_version: u32,
    pub last_operation: String,
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
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}


pub fn write_bootstrap(app_data_dir: &Path, kb_path: &str) -> Result<(), String> {
    let bootstrap_path = app_data_dir.join("bootstrap.json");
    let json = serde_json::json!({ "kb_path": kb_path });
    let content = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;

    std::fs::write(&bootstrap_path, &content).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn read_bootstrap(app_data_dir: &Path) -> Option<String> {
    let path = app_data_dir.join("bootstrap.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json["kb_path"].as_str().map(|s| s.to_string())
}


fn ensure_insightcap_dir(kb_path: &Path) -> Result<(), String> {
    let dir = kb_path.join(".insightcap");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create .insightcap dir: {}", e))?;
    std::fs::create_dir_all(dir.join("vectors")).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn init_db(kb_path: &Path, db_key_hex: Option<&str>) -> Result<SqlitePool, String> {
    ensure_insightcap_dir(kb_path)?;

    let db_path = kb_path.join(".insightcap").join("insightcap.db");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy().replace('\\', "/"));

    let mut options = SqliteConnectOptions::from_str(&db_url)
        .map_err(|e| e.to_string())?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    if let Some(key_hex) = db_key_hex {
        options = options.pragma("key", format!("\"x'{}'\"", key_hex));
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|e| format!("Failed to connect/open database: {}", e))?;

    let _ = sqlx::query("PRAGMA wal_checkpoint(PASSIVE)")
        .execute(&pool)
        .await;

    run_migrations(&pool).await?;

    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), String> {
    println!("[DB] run_migrations started");
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _migrations (id TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create migration table: {}", e))?;
    println!("[DB] _migrations table ensured");

    let migrations: &[(&str, &str)] = &[
        ("001", include_str!("../../migrations/001_init.sql")),
        (
            "002",
            include_str!("../../migrations/002_pattern_engine.sql"),
        ),
        ("003", include_str!("../../migrations/003_enterprise.sql")),
        (
            "004",
            include_str!("../../migrations/004_add_project_color.sql"),
        ),
        (
            "005",
            include_str!("../../migrations/005_conversation_pin_lock.sql"),
        ),
        (
            "006",
            include_str!("../../migrations/006_repository_timeline.sql"),
        ),
        (
            "007",
            include_str!("../../migrations/007_fix_local_doc_path.sql"),
        ),
        ("008", include_str!("../../migrations/008_decisions.sql")),
        (
            "009",
            include_str!("../../migrations/009_chunk_relations.sql"),
        ),
        (
            "010",
            include_str!("../../migrations/010_space_knowledge_guide.sql"),
        ),
        ("011", include_str!("../../migrations/011_source_tags.sql")),
        (
            "012",
            include_str!("../../migrations/012_deep_synthesis.sql"),
        ),
        (
            "013",
            include_str!("../../migrations/013_compiled_knowledge.sql"),
        ),
        (
            "016",
            include_str!("../../migrations/016_source_group_ingestion.sql"),
        ),
    ];

    for (id, sql) in migrations {
        let already: bool =
            sqlx::query_scalar::<_, i32>("SELECT COUNT(*) FROM _migrations WHERE id = ?")
                .bind(id)
                .fetch_one(pool)
                .await
                .unwrap_or(0)
                > 0;

        if already {
            println!("[DB] Migration {} already applied, skipping", id);
            continue;
        }

        println!("[DB] Applying migration {}...", id);
        for statement in sql.split(';') {
            let trimmed = statement.trim();
            if trimmed.is_empty() {
                continue;
            }
            let non_comment: String = trimmed
                .lines()
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
                        println!("[DB] Migration {} has idempotent statement, skipped", id);
                        continue;
                    }
                    let preview_len = trimmed.floor_char_boundary(trimmed.len().min(80));
                    let sql_preview = &trimmed[..preview_len];
                    return Err(format!(
                        "Migration {} failed: {} | SQL: {}",
                        id,
                        e,
                        sql_preview
                    ));
                }
            }
        }

        sqlx::query("INSERT INTO _migrations (id, applied_at) VALUES (?, ?)")
            .bind(id)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|e| format!("Migration {} record insert failed: {}", id, e))?;

        println!("[DB] Migration {} done", id);
    }

    Ok(())
}


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

pub async fn health_check(app_data_dir: &Path) -> HealthCheckResult {
    let db_state = read_db_state(app_data_dir);
    if db_state.last_operation != "idle" {
        return HealthCheckResult::NeedsRepair(RepairReason::LastOperationNotIdle(
            db_state.last_operation,
        ));
    }

    let kb_path_str =
        read_bootstrap(app_data_dir).unwrap_or_else(|| app_data_dir.to_string_lossy().to_string());
    let kb_path = PathBuf::from(&kb_path_str);

    let effective_kb_path = if std::fs::create_dir_all(&kb_path).is_ok() && kb_path.exists() {
        kb_path
    } else {
        eprintln!(
            "[DB-HEALTH] kb_path invalid, fallback to app_data_dir: {:?}",
            app_data_dir
        );
        app_data_dir.to_path_buf()
    };

    let db_key_hex: Option<String> = keyring::Entry::new("insightcap", "auto_login_key")
        .ok()
        .and_then(|e| e.get_password().ok());

    match init_db(&effective_kb_path, db_key_hex.as_deref()).await {
        Ok(pool) => {
            let integrity_ok = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
                .fetch_one(&pool)
                .await
                .map(|r| r == "ok")
                .unwrap_or(false);

            if !integrity_ok {
                pool.close().await;
                return HealthCheckResult::NeedsRepair(RepairReason::IntegrityCheckFailed);
            }

            let _ = write_db_state(
                app_data_dir,
                &DbState {
                    last_successful_open: Some(chrono::Utc::now().to_rfc3339()),
                    ..db_state
                },
            );

            HealthCheckResult::Ok(pool, effective_kb_path)
        }
        Err(e) => HealthCheckResult::NeedsRepair(RepairReason::DbOpenFailed(e)),
    }
}
