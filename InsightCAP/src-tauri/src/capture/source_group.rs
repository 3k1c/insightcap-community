use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SourceIdentity {
    pub group_type: String,
    pub canonical_key: String,
    pub canonical_url: Option<String>,
    pub content_hash: String,
}

pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn identity_for_file(_file_path: &str, content: &str) -> SourceIdentity {
    let hash = content_hash(content);
    SourceIdentity {
        group_type: "file".to_string(),
        canonical_key: format!("file:{}", hash),
        canonical_url: None,
        content_hash: hash,
    }
}

pub fn identity_for_url(url: &str, content: &str) -> SourceIdentity {
    let hash = content_hash(content);
    let canonical = canonicalize_url(url);
    SourceIdentity {
        group_type: if canonical.contains("youtube.com/watch")
            || canonical.contains("bilibili.com/video/")
        {
            "video".to_string()
        } else {
            "url".to_string()
        },
        canonical_key: format!("url:{}", canonical),
        canonical_url: Some(canonical),
        content_hash: hash,
    }
}

pub fn identity_for_clipboard(title: &str, content: &str) -> SourceIdentity {
    let hash = content_hash(content);
    let clean_title = title.trim().to_lowercase();
    SourceIdentity {
        group_type: "clipboard".to_string(),
        canonical_key: format!("clipboard:{}:{}", clean_title, hash),
        canonical_url: None,
        content_hash: hash,
    }
}

pub async fn get_or_create_source_group(
    pool: &SqlitePool,
    identity: &SourceIdentity,
    title: &str,
) -> Result<String, String> {
    if let Some(id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM source_groups WHERE canonical_key = ? LIMIT 1",
    )
    .bind(&identity.canonical_key)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    {
        let now = chrono::Utc::now().to_rfc3339();
        let _ = sqlx::query(
            "UPDATE source_groups SET title = COALESCE(NULLIF(?, ''), title), content_hash = ?, updated_at = ? WHERE id = ?"
        )
        .bind(title)
        .bind(&identity.content_hash)
        .bind(&now)
        .bind(&id)
        .execute(pool)
        .await;
        return Ok(id);
    }

    let id = Uuid::now_v7().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO source_groups (id, group_type, canonical_key, canonical_url, title, content_hash, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&identity.group_type)
    .bind(&identity.canonical_key)
    .bind(&identity.canonical_url)
    .bind(title)
    .bind(&identity.content_hash)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(id)
}

fn canonicalize_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.contains("bilibili.com/video/") {
        if let Some(start) = trimmed.find("/video/") {
            let bvid = trimmed[start + 7..]
                .split('/')
                .next()
                .unwrap_or("")
                .split('?')
                .next()
                .unwrap_or("");
            if !bvid.is_empty() {
                return format!("https://www.bilibili.com/video/{}", bvid);
            }
        }
    }

    if trimmed.contains("youtu.be/") {
        if let Some(id) = trimmed.split("youtu.be/").nth(1) {
            let video_id = id
                .split('?')
                .next()
                .unwrap_or("")
                .split('/')
                .next()
                .unwrap_or("");
            if !video_id.is_empty() {
                return format!("https://www.youtube.com/watch?v={}", video_id);
            }
        }
    }

    if trimmed.contains("youtube.com/watch") {
        if let Ok(parsed) = Url::parse(trimmed) {
            if let Some((_, video_id)) = parsed.query_pairs().find(|(key, _)| key == "v") {
                return format!("https://www.youtube.com/watch?v={}", video_id);
            }
        }
    }

    match Url::parse(trimmed) {
        Ok(mut parsed) => {
            parsed.set_fragment(None);
            let kept_pairs: Vec<(String, String)> = parsed
                .query_pairs()
                .filter(|(k, _)| {
                    let key = k.as_ref();
                    !key.starts_with("utm_")
                        && key != "fbclid"
                        && key != "gclid"
                        && key != "ref"
                        && key != "source"
                })
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            parsed.set_query(None);
            if !kept_pairs.is_empty() {
                let query = kept_pairs
                    .into_iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&");
                parsed.set_query(Some(&query));
            }
            parsed.to_string().trim_end_matches('/').to_string()
        }
        Err(_) => trimmed.trim_end_matches('/').to_lowercase(),
    }
}
