use std::fs;
use std::path::Path;

use crate::providers::llm::vision::VisionConfig;
use serde_json::{json, Value};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileChunk {
    pub content: String,
    pub chunk_type: String, // "text" | "image" | "document" | "mixed"
    pub source_type: String,
    pub metadata: Value,
    pub image_path: Option<String>,
    pub status: String, // "processed" | "pending_ocr"
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ParsedDocument {
    pub chunks: Vec<FileChunk>,
    pub title: String,
}

pub async fn parse_file(
    kb_path: &str,
    file_path: &str,
    vision: Option<&VisionConfig>,
) -> Result<ParsedDocument, String> {
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
        "txt" | "log" => {
            let content = extractors::txt::extract_txt(file_path)
                .map_err(|e| format!("Failed to read text file: {}", e))?;
            vec![FileChunk {
                content: extractors::preclean::normalize_extracted_markdown(&content),
                chunk_type: "text".to_string(),
                source_type: if ext == "log" { "log" } else { "plain_text" }.to_string(),
                metadata: json!({
                    "source_type": if ext == "log" { "log" } else { "plain_text" },
                    "file_name": title,
                }),
                image_path: None,
                status: "processed".to_string(),
            }]
        }
        "md" => {
            let content = extractors::md::extract_md(file_path)
                .map_err(|e| format!("Failed to read markdown file: {}", e))?;
            let content = extractors::preclean::normalize_extracted_markdown(&content);
            vec![FileChunk {
                metadata: json!({
                    "source_type": "markdown",
                    "file_name": title,
                    "heading": extractors::preclean::extract_heading_context(&content),
                }),
                content,
                chunk_type: "text".to_string(),
                source_type: "markdown".to_string(),
                image_path: None,
                status: "processed".to_string(),
            }]
        }
        "pdf" => {
            let p_chunks = extractors::pdf::extract_pdf(kb_path, file_path, file_stem, vision)
                .await
                .map_err(|e| e.to_string())?;
            p_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: extractors::preclean::normalize_extracted_markdown(&c.clean_content),
                    source_type: if c.chunk_type == "image" {
                        "image"
                    } else {
                        "pdf"
                    }
                    .to_string(),
                    metadata: json!({
                        "source_type": if c.chunk_type == "image" { "image" } else { "pdf" },
                        "page_number": c.page_num,
                    }),
                    chunk_type: c.chunk_type,
                    image_path: c.image_path,
                    status: c.status,
                })
                .collect()
        }
        "docx" => {
            let d_chunks = extractors::docx::extract_docx(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            d_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: extractors::preclean::normalize_extracted_markdown(&c.clean_content),
                    source_type: if c.chunk_type == "image" {
                        "image"
                    } else {
                        "docx"
                    }
                    .to_string(),
                    metadata: json!({
                        "source_type": if c.chunk_type == "image" { "image" } else { "docx" },
                        "heading_level": c.title_level,
                    }),
                    chunk_type: c.chunk_type,
                    image_path: c.image_path,
                    status: "processed".to_string(),
                })
                .collect()
        }
        "xlsx" => {
            let x_chunks = extractors::xlsx::extract_xlsx(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            x_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: extractors::preclean::normalize_extracted_markdown(&c.clean_content),
                    chunk_type: "document".to_string(),
                    source_type: "xlsx".to_string(),
                    metadata: json!({
                        "source_type": "xlsx",
                        "sheet_name": c.sheet_name,
                    }),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        "pptx" => {
            let p_chunks = extractors::pptx::extract_pptx(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            p_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: extractors::preclean::normalize_extracted_markdown(&c.clean_content),
                    chunk_type: "document".to_string(),
                    source_type: "pptx".to_string(),
                    metadata: json!({
                        "source_type": "pptx",
                        "slide_number": c.slide_index,
                    }),
                    image_path: c.image_path,
                    status: "processed".to_string(),
                })
                .collect()
        }
        "csv" => {
            let c_chunks = extractors::csv::extract_csv(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            c_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: extractors::preclean::normalize_extracted_markdown(&c.clean_content),
                    chunk_type: "document".to_string(),
                    source_type: "csv".to_string(),
                    metadata: json!({
                        "source_type": "csv",
                    }),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        "py" | "js" | "ts" | "tsx" | "jsx" | "swift" | "rs" | "go" | "java" | "cpp" | "c" | "h"
        | "rb" | "php" => {
            let c_chunks = extractors::code::extract_code(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            c_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: extractors::preclean::normalize_text_basics(&c.clean_content)
                        .trim()
                        .to_string(),
                    chunk_type: "document".to_string(),
                    source_type: "code".to_string(),
                    metadata: json!({
                        "source_type": "code",
                        "language": c.language,
                        "file_name": title,
                    }),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        "rtf" => {
            let r_chunks = extractors::rtf::extract_rtf(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            r_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: extractors::preclean::normalize_extracted_markdown(&c.clean_content),
                    chunk_type: "document".to_string(),
                    source_type: "rtf".to_string(),
                    metadata: json!({
                        "source_type": "rtf",
                    }),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        "epub" => {
            let e_chunks = extractors::epub::extract_epub(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            e_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: extractors::preclean::normalize_extracted_markdown(&c.clean_content),
                    chunk_type: "document".to_string(),
                    source_type: "epub".to_string(),
                    metadata: json!({
                        "source_type": "epub",
                        "heading": c.title,
                    }),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        "html" | "htm" => {
            let h_chunks = extractors::html::extract_html(kb_path, file_path, file_stem)
                .await
                .map_err(|e| e.to_string())?;
            h_chunks
                .into_iter()
                .map(|c| FileChunk {
                    content: extractors::preclean::normalize_extracted_markdown(&c.clean_content),
                    chunk_type: "document".to_string(),
                    source_type: "html".to_string(),
                    metadata: json!({
                        "source_type": "html",
                        "heading": c.title,
                    }),
                    image_path: None,
                    status: "processed".to_string(),
                })
                .collect()
        }
        "png" | "jpg" | "jpeg" | "webp" | "gif" => {
            let image_bytes =
                fs::read(path).map_err(|e| format!("Failed to read image file: {}", e))?;

            // Fix #6: on OCR failure, use an empty string and mark status as "ocr_failed".
            // chunking.rs will skip chunks with this status, preventing garbage from
            // entering the vector index.
            let (ocr_text, ocr_failed) = match crate::ocr::perform_ocr(&image_bytes).await {
                Ok(raw) => {
                    let lang = crate::ocr::postprocess::detect_language(&raw);
                    (crate::ocr::postprocess::postprocess_ocr_text(&raw, lang), false)
                }
                Err(e) => {
                    eprintln!("[FileParser] OCR failed for {}: {}", title, e);
                    (String::new(), true)
                }
            };

            let (final_text, final_status) = if ocr_failed {
                // OCR failed and no vision fallback — mark as failed so chunking skips it
                (String::new(), "ocr_failed".to_string())
            } else if let Some(vc) = vision {
                match crate::providers::llm::vision::try_vision_enhance(
                    vc,
                    &image_bytes,
                    crate::providers::llm::vision::general_vision_prompt(),
                )
                .await
                {
                    Some(vision_text) => {
                        println!("[FileParser] Vision enhancement succeeded: {}", title);
                        (vision_text, "processed".to_string())
                    }
                    None => (ocr_text, "processed".to_string()),
                }
            } else {
                (ocr_text, "processed".to_string())
            };

            vec![FileChunk {
                content: extractors::preclean::normalize_extracted_markdown(&final_text),
                chunk_type: "image".to_string(),
                source_type: "image".to_string(),
                metadata: json!({
                    "source_type": "image",
                    "file_name": title,
                }),
                image_path: Some(file_path.to_string()),
                status: final_status,
            }]
        }
        _ => return Err(format!("Unsupported file extension: {}", ext)),
    };

    Ok(ParsedDocument { chunks, title })
}

pub async fn parse_content(
    kb_path: &str,
    file_path: Option<String>,
    url: Option<String>,
    sessdata: Option<String>,
    vision: Option<&VisionConfig>,
) -> Result<ParsedDocument, String> {
    if let Some(path) = file_path {
        return parse_file(kb_path, &path, vision).await;
    }
    if let Some(url_str) = url {
        return crate::capture::video_parser::parse_url_content(&url_str, sessdata).await;
    }
    Err("Requires either a file path or URL".to_string())
}

pub fn take_screenshot() -> Result<String, String> {
    use chrono::Local;
    use screenshots::Screen;

    let screens = Screen::all().map_err(|e| format!("Failed to detect screens: {}", e))?;

    let screen = screens.first().ok_or("No screen found")?;

    let image = screen
        .capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    use std::io::Cursor;
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    image
        .write_to(&mut cursor, screenshots::image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode screenshot: {}", e))?;

    let now = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let _filename = format!("screenshot_{}.png", now);

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let b64 = STANDARD.encode(&buffer);

    Ok(b64)
}
