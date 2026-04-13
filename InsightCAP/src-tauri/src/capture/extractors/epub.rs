//! # EPUB 提取器
//!
//! 實作 EPUB 電子書按章節提取。
//! 見 Phase3.1.md P3.1-12。

use crate::error::AppError;
use epub::doc::EpubDoc;
use std::io::Cursor;
use url::Url;

pub struct EpubChunk {
    pub title: String,
    pub clean_content: String,
}

/// 提取 EPUB 內容，按章節切分
pub async fn extract_epub(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
) -> Result<Vec<EpubChunk>, AppError> {
    let mut doc =
        EpubDoc::new(file_path).map_err(|e| AppError::Capture(format!("EPUB 載入失敗: {}", e)))?;

    let mut chunks = Vec::new();
    let dummy_url = Url::parse("file://localhost/").unwrap();

    // 取得章節總數
    let num_chapters = doc.spine.len();

    for i in 0..num_chapters {
        // 切換到指定章節
        doc.set_current_chapter(i);

        // 嘗試取得該章節標題 (從 TOC 中尋找目前資源對應的標題)
        let chapter_title = doc
            .get_current_str()
            .and_then(|(title, _)| Some(title))
            .unwrap_or_else(|| format!("Chapter {}", i + 1));

        if let Ok(content_bytes) = doc.get_current_with_epub_uris() {
            if let Ok(html_content) = String::from_utf8(content_bytes) {
                // 使用 readability 提取正文，過濾 HTML 標籤
                let mut cursor = Cursor::new(html_content);
                if let Ok(product) = readability::extractor::extract(&mut cursor, &dummy_url) {
                    if !product.text.trim().is_empty() {
                        chunks.push(EpubChunk {
                            title: chapter_title,
                            clean_content: product.text,
                        });
                    }
                }
            }
        }
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    // EPUB 測試需要真實檔案，暫時以編譯通過為主。
}
