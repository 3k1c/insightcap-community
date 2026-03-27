use chrono::Utc;
use regex::Regex;
use reqwest::Client;
use sqlx::SqlitePool;
use std::io::Cursor;
use std::net::{IpAddr, ToSocketAddrs};
use std::time::Duration;
use url::Url;
use uuid::Uuid;

fn is_url_safe(url: &Url) -> Result<(), String> {
    // 1. Check scheme
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!("不支援的協議：{}。僅支援 http 及 https。", scheme));
    }

    // 2. Check host
    if let Some(host) = url.host_str() {
        // Reject localhost string
        if host.to_lowercase() == "localhost" {
            return Err("禁止訪問本機 (localhost)".to_string());
        }

        // Try to parse as IP
        if let Ok(ip) = host.parse::<IpAddr>() {
            if is_ip_private_or_loopback(ip) {
                return Err("禁止訪問私有網域或回環地址".to_string());
            }
        } else {
            // It's a hostname, try to resolve to IPs (blocking but fine for this background task)
            // Note: In a production app, we should use a non-blocking resolver or do this in a thread.
            // For now, we do a simple resolution check.
            if let Ok(mut addrs) = (host, 80).to_socket_addrs() {
                if addrs.any(|addr| is_ip_private_or_loopback(addr.ip())) {
                    return Err("主機指向禁止訪問的私有地理位置".to_string());
                }
            }
        }
    } else {
        return Err("無效的主機名稱".to_string());
    }

    Ok(())
}

fn is_ip_private_or_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unspecified() || (v6.segments()[0] & 0xfe00 == 0xfc00)
            // Unique Local Address
        }
    }
}

/// 從 HTML 中提取 <img> 標籤的 src，並轉為 Markdown 清單
fn extract_images_as_markdown(html: &str, base_url: &Url) -> String {
    let re = Regex::new(r#"(?i)<img[^>]+src\s*=\s*["']([^"']+)["']"#).unwrap();
    let mut images = Vec::new();

    for cap in re.captures_iter(html) {
        if let Some(src) = cap.get(1) {
            let src_val = src.as_str();
            // 排除 base64 圖片，因為存入 DB 太大且通常是圖示
            if src_val.starts_with("data:image") {
                continue;
            }

            // 處理相對路徑轉絕對路徑
            if let Ok(abs_url) = base_url.join(src_val) {
                let url_str = abs_url.to_string();
                if !images.contains(&url_str) {
                    images.push(url_str);
                }
            }
        }
    }

    if images.is_empty() {
        return String::new();
    }

    let mut md = String::from("\n\n---\n### 📄 網頁原始圖片清單\n\n");
    for img_url in images {
        // 使用 Markdown 格式：[![](圖片URL)](圖片URL) 達到預覽 + 點擊跳轉的效果
        md.push_str(&format!("[![image]({0})]({0})\n\n", img_url));
    }
    md
}

pub async fn scrape_and_ingest_url(
    pool: &SqlitePool,
    url_str: &str,
    _conversation_id: Option<String>,
) -> Result<(), String> {
    let parsed_url = Url::parse(url_str).map_err(|e| format!("Invalid URL: {}", e))?;

    // Validate URL safety (SSRF Protection S-202)
    is_url_safe(&parsed_url)?;

    // 1. Deduplication Check: Check if this URL was captured very recently (within 1 min)
    // to prevent double-capture from hotkey + manual input or triple/double triggers.
    let now_minus_1m = (Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM sources WHERE url = ? AND captured_at > ? LIMIT 1")
            .bind(url_str)
            .bind(&now_minus_1m)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    if existing.is_some() {
        println!(
            "[SCRAPER] URL {} was recently captured. Skipping duplicate.",
            url_str
        );
        return Ok(());
    }

    // Branch video processing
    let host = parsed_url.host_str().unwrap_or("");
    let mut title = String::new();
    let mut clean_content = String::new();
    let mut html = String::new();

    if host.contains("youtube.com") || host.contains("youtu.be") || host.contains("bilibili.com") {
        title = "自動擷取影片".to_string();
        clean_content = "影片網址處理功能轉移至背景，稍後自動產生文字稿。".to_string();
        html = format!("<h1>{}</h1><p>{}</p>", title, clean_content);
    }

    if clean_content.is_empty() {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

        html = client
            .get(parsed_url.clone())
            .send()
            .await
            .map_err(|e| format!("Failed to fetch URL: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Failed to read HTML: {}", e))?;

        let mut cursor = Cursor::new(html.clone());
        let product = readability::extractor::extract(&mut cursor, &parsed_url)
            .map_err(|e| format!("Readability extraction failed: {:?}", e))?;

        title = product.title;
        clean_content = product.text;

        // 【優化】只從清洗後的 content（文章主體）提取圖片，自動過濾無關圖示
        let image_md = extract_images_as_markdown(&product.content, &parsed_url);
        clean_content.push_str(&image_md);
    }

    // Generate Hash (using a simple DJB2 format or similar, or just save without hash if we don't have sha2 in crate)
    // Actually we can just skip hashing for MVP or use simple string length + title as a pseudo hash
    let content_hash = format!("{}_{}", clean_content.len(), title.len());

    let source_id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    // 1. Save Full Text to `sources`
    sqlx::query(
        "INSERT INTO sources (id, type, url, file_path, title, clean_content, raw_html, content_hash, captured_at, updated_at) VALUES (?, 'url', ?, '', ?, ?, ?, ?, ?, ?)"
    )
    .bind(&source_id)
    .bind(url_str)
    .bind(&title)
    .bind(&clean_content)
    .bind(&html)
    .bind(&content_hash)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to save source: {}", e))?;

    // 2. Queue into `inbox` for processing
    let inbox_id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO inbox (id, content, content_type, source_url, window_title, session_id, status, captured_at) VALUES (?, ?, 'url', ?, ?, ?, 'pending', ?)"
    )
    .bind(&inbox_id)
    .bind(&clean_content)
    .bind(url_str)
    .bind(&title)
    .bind(&source_id) // Using source_id as session_id to link
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to queue inbox chunk: {}", e))?;

    Ok(())
}

pub async fn scrape_url(
    url_str: &str,
) -> Result<crate::capture::file_parser::ParsedDocument, String> {
    let parsed_url = Url::parse(url_str).map_err(|e| format!("Invalid URL: {}", e))?;

    // Validate URL safety (SSRF Protection S-202)
    is_url_safe(&parsed_url)?;

    // Branch video processing
    let host = parsed_url.host_str().unwrap_or("");
    if host.contains("youtube.com") || host.contains("youtu.be") || host.contains("bilibili.com") {
        return Ok(crate::capture::file_parser::ParsedDocument {
            title: "影片網址".to_string(),
            chunks: vec![crate::capture::file_parser::FileChunk {
                content: "影片網址處理功能轉移至背景，稍後自動產生文字稿。".to_string(),
                chunk_type: "document".to_string(),
                image_path: None,
                status: "processed".to_string(),
            }],
        });
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let html = client
        .get(parsed_url.clone())
        .send()
        .await
        .map_err(|e| format!("Failed to fetch URL: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read HTML: {}", e))?;

    let mut cursor = Cursor::new(html);
    let product = readability::extractor::extract(&mut cursor, &parsed_url)
        .map_err(|e| format!("Readability extraction failed: {:?}", e))?;

    let mut final_content = product.text;

    // 同樣只從清洗後的 content 提取圖片
    let image_md = extract_images_as_markdown(&product.content, &parsed_url);
    final_content.push_str(&image_md);

    Ok(crate::capture::file_parser::ParsedDocument {
        title: product.title,
        chunks: vec![crate::capture::file_parser::FileChunk {
            content: final_content,
            chunk_type: "document".to_string(),
            image_path: None,
            status: "processed".to_string(),
        }],
    })
}
