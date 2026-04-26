use crate::error::AppError;
use calamine::{open_workbook, Data, Reader, Xlsx};
use std::path::Path;

pub struct XlsxChunk {
    pub sheet_name: String,
    pub clean_content: String,
}

pub async fn extract_xlsx(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
) -> Result<Vec<XlsxChunk>, AppError> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(AppError::Capture(format!(
            "XLSX file not found: {}",
            file_path
        )));
    }

    let mut excel: Xlsx<_> = open_workbook(file_path)
        .map_err(|e| AppError::Capture(format!("Failed to open XLSX: {}", e)))?;

    let sheet_names = excel.sheet_names();
    let mut chunks = Vec::new();

    for sheet_name in sheet_names {
        if let Ok(range) = excel.worksheet_range(&sheet_name) {
            let total_rows = range.height();
            if total_rows == 0 {
                continue;
            }

            let rows = range
                .rows()
                .map(|row| row.iter().map(|c: &Data| c.to_string()).collect::<Vec<_>>())
                .collect::<Vec<_>>();

            let mut content = format!("[Sheet: {}]\n", sheet_name);
            content
                .push_str(&crate::capture::extractors::preclean::format_table_as_markdown(&rows));

            chunks.push(XlsxChunk {
                sheet_name,
                clean_content: content.trim().to_string(),
            });
        }
    }

    Ok(chunks)
}

pub fn read_xlsx_full(file_path: &str) -> Result<String, AppError> {
    let mut excel: Xlsx<_> = open_workbook(file_path)
        .map_err(|e| AppError::Capture(format!("Failed to open XLSX: {}", e)))?;

    let sheet_names = excel.sheet_names();
    let mut full_content = String::new();

    for sheet_name in sheet_names {
        if let Ok(range) = excel.worksheet_range(&sheet_name) {
            full_content.push_str(&format!("\n=== Sheet: {} ===\n", sheet_name));

            for row in range.rows() {
                let row_str: Vec<String> = row.iter().map(|c| c.to_string()).collect();
                full_content.push_str(&format!("{}\n", row_str.join(" | ")));
            }
        }
    }

    Ok(full_content.trim().to_string())
}
