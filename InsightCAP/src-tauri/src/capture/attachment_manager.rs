use crate::error::AppError;
use chrono::Datelike;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

pub async fn copy_to_attachments(
    kb_path: &str,
    source_path: &Path,
) -> Result<(PathBuf, String), AppError> {
    let bytes = fs::read(source_path)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;
    let hash = format!("{:x}", Sha256::digest(&bytes));

    let now = chrono::Utc::now();
    let month_dir = PathBuf::from(format!(
        "{}/attachments/{:04}-{:02}",
        kb_path,
        now.year(),
        now.month()
    ));
    fs::create_dir_all(&month_dir)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;

    let original_name = source_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let stem = source_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let ext = source_path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let simple_path = month_dir.join(&original_name);
    if !simple_path.exists() {
        fs::copy(source_path, &simple_path)
            .await
            .map_err(|e| AppError::Capture(e.to_string()))?;
        return Ok((simple_path, hash));
    }

    let existing_bytes = fs::read(&simple_path)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;
    let existing_hash = format!("{:x}", Sha256::digest(&existing_bytes));
    if existing_hash == hash {
        return Ok((simple_path, hash));
    }

    let hashed_name = if ext.is_empty() {
        format!("{}_{}", stem, &hash[..6])
    } else {
        format!("{}_{}.{}", stem, &hash[..6], ext)
    };

    let hashed_path = month_dir.join(hashed_name);
    if !hashed_path.exists() {
        fs::copy(source_path, &hashed_path)
            .await
            .map_err(|e| AppError::Capture(e.to_string()))?;
    }

    Ok((hashed_path, hash))
}

pub async fn copy_image_to_attachments(
    kb_path: &str,
    image_bytes: &[u8],
    prefix: &str,
    ext: &str,
) -> Result<PathBuf, AppError> {
    let images_dir = PathBuf::from(format!("{}/attachments/images", kb_path));
    fs::create_dir_all(&images_dir)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;

    let uuid = Uuid::now_v7().to_string();
    let filename = format!("{}_{}.{}", prefix, uuid, ext);
    let dest_path = images_dir.join(filename);

    fs::write(&dest_path, image_bytes)
        .await
        .map_err(|e| AppError::Capture(e.to_string()))?;

    Ok(dest_path)
}

pub async fn delete_if_orphaned(db: &sqlx::SqlitePool, source_id: &str) -> Result<(), AppError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chunks WHERE source_id = ?")
        .bind(source_id)
        .fetch_one(db)
        .await?;

    if count > 0 {
        return Ok(());
    }

    let file_path: Option<String> =
        sqlx::query_scalar("SELECT file_path FROM sources WHERE id = ?")
            .bind(source_id)
            .fetch_optional(db)
            .await?;

    if let Some(path) = file_path {
        let p = Path::new(&path);
        if p.exists() {
            let _ = fs::remove_file(p).await;
        }
    }

    sqlx::query("DELETE FROM sources WHERE id = ?")
        .bind(source_id)
        .execute(db)
        .await?;

    Ok(())
}

pub async fn scan_orphaned_attachments(
    db: &sqlx::SqlitePool,
    kb_path: &str,
) -> Result<Vec<PathBuf>, AppError> {
    let attachments_dir = PathBuf::from(format!("{}/attachments", kb_path));
    let mut orphaned = Vec::new();

    let rows: Vec<(Option<String>,)> =
        sqlx::query_as("SELECT file_path FROM sources WHERE file_path IS NOT NULL")
            .fetch_all(db)
            .await?;

    let known_paths: Vec<String> = rows.into_iter().filter_map(|r| r.0).collect();

    scan_dir(&attachments_dir, &known_paths, &mut orphaned).await?;
    Ok(orphaned)
}

fn scan_dir<'a>(
    dir: &'a Path,
    known_paths: &'a [String],
    orphaned: &'a mut Vec<PathBuf>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), AppError>> + Send + 'a>> {
    Box::pin(async move {
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
                scan_dir(&path, known_paths, orphaned).await?;
            } else {
                let path_str = path.to_string_lossy().to_string();
                if !known_paths.contains(&path_str) {
                    orphaned.push(path);
                }
            }
        }

        Ok(())
    })
}
