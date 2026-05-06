use crate::capture::attachment_manager::copy_image_to_attachments;
use crate::error::AppError;
use docx_rs::*;
use std::fs;
use std::path::Path;

pub struct DocxChunk {
    pub title_level: Option<usize>,
    pub clean_content: String,
    pub image_path: Option<String>,
    pub chunk_type: String, // "text" | "image"
}

pub async fn extract_docx(
    kb_path: &str,
    file_path: &str,
    file_stem: &str,
) -> Result<Vec<DocxChunk>, AppError> {
    let bytes = fs::read(file_path).map_err(|e| AppError::Capture(e.to_string()))?;
    let docx = docx_rs::read_docx(&bytes)
        .map_err(|e| AppError::Capture(format!("Failed to parse DOCX: {}", e)))?;

    let mut chunks = Vec::new();
    let mut current_text = String::new();
    let mut current_level: Option<usize> = None;

    for child in &docx.document.children {
        match child {
            DocumentChild::Paragraph(p) => {
                let level = get_heading_level(p);

                if let Some(l) = level {
                    if !current_text.trim().is_empty() {
                        chunks.push(DocxChunk {
                            title_level: current_level,
                            clean_content: current_text.trim().to_string(),
                            image_path: None,
                            chunk_type: "text".to_string(),
                        });
                        current_text = String::new();
                    }
                    current_level = Some(l);
                }

                let p_text = extract_paragraph_text(p);
                if !p_text.is_empty() {
                    current_text.push_str(&p_text);
                    current_text.push('\n');
                }
            }
            DocumentChild::Table(t) => {
                let t_text = extract_table_text(t);
                if !t_text.is_empty() {
                    current_text.push_str(&t_text);
                    current_text.push('\n');
                }
            }
            _ => {}
        }
    }

    if !current_text.trim().is_empty() {
        chunks.push(DocxChunk {
            title_level: current_level,
            clean_content: current_text.trim().to_string(),
            image_path: None,
            chunk_type: "text".to_string(),
        });
    }

    for (i, (name, data)) in docx.media.iter().enumerate() {
        let ext = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg");

        let prefix = format!("{}_img{}", file_stem, i + 1);

        match copy_image_to_attachments(kb_path, data, &prefix, ext).await {
            Ok(path) => {
                chunks.push(DocxChunk {
                    title_level: None,
                    clean_content: "[Image]".to_string(),
                    image_path: Some(path.to_string_lossy().to_string()),
                    chunk_type: "image".to_string(),
                });
            }
            Err(e) => eprintln!("[DOCX] Failed to store image: {}", e),
        }
    }

    Ok(chunks)
}

fn get_heading_level(p: &Paragraph) -> Option<usize> {
    if let Some(style_id) = &p.property.style {
        let style = style_id.val.to_lowercase();
        if style.starts_with("heading") {
            return style.replace("heading", "").parse().ok();
        }
    }
    None
}

fn extract_paragraph_text(p: &Paragraph) -> String {
    let mut text = String::new();
    for child in &p.children {
        match child {
            ParagraphChild::Run(r) => {
                for run_child in &r.children {
                    if let RunChild::Text(t) = run_child {
                        text.push_str(&t.text);
                    }
                }
            }
            ParagraphChild::Hyperlink(h) => {
                for child in &h.children {
                    if let ParagraphChild::Run(r) = child {
                        for run_child in &r.children {
                            if let RunChild::Text(t) = run_child {
                                text.push_str(&t.text);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    text
}

fn extract_table_text(t: &Table) -> String {
    let mut table_text = String::new();
    for table_child in &t.rows {
        let TableChild::TableRow(row) = table_child;
        let mut row_text = Vec::new();
        for row_child in &row.cells {
            let TableRowChild::TableCell(cell) = row_child;
            let mut cell_content = String::new();
            for cell_child in &cell.children {
                match cell_child {
                    TableCellContent::Paragraph(p) => {
                        cell_content.push_str(&extract_paragraph_text(p));
                    }
                    TableCellContent::Table(inner_t) => {
                        cell_content.push_str(&extract_table_text(inner_t));
                    }
                    _ => {}
                }
            }
            row_text.push(cell_content.trim().to_string());
        }
        table_text.push_str(&row_text.join(" | "));
        table_text.push('\n');
    }
    table_text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_extract_docx() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("test_extract.docx");
        let file = File::create(&path).unwrap();

        let doc = Docx::new()
            .add_paragraph(
                Paragraph::new()
                    .style("Heading1")
                    .add_run(Run::new().add_text("My Title")),
            )
            .add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("First paragraph below title.")),
            )
            .add_paragraph(
                Paragraph::new()
                    .style("Heading2")
                    .add_run(Run::new().add_text("My Subtitle")),
            )
            .add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("Second paragraph below subtitle.")),
            )
            .add_table(Table::new(vec![
                TableRow::new(vec![
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Col1"))),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Col2"))),
                ]),
                TableRow::new(vec![
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Val1"))),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Val2"))),
                ]),
            ]));

        doc.build().pack(file).unwrap();

        let chunks = extract_docx("kb_path", path.to_str().unwrap(), "test")
            .await
            .unwrap();

        assert_eq!(chunks.len(), 2);

        let chunk1 = &chunks[0];
        assert_eq!(chunk1.title_level, Some(1));
        assert!(chunk1.clean_content.contains("My Title"));
        assert!(chunk1
            .clean_content
            .contains("First paragraph below title."));
        assert_eq!(chunk1.chunk_type, "text");

        let chunk2 = &chunks[1];
        assert_eq!(chunk2.title_level, Some(2));
        assert!(chunk2.clean_content.contains("My Subtitle"));
        assert!(chunk2
            .clean_content
            .contains("Second paragraph below subtitle."));

        assert!(chunk2.clean_content.contains("Col1 | Col2"));
        assert!(chunk2.clean_content.contains("Val1 | Val2"));
        assert_eq!(chunk2.chunk_type, "text");
    }
}
