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

pub async fn extract_pptx(
    kb_path: &str,
    file_path: &str,
    file_stem: &str,
) -> Result<Vec<PptxChunk>, AppError> {
    let file = File::open(file_path).map_err(|e| AppError::Capture(e.to_string()))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| AppError::Capture(format!("Failed to read PPTX archive: {}", e)))?;

    let mut slide_texts = BTreeMap::new();
    let mut slide_notes = BTreeMap::new();
    let mut media_files = Vec::new();

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

    let mut slide_indices: Vec<_> = slide_texts.keys().cloned().collect();
    slide_indices.sort();

    for idx in slide_indices {
        let text = slide_texts.get(&idx).cloned().unwrap_or_default();
        let notes = slide_notes.get(&idx).cloned().unwrap_or_default();

        let mut combined = format!("[Slide {}/{}]\n\nMain:\n{}", idx, slide_texts.len(), text);
        if !notes.trim().is_empty() {
            combined.push_str("\n\nNotes:\n");
            combined.push_str(&notes);
        }

        chunks.push(PptxChunk {
            slide_index: idx,
            clean_content: combined.trim().to_string(),
            image_path: None,
        });
    }

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
                    slide_index: 0, // not tied to a specific slide index
                    clean_content: "[Image Attachment]".to_string(),
                    image_path: Some(path.to_string_lossy().to_string()),
                });
            }
            Err(e) => eprintln!("[PPTX] Failed to copy media asset: {}", e),
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
        .map_err(|e| AppError::Capture(format!("Failed to read XML in PPTX: {}", e)))?;
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

        let pptx_path = temp_dir.path().join("test.pptx");
        let file = std::fs::File::create(&pptx_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);

        let options = zip::write::SimpleFileOptions::default();

        zip.start_file("ppt/slides/slide1.xml", options).unwrap();
        zip.write_all(b"<p:txBody><a:t>Hello Slide 1</a:t></p:txBody>")
            .unwrap();

        zip.start_file("ppt/slides/slide2.xml", options).unwrap();
        zip.write_all(b"<p:txBody><a:t>Content of Slide 2</a:t></p:txBody>")
            .unwrap();

        zip.start_file("ppt/notesSlides/notesSlide1.xml", options)
            .unwrap();
        zip.write_all(b"<p:txBody><a:t>Note for slide 1</a:t></p:txBody>")
            .unwrap();

        zip.start_file("ppt/media/image1.jpg", options).unwrap();
        zip.write_all(b"fake image data").unwrap();

        zip.finish().unwrap();

        let mut chunks = extract_pptx(kb_path, pptx_path.to_str().unwrap(), "test")
            .await
            .unwrap();

        assert_eq!(chunks.len(), 3); // 2 slides + 1 image

        chunks.sort_by_key(|c| c.slide_index);

        assert_eq!(chunks[0].slide_index, 0);
        assert_eq!(chunks[0].clean_content, "[Image Attachment]");
        assert!(chunks[0].image_path.is_some());

        assert_eq!(chunks[1].slide_index, 1);
        assert!(chunks[1].clean_content.starts_with("[Slide 1/2]"));
        assert!(chunks[1].clean_content.contains("Main:\nHello Slide 1"));
        assert!(chunks[1].clean_content.contains("Notes:\nNote for slide 1"));

        assert_eq!(chunks[2].slide_index, 2);
        assert!(chunks[2].clean_content.starts_with("[Slide 2/2]"));
        assert!(chunks[2].clean_content.contains("Content of Slide 2"));
        assert!(!chunks[2].clean_content.contains("Note for slide"));
    }
}
