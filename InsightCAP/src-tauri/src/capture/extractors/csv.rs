use crate::error::AppError;
use csv::ReaderBuilder;
use std::fs::File;
use std::path::Path;

pub struct CsvChunk {
    pub clean_content: String,
}

pub async fn extract_csv(
    _kb_path: &str,
    file_path: &str,
    file_stem: &str,
) -> Result<Vec<CsvChunk>, AppError> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(AppError::Capture(format!(
            "CSV file not found: {}",
            file_path
        )));
    }

    let file = File::open(file_path)
        .map_err(|e| AppError::Capture(format!("Failed to open CSV file: {}", e)))?;
    let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(file);

    let headers = rdr
        .headers()
        .map_err(|e| AppError::Capture(format!("Failed to read CSV headers: {}", e)))?
        .clone();

    let mut rows = vec![headers.iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    for result in rdr.records() {
        let record =
            result.map_err(|e| AppError::Capture(format!("Failed to parse CSV record: {}", e)))?;
        rows.push(record.iter().map(|s| s.to_string()).collect());
    }

    let mut content = format!("[CSV: {}]\n", file_stem);
    content.push_str(&crate::capture::extractors::preclean::format_table_as_markdown(&rows));

    Ok(vec![CsvChunk {
        clean_content: content.trim().to_string(),
    }])
}

pub fn read_csv_full(file_path: &str) -> Result<String, AppError> {
    let file = File::open(file_path)
        .map_err(|e| AppError::Capture(format!("Failed to open CSV file: {}", e)))?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(file);

    let mut full_content = String::new();
    for result in rdr.records() {
        if let Ok(record) = result {
            let row_str: Vec<String> = record.iter().map(|s| s.to_string()).collect();
            full_content.push_str(&format!("{}\n", row_str.join(" | ")));
        }
    }

    Ok(full_content.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn extract_csv_outputs_markdown_table() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Region,Revenue").unwrap();
        writeln!(file, "North,100").unwrap();
        writeln!(file, "South,200").unwrap();

        let chunks = extract_csv("", file.path().to_str().unwrap(), "sales")
            .await
            .unwrap();

        assert_eq!(chunks.len(), 1);
        assert!(chunks[0]
            .clean_content
            .starts_with("[CSV: sales]\n| Region | Revenue |"));
        assert!(chunks[0].clean_content.contains("| --- | --- |"));
        assert!(chunks[0].clean_content.contains("| North | 100 |"));
    }
}
