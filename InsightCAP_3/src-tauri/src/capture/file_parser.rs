use std::fs;
use std::path::Path;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileChunk {
    pub content: String,
    pub chunk_type: String, // "text" | "image" | "document" | "mixed"
    pub image_path: Option<String>,
    pub status: String, // "processed" | "pending_ocr"
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ParsedDocument {
    pub chunks: Vec<FileChunk>,
    pub title: String,
}

pub async fn parse_file(kb_path: &str, file_path: &str) -> Result<ParsedDocument, String> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let title = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown Document")
        .to_string();

    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");

    use crate::capture::extractors;

    let chunks = match ext.as_str() {
        // Plain Text types
        "txt" | "log" => {
            let content =
                fs::read_to_string(path).map_err(|e| format!("Failed to read text file: {}", e))?;
            vec![FileChunk {
                content,
                chunk_type: "text".to_string(),
                image_path: None,
                status: "processed".to_string(),
            }]
        }
        "md" => {
            let content = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read markdown file: {}", e))?;
            vec![FileChunk {
                content,
                chunk_type: "text".to_string(),
                image_path: None,
                status: "processed".to_string(),
            }]
        }
        // PDF
        "pdf" => {
            let p_chunks = extractors::pdf::extract_pdf(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            p_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: c.clean_content,
                    chunk_type: "document".to_string(),
                    image_path: c.image_path,
                    status: c.status,
                })
                .collect()
        }
        // DOCX
        "docx" => {
            let d_chunks = extractors::docx::extract_docx(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            d_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: c.clean_content,
                    chunk_type: "document".to_string(),
                    image_path: c.image_path,
                    status: "processed".to_string(),
                })
                .collect()
        }
        // XLSX
        "xlsx" => {
            let x_chunks = extractors::xlsx::extract_xlsx(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            x_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: c.clean_content,
                    chunk_type: "document".to_string(),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        // PPTX
        "pptx" => {
            let p_chunks = extractors::pptx::extract_pptx(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            p_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: c.clean_content,
                    chunk_type: "document".to_string(),
                    image_path: c.image_path,
                    status: "processed".to_string(),
                })
                .collect()
        }
        // CSV
        "csv" => {
            let c_chunks = extractors::csv::extract_csv(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            c_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: c.clean_content,
                    chunk_type: "document".to_string(),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        // CODE
        "py" | "js" | "ts" | "tsx" | "jsx" | "swift" | "rs" | "go" | "java" | "cpp" | "c" | "h"
        | "rb" | "php" => {
            let c_chunks = extractors::code::extract_code(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            c_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: c.clean_content,
                    chunk_type: "document".to_string(),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        // RTF
        "rtf" => {
            let r_chunks = extractors::rtf::extract_rtf(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            r_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: c.clean_content,
                    chunk_type: "document".to_string(),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        // EPUB
        "epub" => {
            let e_chunks = extractors::epub::extract_epub(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            e_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: c.clean_content,
                    chunk_type: "document".to_string(),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        // HTML
        "html" | "htm" => {
            let h_chunks = extractors::html::extract_html(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            h_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: c.clean_content,
                    chunk_type: "document".to_string(),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        // Images
        "png" | "jpg" | "jpeg" | "webp" | "gif" => {
            vec![FileChunk {
                content: format!("Image File: {}", title),
                chunk_type: "image".to_string(),
                image_path: Some(file_path.to_string()),
                status: "processed".to_string(),
            }]
        }
        _ => return Err(format!("Unsupported file extension: {}", ext)),
    };

    Ok(ParsedDocument { chunks, title })
}

pub fn take_screenshot() -> Result<String, String> {
    use chrono::Local;
    use screenshots::Screen;

    let screens = Screen::all().map_err(|e| format!("Failed to detect screens: {}", e))?;

    // For simplicity of MVP, just capture the primary monitor
    let screen = screens.first().ok_or("No screen found")?;

    let image = screen
        .capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    // Convert screenshot into a PNG byte vector
    use std::io::Cursor;
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    image
        .write_to(&mut cursor, screenshots::image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode screenshot: {}", e))?;

    // Save locally
    let now = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let _filename = format!("screenshot_{}.png", now);

    // Save to the user's data dir / captures
    // But since we don't have AppHandle here, we'll return the base64 string to the command to save it properly

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let b64 = STANDARD.encode(&buffer);

    Ok(b64)
}
