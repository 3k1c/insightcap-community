//! # 附件管理模組
//!
//! 負責文件和圖片的複製、去重、刪除邏輯。
//! 所有加入知識庫的文件統一複製到 attachments/ 目錄。
//! 見 ADR-024。

use crate::error::AppError;
use chrono::Datelike;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

/// 將文件複製到 attachments/{YYYY-MM}/ 目錄
/// 返回複製後的路徑和 SHA-256 hash
pub async fn copy_to_attachments(
    kb_path: &str,
    source_path: &Path,
) -> Result<(PathBuf, String), AppError> {
    // 計算 SHA-256
    let bytes = fs::read(source_path)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;
    let hash = format!("{:x}", Sha256::digest(&bytes));

    // 按年月建立目錄
    let now = chrono::Utc::now();
    let month_dir = format!(
        "{}/attachments/{:04}-{:02}",
        kb_path,
        now.year(),
        now.month()
    );
    fs::create_dir_all(&month_dir)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;

    // 處理檔名衝突
    let original_name = source_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let ext = source_path
        .extension()
        .unwrap_or_default()
        .to_string_lossy();
    let stem = source_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();

    let dest_name = {
        let simple = format!("{}/{}", month_dir, original_name);
        if !Path::new(&simple).exists() {
            original_name.to_string()
        } else {
            // Check if existing file has the exact same content
            let existing_bytes = fs::read(&simple)
                .await
                .map_err(|e| AppError::Capture(e.to_string()))?;
            let existing_hash = format!("{:x}", Sha256::digest(&existing_bytes));

            if existing_hash == hash {
                return Ok((PathBuf::from(simple), hash));
            }

            // 附加 hash 前 6 碼避免衝突
            let hashed_name = if ext.is_empty() {
                format!("{}_{}", stem, &hash[..6])
            } else {
                format!("{}_{}.{}", stem, &hash[..6], ext)
            };

            let hashed_path = format!("{}/{}", month_dir, hashed_name);
            if Path::new(&hashed_path).exists() {
                // Already exists with hash
                return Ok((PathBuf::from(hashed_path), hash));
            }

            hashed_name
        }
    };

    let dest_path = PathBuf::from(format!("{}/{}", month_dir, dest_name));

    // 複製文件
    fs::copy(source_path, &dest_path)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;

    Ok((dest_path, hash))
}

/// 將圖片複製到 attachments/images/ 目錄
/// prefix: "capture" | "screenshot" | 文件名稱
pub async fn copy_image_to_attachments(
    kb_path: &str,
    image_bytes: &[u8],
    prefix: &str,
    ext: &str,
) -> Result<PathBuf, AppError> {
    // 建立 images 目錄
    let images_dir = format!("{}/attachments/images", kb_path);
    fs::create_dir_all(&images_dir)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;

    // UUID v7 命名
    let uuid = Uuid::now_v7().to_string();
    let filename = format!("{}_{}.{}", prefix, uuid, ext);
    let dest_path = PathBuf::from(format!("{}/{}", images_dir, filename));

    // 寫入圖片
    fs::write(&dest_path, image_bytes)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;

    Ok(dest_path)
}

/// 刪除附件（若無其他 Chunk 引用）
/// 在刪除 Chunk 後呼叫
pub async fn delete_if_orphaned(db: &sqlx::SqlitePool, source_id: &str) -> Result<(), AppError> {
    // 檢查剩餘引用數量
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chunks WHERE source_id = ?")
        .bind(source_id)
        .fetch_one(db)
        .await?;

    if count > 0 {
        // 還有其他 Chunk 引用，保留附件
        return Ok(());
    }

    // 取得文件路徑
    let file_path: Option<String> =
        sqlx::query_scalar("SELECT file_path FROM sources WHERE id = ?")
            .bind(source_id)
            .fetch_one(db)
            .await?;

    // 刪除實體文件
    if let Some(path) = file_path {
        let p = Path::new(&path);
        if p.exists() {
            fs::remove_file(p)
                .await
                .map_err(|e| AppError::Capture(e.to_string()))?;
        }
    }

    // 刪除 source 記錄
    sqlx::query("DELETE FROM sources WHERE id = ?")
        .bind(source_id)
        .execute(db)
        .await?;

    Ok(())
}

/// 掃描孤立附件（有文件但無對應 source 記錄）
pub async fn scan_orphaned_attachments(
    kb_path: &str,
    db: &sqlx::SqlitePool,
) -> Result<Vec<PathBuf>, AppError> {
    let attachments_dir = format!("{}/attachments", kb_path);
    let mut orphaned = Vec::new();

    // 取得所有 source 的 file_path
    let rows: Vec<(Option<String>,)> =
        sqlx::query_as("SELECT file_path FROM sources WHERE file_path IS NOT NULL")
            .fetch_all(db)
            .await?;

    let known_paths: Vec<String> = rows.into_iter().filter_map(|r| r.0).collect();

    // 遞歸掃描目錄
    scan_dir(Path::new(&attachments_dir), &known_paths, &mut orphaned).await?;

    Ok(orphaned)
}

/// 遞歸掃描目錄，找出孤立文件
async fn scan_dir(
    dir: &Path,
    known_paths: &[String],
    orphaned: &mut Vec<PathBuf>,
) -> Result<(), AppError> {
    if !dir.exists() || !dir.is_dir() {
        return Ok(());
    }

    let mut entries = fs::read_dir(dir)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?
    {
        let path = entry.path();

        if path.is_dir() {
            // 遞歸掃描子目錄
            Box::pin(scan_dir(&path, known_paths, orphaned)).await?;
        } else {
            let path_str = path.to_string_lossy().to_string();
            // TODO: 這裡的路徑比對可能需要考慮相對/絕對路徑的一致性
            if !known_paths.contains(&path_str) {
                orphaned.push(path);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_copy_to_attachments_dedup() {
        let dir = tempdir().unwrap();
        let kb_path = dir.path().to_str().unwrap().to_string();

        let source_dir = tempdir().unwrap();
        let source_path = source_dir.path().join("test.txt");
        let mut f = std::fs::File::create(&source_path).unwrap();
        f.write_all(b"hello world").unwrap();

        // First copy
        let (p1, h1) = copy_to_attachments(&kb_path, &source_path).await.unwrap();
        assert!(p1.exists());

        // Second copy of same file (should deduplicate, no new file created)
        let (p2, h2) = copy_to_attachments(&kb_path, &source_path).await.unwrap();
        assert_eq!(p1, p2);
        assert_eq!(h1, h2);

        // Third copy with DIFFERENT content but SAME original filename
        let source_path2 = source_dir.path().join("test.txt");
        let mut f2 = std::fs::File::create(&source_path2).unwrap();
        f2.write_all(b"different content").unwrap();

        let (p3, h3) = copy_to_attachments(&kb_path, &source_path2).await.unwrap();
        assert_ne!(p1, p3); // Path should be different (hash appended)
        assert_ne!(h1, h3);
        assert!(p3.exists());

        let file_name = p3.file_name().unwrap().to_string_lossy();
        assert!(file_name.contains(&h3[..6]));
    }

    #[tokio::test]
    async fn test_copy_image_to_attachments() {
        let dir = tempdir().unwrap();
        let kb_path = dir.path().to_str().unwrap().to_string();

        let p1 = copy_image_to_attachments(&kb_path, b"fake image", "capture", "jpg")
            .await
            .unwrap();
        assert!(p1.exists());
        let name = p1.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("capture_"));
        assert!(name.ends_with(".jpg"));

        let p2 = copy_image_to_attachments(&kb_path, b"fake image 2", "screenshot", "png")
            .await
            .unwrap();
        assert!(p2.exists());
        let name2 = p2.file_name().unwrap().to_string_lossy();
        assert!(name2.starts_with("screenshot_"));
        assert!(name2.ends_with(".png"));
    }

    #[tokio::test]
    async fn test_delete_if_orphaned() {
        let dir = tempdir().unwrap();

        // Setup SQLite for testing
        let db_path = dir.path().join("test.db");
        let db =
            sqlx::SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path.to_string_lossy()))
                .await
                .unwrap();

        sqlx::query("CREATE TABLE sources (id TEXT PRIMARY KEY, file_path TEXT)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE chunks (id TEXT PRIMARY KEY, source_id TEXT)")
            .execute(&db)
            .await
            .unwrap();

        // Create a fake file
        let file_path = dir.path().join("fake_file.txt");
        std::fs::File::create(&file_path).unwrap();

        // Insert source
        sqlx::query("INSERT INTO sources (id, file_path) VALUES (?, ?)")
            .bind("s1")
            .bind(file_path.to_string_lossy().to_string())
            .execute(&db)
            .await
            .unwrap();

        // Insert chunk
        sqlx::query("INSERT INTO chunks (id, source_id) VALUES (?, ?)")
            .bind("c1")
            .bind("s1")
            .execute(&db)
            .await
            .unwrap();

        // Delete if orphaned should KEEP file (since c1 exists)
        delete_if_orphaned(&db, "s1").await.unwrap();
        assert!(file_path.exists());

        // Remove chunk
        sqlx::query("DELETE FROM chunks WHERE id = ?")
            .bind("c1")
            .execute(&db)
            .await
            .unwrap();

        // Delete if orphaned should DELETE file & source
        delete_if_orphaned(&db, "s1").await.unwrap();
        assert!(!file_path.exists());

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sources WHERE id = 's1'")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
