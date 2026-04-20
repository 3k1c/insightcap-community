//! # HTML 提取器
//!
//! 複用 Readability 邏輯，處理本地 HTML 文件。
//! 見 Phase3.1.md P3.1-10。

use crate::error::AppError;
use std::io::Cursor;
use url::Url;

pub struct HtmlChunk {
    pub title: String,
    pub clean_content: String,
}

/// 提取 HTML 內容
pub async fn extract_html(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
) -> Result<Vec<HtmlChunk>, AppError> {
    let content = crate::capture::encoding::read_text_file(file_path)
        .map_err(|e| AppError::Capture(format!("HTML 讀取失敗: {}", e)))?;

    // 使用 readability 提取正文
    // 建立一個假的 URL 作為基礎路徑（Readability 提取連結時會用到）
    let dummy_url = Url::parse("file://localhost/").unwrap();
    let mut cursor = Cursor::new(content);

    let product = readability::extractor::extract(&mut cursor, &dummy_url)
        .map_err(|e| AppError::Capture(format!("Readability 提取失敗: {:?}", e)))?;

    Ok(vec![HtmlChunk {
        title: product.title,
        clean_content: product.text,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_extract_html() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("test.html");
        let mut file = File::create(&path).unwrap();

        let html_content = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Test Page Title</title>
                <style>body { color: red; }</style>
                <script>console.log("noisy script!");</script>
            </head>
            <body>
                <nav>
                    <ul><li>Menu Item 1</li></ul>
                </nav>
                <article>
                    <h1>The True Concept</h1>
                    <p>This is the main readable content.</p>
                </article>
                <footer>Copyright 2026</footer>
            </body>
            </html>
        "#;

        file.write_all(html_content.as_bytes()).unwrap();

        let chunks = extract_html("kb_path", path.to_str().unwrap(), "test")
            .await
            .unwrap();

        // Should only be one chunk since it extracts the page into one document
        assert_eq!(chunks.len(), 1);

        let chunk = &chunks[0];
        assert_eq!(chunk.title, "Test Page Title");

        // Assert clean content retains the real text
        assert!(chunk
            .clean_content
            .contains("This is the main readable content."));

        // Assert it strips out tags/noise
        assert!(!chunk.clean_content.contains("noisy script"));
        assert!(!chunk.clean_content.contains("body { color: red; }"));
        // Readability might also strip out nav menus depending on the layout, but the focus is stripping styles and scripts.
    }
}
