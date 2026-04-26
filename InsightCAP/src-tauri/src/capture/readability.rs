use chrono::Utc;
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::io::Cursor;
use std::net::{IpAddr, ToSocketAddrs};
use std::time::Duration;
use url::Url;
use uuid::Uuid;

fn is_url_safe(url: &Url) -> Result<(), String> {
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!(
            "Unsupported URL scheme '{}'. Only http/https are allowed.",
            scheme
        ));
    }

    if let Some(host) = url.host_str() {
        if host.to_lowercase() == "localhost" {
            return Err("Localhost is not allowed for URL capture.".to_string());
        }

        if let Ok(ip) = host.parse::<IpAddr>() {
            if is_ip_private_or_loopback(ip) {
                return Err("Private or loopback IP addresses are not allowed.".to_string());
            }
        } else {
            if let Ok(mut addrs) = (host, 80).to_socket_addrs() {
                if addrs.any(|addr| is_ip_private_or_loopback(addr.ip())) {
                    return Err(
                        "Resolved host points to a private address and is blocked.".to_string()
                    );
                }
            }
        }
    } else {
        return Err("URL host is missing.".to_string());
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
        }
    }
}

fn extract_images_as_markdown(html: &str, base_url: &Url) -> String {
    let re = Regex::new(r#"(?i)<img[^>]+src\s*=\s*["']([^"']+)["']"#).unwrap();
    let mut images = Vec::new();

    for cap in re.captures_iter(html) {
        if let Some(src) = cap.get(1) {
            let src_val = src.as_str();
            if src_val.starts_with("data:image") {
                continue;
            }

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

    let mut md = String::from("\n\n---\n### Images in Source\n\n");
    for img_url in images {
        md.push_str(&format!("[![image]({0})]({0})\n\n", img_url));
    }
    md
}

fn extract_title_from_html(html: &str) -> String {
    let re_title = Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap();
    if let Some(cap) = re_title.captures(html) {
        let t = html_to_text(cap.get(1).map(|m| m.as_str()).unwrap_or(""));
        if !t.is_empty() {
            return t;
        }
    }
    String::new()
}

fn html_to_text(input: &str) -> String {
    let mut s = input.to_string();
    let re_script = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    s = re_script.replace_all(&s, " ").to_string();
    let re_style = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    s = re_style.replace_all(&s, " ").to_string();
    let re_tags = Regex::new(r"(?is)<[^>]+>").unwrap();
    s = re_tags.replace_all(&s, " ").to_string();

    s = s
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    let re_space = Regex::new(r"[ \t\r\n]+").unwrap();
    re_space.replace_all(&s, " ").trim().to_string()
}

fn looks_like_noise(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 24 {
        return true;
    }
    if t.starts_with("http://") || t.starts_with("https://") {
        return true;
    }
    false
}

fn collect_text_from_json_value(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            let t = s.trim();
            if !looks_like_noise(t) {
                out.push(t.to_string());
            }
        }
        Value::Array(arr) => {
            for it in arr {
                collect_text_from_json_value(it, out);
            }
        }
        Value::Object(map) => {
            for (_, it) in map {
                collect_text_from_json_value(it, out);
            }
        }
        _ => {}
    }
}

fn extract_json_script_text(html: &str) -> Vec<String> {
    let mut result = Vec::new();

    let re_next =
        Regex::new(r#"(?is)<script[^>]*id=["']__NEXT_DATA__["'][^>]*>(.*?)</script>"#).unwrap();
    for cap in re_next.captures_iter(html) {
        if let Some(raw) = cap.get(1).map(|m| m.as_str()) {
            if let Ok(v) = serde_json::from_str::<Value>(raw) {
                collect_text_from_json_value(&v, &mut result);
            }
        }
    }

    let re_ld =
        Regex::new(r#"(?is)<script[^>]*type=["']application/ld\+json["'][^>]*>(.*?)</script>"#)
            .unwrap();
    for cap in re_ld.captures_iter(html) {
        if let Some(raw) = cap.get(1).map(|m| m.as_str()) {
            if let Ok(v) = serde_json::from_str::<Value>(raw) {
                collect_text_from_json_value(&v, &mut result);
            }
        }
    }

    result
}

fn extract_main_article_text(html: &str) -> String {
    let re_main = Regex::new(r"(?is)<(main|article)[^>]*>(.*?)</(main|article)>").unwrap();
    let mut out = String::new();
    for cap in re_main.captures_iter(html) {
        let raw = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let text = html_to_text(raw);
        if text.len() > 80 {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&text);
        }
    }
    out
}

fn dedup_and_join_texts(parts: Vec<String>, max_len: usize) -> String {
    let mut seen = HashSet::new();
    let mut out = String::new();
    for p in parts {
        let norm = p.to_lowercase();
        if seen.contains(&norm) {
            continue;
        }
        seen.insert(norm);
        if out.len() + p.len() + 2 > max_len {
            break;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&p);
    }
    out
}

fn recover_dynamic_text(html: &str) -> String {
    let mut parts = extract_json_script_text(html);
    let main_text = extract_main_article_text(html);
    if !main_text.is_empty() {
        parts.push(main_text);
    }
    dedup_and_join_texts(parts, 60_000)
}

fn should_merge_recovered_text(current: &str, recovered: &str) -> bool {
    if recovered.trim().is_empty() {
        return false;
    }
    let cur_len = current.trim().len();
    let rec_len = recovered.trim().len();
    if cur_len < 500 && rec_len > 300 {
        return true;
    }
    rec_len > (cur_len.saturating_mul(3) / 2) && rec_len > 500
}

async fn fetch_reader_mirror_text(url_str: &str) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("Failed to build reader fallback client: {}", e))?;

    let reader_url = format!("https://r.jina.ai/{}", url_str);
    let text = client
        .get(&reader_url)
        .send()
        .await
        .map_err(|e| format!("Reader fallback fetch failed: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Reader fallback read failed: {}", e))?;

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Reader fallback returned empty content".to_string());
    }
    Ok(trimmed.to_string())
}

pub async fn scrape_and_ingest_url(
    pool: &SqlitePool,
    url_str: &str,
    _conversation_id: Option<String>,
) -> Result<(), String> {
    let parsed_url = Url::parse(url_str).map_err(|e| format!("Invalid URL: {}", e))?;

    is_url_safe(&parsed_url)?;

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

    let host = parsed_url.host_str().unwrap_or("");
    let mut title = String::new();
    let mut clean_content = String::new();
    let mut html = String::new();

    if host.contains("youtube.com") || host.contains("youtu.be") || host.contains("bilibili.com") {
        title = "Video Link".to_string();
        clean_content = "Video content is handled by the dedicated video parser.".to_string();
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

        title = if product.title.trim().is_empty() {
            extract_title_from_html(&html)
        } else {
            product.title
        };
        clean_content = product.text;

        let recovered = recover_dynamic_text(&html);
        if should_merge_recovered_text(&clean_content, &recovered) {
            if !clean_content.trim().is_empty() {
                clean_content.push_str("\n\n---\n### Recovered Dynamic Content\n\n");
            }
            clean_content.push_str(&recovered);
        }

        if clean_content.trim().len() < 1200 {
            if let Ok(reader_text) = fetch_reader_mirror_text(url_str).await {
                if reader_text.len() > clean_content.trim().len().saturating_add(300) {
                    if !clean_content.trim().is_empty() {
                        clean_content.push_str("\n\n---\n### Reader Fallback Content\n\n");
                    }
                    clean_content.push_str(&reader_text);
                }
            }
        }

        let image_md = extract_images_as_markdown(&product.content, &parsed_url);
        clean_content.push_str(&image_md);
    }

    let content_hash = format!("{}_{}", clean_content.len(), title.len());

    let source_id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

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

    is_url_safe(&parsed_url)?;

    let host = parsed_url.host_str().unwrap_or("");
    if host.contains("youtube.com") || host.contains("youtu.be") || host.contains("bilibili.com") {
        return Ok(crate::capture::file_parser::ParsedDocument {
            title: "Video Link".to_string(),
            chunks: vec![crate::capture::file_parser::FileChunk {
                content: "Video content is handled by the dedicated video parser.".to_string(),
                chunk_type: "document".to_string(),
                source_type: "web_url".to_string(),
                metadata: serde_json::json!({
                    "source_type": "web_url",
                    "url": url_str,
                }),
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

    let mut cursor = Cursor::new(html.clone());
    let product = readability::extractor::extract(&mut cursor, &parsed_url)
        .map_err(|e| format!("Readability extraction failed: {:?}", e))?;

    let mut final_content = product.text;
    let recovered = recover_dynamic_text(&html);
    if should_merge_recovered_text(&final_content, &recovered) {
        if !final_content.trim().is_empty() {
            final_content.push_str("\n\n---\n### Recovered Dynamic Content\n\n");
        }
        final_content.push_str(&recovered);
    }

    if final_content.trim().len() < 1200 {
        if let Ok(reader_text) = fetch_reader_mirror_text(url_str).await {
            if reader_text.len() > final_content.trim().len().saturating_add(300) {
                if !final_content.trim().is_empty() {
                    final_content.push_str("\n\n---\n### Reader Fallback Content\n\n");
                }
                final_content.push_str(&reader_text);
            }
        }
    }

    let image_md = extract_images_as_markdown(&product.content, &parsed_url);
    final_content.push_str(&image_md);

    Ok(crate::capture::file_parser::ParsedDocument {
        title: if product.title.trim().is_empty() {
            extract_title_from_html(&html)
        } else {
            product.title
        },
        chunks: vec![crate::capture::file_parser::FileChunk {
            content: final_content,
            chunk_type: "document".to_string(),
            source_type: "html".to_string(),
            metadata: serde_json::json!({
                "source_type": "html",
                "url": url_str,
                "domain": host,
            }),
            image_path: None,
            status: "processed".to_string(),
        }],
    })
}
