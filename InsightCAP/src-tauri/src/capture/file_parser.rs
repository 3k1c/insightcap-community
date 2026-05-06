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

const AUDIO_TRANSCRIPT_CHUNK_CHARS: usize = 2200;

fn format_transcript_line(segment: &crate::whisper_transcribe::TranscriptSegment) -> String {
    if segment.end_ms > segment.start_ms {
        format!(
            "[{} - {}] {}",
            crate::whisper_transcribe::format_timestamp_ms(segment.start_ms),
            crate::whisper_transcribe::format_timestamp_ms(segment.end_ms),
            segment.text.trim()
        )
    } else {
        segment.text.trim().to_string()
    }
}

fn build_audio_transcript_chunk(
    lines: &[String],
    start_ms: i64,
    end_ms: i64,
    segment_count: usize,
    file_name: &str,
) -> FileChunk {
    FileChunk {
        content: lines.join("\n"),
        chunk_type: "audio_transcript".to_string(),
        source_type: "audio_transcript".to_string(),
        metadata: json!({
            "source_type": "audio_transcript",
            "file_name": file_name,
            "transcript_start_ms": start_ms,
            "transcript_end_ms": end_ms,
            "transcript_start": crate::whisper_transcribe::format_timestamp_ms(start_ms),
            "transcript_end": crate::whisper_transcribe::format_timestamp_ms(end_ms),
            "segment_count": segment_count,
        }),
        image_path: None,
        status: "processed".to_string(),
    }
}

fn transcript_segments_to_file_chunks(
    segments: &[crate::whisper_transcribe::TranscriptSegment],
    file_name: &str,
) -> Vec<FileChunk> {
    let mut chunks = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut current_chars = 0usize;
    let mut current_start_ms = 0i64;
    let mut current_end_ms = 0i64;
    let mut current_segment_count = 0usize;

    for segment in segments {
        let line = format_transcript_line(segment);
        if line.trim().is_empty() {
            continue;
        }

        let line_chars = line.chars().count();
        if !lines.is_empty() && current_chars + line_chars > AUDIO_TRANSCRIPT_CHUNK_CHARS {
            chunks.push(build_audio_transcript_chunk(
                &lines,
                current_start_ms,
                current_end_ms,
                current_segment_count,
                file_name,
            ));
            lines.clear();
            current_chars = 0;
            current_segment_count = 0;
        }

        if lines.is_empty() {
            current_start_ms = segment.start_ms;
        }
        current_end_ms = segment.end_ms;
        current_chars += line_chars + 1;
        current_segment_count += 1;
        lines.push(line);
    }

    if !lines.is_empty() {
        chunks.push(build_audio_transcript_chunk(
            &lines,
            current_start_ms,
            current_end_ms,
            current_segment_count,
            file_name,
        ));
    }

    chunks
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
                    (
                        crate::ocr::postprocess::postprocess_ocr_text(&raw, lang),
                        false,
                    )
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
        "wav" | "mp3" | "m4a" | "aac" | "flac" | "ogg" | "opus" | "webm" => {
            let model = crate::whisper_transcribe::read_model_preference();
            let segments =
                crate::whisper_transcribe::transcribe_audio_file(path, model, Some("auto")).await?;
            if segments.is_empty() {
                return Err("Whisper returned empty transcript".to_string());
            }
            transcript_segments_to_file_chunks(&segments, &title)
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

#[cfg(test)]
mod audio_tests {
    use super::*;

    #[test]
    fn groups_transcript_segments_with_time_metadata() {
        let segments = vec![
            crate::whisper_transcribe::TranscriptSegment {
                start_ms: 1_000,
                end_ms: 2_500,
                text: "First segment.".to_string(),
            },
            crate::whisper_transcribe::TranscriptSegment {
                start_ms: 2_500,
                end_ms: 4_000,
                text: "Second segment.".to_string(),
            },
        ];

        let chunks = transcript_segments_to_file_chunks(&segments, "meeting.m4a");

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].source_type, "audio_transcript");
        assert!(chunks[0]
            .content
            .contains("[00:00:01.000 - 00:00:02.500] First segment."));
        assert_eq!(chunks[0].metadata["transcript_start_ms"], 1_000);
        assert_eq!(chunks[0].metadata["transcript_end_ms"], 4_000);
        assert_eq!(chunks[0].metadata["segment_count"], 2);
        assert_eq!(chunks[0].metadata["file_name"], "meeting.m4a");
    }
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
