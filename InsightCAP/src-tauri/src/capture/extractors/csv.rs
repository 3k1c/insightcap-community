use crate::error::AppError;
use csv::ReaderBuilder;
use rand::seq::SliceRandom;
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

    let header_str = headers.iter().collect::<Vec<_>>().join(" | ");

    let mut records = Vec::new();
    for result in rdr.records() {
        let record =
            result.map_err(|e| AppError::Capture(format!("Failed to parse CSV record: {}", e)))?;
        records.push(record);
    }

    let total_rows = records.len();
    let mut sample_rows = Vec::new();

    if total_rows > 0 {
        let mut indices: Vec<usize> = (0..total_rows).collect();
        let mut rng = rand::rng();
        indices.shuffle(&mut rng);

        let sample_size = std::cmp::min(5, total_rows);
        for &idx in &indices[0..sample_size] {
            let row = &records[idx];
            let row_str: Vec<String> = row.iter().map(|s| s.to_string()).collect();
            sample_rows.push(row_str.join(" | "));
        }
    }

    let mut content = format!("[CSV: {}]\n", file_stem);
    content.push_str(&format!("Headers: {}\n", header_str));
    content.push_str(&format!("Total rows: {}\n", total_rows));
    for row in sample_rows {
        content.push_str(&format!("{}\n", row));
    }

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
