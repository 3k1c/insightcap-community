//! # PowerPoint 提取器
//!
//! 實作 pptx 每頁文字提取（標題、內文、備註）與圖片儲存。
//! 見 Phase3.1.md P3.1-05。

use crate::capture::attachment_manager::copy_image_to_attachments;
use crate::error::AppError;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub struct PptxChunk {
    pub slide_index: usize,
    pub clean_content: String,
    pub image_path: Option<String>,
}

/// 提取 pptx 內容
pub async fn extract_pptx(
    kb_path: &str,
    file_path: &str,
    file_stem: &str,
) -> Result<Vec<PptxChunk>, AppError> {
    let file = File::open(file_path).map_err(|e| AppError::Capture(e.to_string()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| AppError::Capture(format!("PPTX 解壓失敗: {}", e)))?;

    let mut slide_texts = BTreeMap::new();
    let mut slide_notes = BTreeMap::new();
    let mut media_files = Vec::new();

    // 1. 遍歷 ZIP 檔案，找出投影片、備註與媒體
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| AppError::Capture(e.to_string()))?;
        let name = file.name().to_string();

        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            if let Some(idx) = parse_index(&name, "ppt/slides/slide", ".xml") {
                let content = read_to_string(file)?;
                let text = extract_text_from_xml(&content);
                slide_texts.insert(idx, text);
            }
        } else if name.starts_with("ppt/notesSlides/notesSlide") && name.ends_with(".xml") {
            // 注意：notesSlide 會透過 rels 關聯 slide，
            // 簡單起見，通常索引是按順序對應的。
            if let Some(idx) = parse_index(&name, "ppt/notesSlides/notesSlide", ".xml") {
                let content = read_to_string(file)?;
                let text = extract_text_from_xml(&content);
                slide_notes.insert(idx, text);
            }
        } else if name.starts_with("ppt/media/") {
            media_files.push(name);
        }
    }

    let mut chunks = Vec::new();

    // 2. 組合每張投影片的文字 Chunk
    // 取得所有 slide 序號並排序
    let mut slide_indices: Vec<_> = slide_texts.keys().cloned().collect();
    slide_indices.sort();

    for idx in slide_indices {
        let text = slide_texts.get(&idx).cloned().unwrap_or_default();
        let notes = slide_notes.get(&idx).cloned().unwrap_or_default();

        let mut combined = format!("[Slide {}]\n{}", idx, text);
        if !notes.trim().is_empty() {
            combined.push_str("\n\n-- 備註 --\n");
            combined.push_str(&notes);
        }

        chunks.push(PptxChunk {
            slide_index: idx,
            clean_content: combined.trim().to_string(),
            image_path: None,
        });
    }

    // 3. 處理圖片
    for (i, media_name) in media_files.iter().enumerate() {
        let mut file = archive
            .by_name(media_name)
            .map_err(|e| AppError::Capture(e.to_string()))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| AppError::Capture(e.to_string()))?;

        let ext = Path::new(media_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg");

        let prefix = format!("{}_slide_img{}", file_stem, i + 1);

        match copy_image_to_attachments(kb_path, &data, &prefix, ext).await {
            Ok(path) => {
                chunks.push(PptxChunk {
                    slide_index: 0, // 圖片不特定掛在某頁（除非解析 rels）
                    clean_content: "[圖片]".to_string(),
                    image_path: Some(path.to_string_lossy().to_string()),
                });
            }
            Err(e) => eprintln!("[PPTX] 提取圖片失敗: {}", e),
        }
    }

    Ok(chunks)
}

fn parse_index(name: &str, prefix: &str, suffix: &str) -> Option<usize> {
    name.strip_prefix(prefix)?
        .strip_suffix(suffix)?
        .parse()
        .ok()
}

fn read_to_string(mut file: zip::read::ZipFile<'_, File>) -> Result<String, AppError> {
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| AppError::Capture(format!("讀取 XML 失敗: {}", e)))?;
    Ok(content)
}

fn extract_text_from_xml(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut extracted = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                if let Ok(text) = std::str::from_utf8(e.as_ref()) {
                    let cleaned = text.trim();
                    if !cleaned.is_empty() {
                        extracted.push_str(cleaned);
                        extracted.push(' ');
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }
    extracted.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_extract_pptx_slides_and_notes() {
        let temp_dir = tempdir().unwrap();
        let kb_path = temp_dir.path().to_str().unwrap();

        // 建立假 PPTX 檔案 (ZIP 格式)
        let pptx_path = temp_dir.path().join("test.pptx");
        let file = std::fs::File::create(&pptx_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);

        let options = zip::write::SimpleFileOptions::default();

        // Slide 1
        zip.start_file("ppt/slides/slide1.xml", options).unwrap();
        zip.write_all(b"<p:txBody><a:t>Hello Slide 1</a:t></p:txBody>")
            .unwrap();

        // Slide 2
        zip.start_file("ppt/slides/slide2.xml", options).unwrap();
        zip.write_all(b"<p:txBody><a:t>Content of Slide 2</a:t></p:txBody>")
            .unwrap();

        // Note for Slide 1
        zip.start_file("ppt/notesSlides/notesSlide1.xml", options)
            .unwrap();
        zip.write_all(b"<p:txBody><a:t>Note for slide 1</a:t></p:txBody>")
            .unwrap();

        // Mock Media
        zip.start_file("ppt/media/image1.jpg", options).unwrap();
        zip.write_all(b"fake image data").unwrap();

        zip.finish().unwrap();

        let mut chunks = extract_pptx(kb_path, pptx_path.to_str().unwrap(), "test")
            .await
            .unwrap();

        assert_eq!(chunks.len(), 3); // 2 slides + 1 image

        // Sort slides just to be sure
        chunks.sort_by_key(|c| c.slide_index);

        // chunk 0: image (index 0)
        assert_eq!(chunks[0].slide_index, 0);
        assert_eq!(chunks[0].clean_content, "[圖片]");
        assert!(chunks[0].image_path.is_some());

        // chunk 1: slide 1 (with notes)
        assert_eq!(chunks[1].slide_index, 1);
        assert!(chunks[1].clean_content.contains("Hello Slide 1"));
        assert!(chunks[1].clean_content.contains("Note for slide 1"));

        // chunk 2: slide 2 (no notes)
        assert_eq!(chunks[2].slide_index, 2);
        assert!(chunks[2].clean_content.contains("Content of Slide 2"));
        assert!(!chunks[2].clean_content.contains("Note for slide"));
    }
}
