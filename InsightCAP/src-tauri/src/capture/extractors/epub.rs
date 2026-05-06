use crate::error::AppError;
use epub::doc::EpubDoc;
use std::io::Cursor;
use url::Url;

pub struct EpubChunk {
    pub title: String,
    pub clean_content: String,
}

pub async fn extract_epub(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
) -> Result<Vec<EpubChunk>, AppError> {
    let mut doc = EpubDoc::new(file_path)
        .map_err(|e| AppError::Capture(format!("EPUB parsing failed: {}", e)))?;

    let mut chunks = Vec::new();
    let dummy_url = Url::parse("file://localhost/").unwrap();

    let num_chapters = doc.spine.len();

    for i in 0..num_chapters {
        doc.set_current_chapter(i);

        let chapter_title = doc
            .get_current_str()
            .and_then(|(title, _)| Some(title))
            .unwrap_or_else(|| format!("Chapter {}", i + 1));

        if let Ok(content_bytes) = doc.get_current_with_epub_uris() {
            if let Ok(html_content) = String::from_utf8(content_bytes) {
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
mod tests {}
