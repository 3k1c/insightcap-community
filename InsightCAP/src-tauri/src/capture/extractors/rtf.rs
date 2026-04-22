use crate::error::AppError;
use regex::Regex;
use rtf_parser::RtfDocument;

pub struct RtfChunk {
    pub clean_content: String,
}

pub async fn extract_rtf(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
) -> Result<Vec<RtfChunk>, AppError> {
    let content = crate::capture::encoding::read_text_file(file_path)
        .map_err(|e| AppError::Capture(format!("Failed to read RTF file: {}", e)))?;

    let re_par = Regex::new(r"\\par([^a-zA-Z]|$)").unwrap();
    let re_line = Regex::new(r"\\line([^a-zA-Z]|$)").unwrap();

    let preprocessed = re_par.replace_all(&content, "\\par [[PAR_BREAK]]$1");
    let preprocessed = re_line.replace_all(&preprocessed, "\\line [[PAR_BREAK]]$1");

    let document = RtfDocument::try_from(preprocessed.as_ref())
        .map_err(|e| AppError::Capture(format!("Failed to parse RTF document: {}", e)))?;

    let clean_text = document.get_text();

    let mut chunks = Vec::new();
    for paragraph in clean_text.split("[[PAR_BREAK]]") {
        for sub_paragraph in paragraph.split('\n') {
            let trimmed = sub_paragraph.trim();
            if !trimmed.is_empty() {
                chunks.push(RtfChunk {
                    clean_content: trimmed.to_string(),
                });
            }
        }
    }

    Ok(chunks)
}
