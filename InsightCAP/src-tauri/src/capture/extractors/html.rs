use crate::error::AppError;
use std::io::Cursor;
use url::Url;

pub struct HtmlChunk {
    pub title: String,
    pub clean_content: String,
}

pub async fn extract_html(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
) -> Result<Vec<HtmlChunk>, AppError> {
    let content = crate::capture::encoding::read_text_file(file_path)
        .map_err(|e| AppError::Capture(format!("Failed to read HTML file: {}", e)))?;

    let dummy_url = Url::parse("file://localhost/").unwrap();
    let mut cursor = Cursor::new(content);

    let product = readability::extractor::extract(&mut cursor, &dummy_url)
        .map_err(|e| AppError::Capture(format!("Readability extraction failed: {:?}", e)))?;

    Ok(vec![HtmlChunk {
        title: product.title,
        clean_content: product.text,
    }])
}
