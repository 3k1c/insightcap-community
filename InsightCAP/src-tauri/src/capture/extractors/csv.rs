//! # CSV 提取器 (懶加載)
//!
//! 實作 csv 表頭索引、隨機樣本提取與總行數統計。
//! 見 Phase3.1.md P3.1-08。

use crate::error::AppError;
use csv::ReaderBuilder;
use rand::seq::SliceRandom;
use std::fs::File;
use std::path::Path;

pub struct CsvChunk {
    pub clean_content: String,
}

/// 提取 csv 內容 (索引結構與部分樣本)
pub async fn extract_csv(
    _kb_path: &str,
    file_path: &str,
    file_stem: &str,
) -> Result<Vec<CsvChunk>, AppError> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(AppError::Capture(format!("檔案不存在: {}", file_path)));
    }

    let file =
        File::open(file_path).map_err(|e| AppError::Capture(format!("CSV 開啟失敗: {}", e)))?;
    let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(file);

    // 1. 提取表頭
    let headers = rdr
        .headers()
        .map_err(|e| AppError::Capture(format!("CSV 表頭讀取失敗: {}", e)))?
        .clone();

    let header_str = headers.iter().collect::<Vec<_>>().join(" | ");

    // 2. 累計所有數據行 (為了隨機抽樣與統計總數)
    // 注意：對於極大型文件，這可能會消耗較多內存。
    // 但作為索引用途，我們暫時全量讀取以確保隨機性。
    let mut records = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| AppError::Capture(format!("CSV 數據讀取失敗: {}", e)))?;
        records.push(record);
    }

    let total_rows = records.len();
    let mut sample_rows = Vec::new();

    if total_rows > 0 {
        let mut indices: Vec<usize> = (0..total_rows).collect();
        let mut rng = rand::rng();
        indices.shuffle(&mut rng);

        let sample_size = std::cmp::min(5, total_rows);
        let selected_indices = &indices[0..sample_size];

        for &idx in selected_indices {
            let row = &records[idx];
            let row_str: Vec<String> = row.iter().map(|s| s.to_string()).collect();
            sample_rows.push(row_str.join(" | "));
        }
    }

    // 3. 組合 clean_content
    let mut content = format!("[CSV: {}]\n", file_stem);
    content.push_str(&format!("欄位：{}\n", header_str));
    content.push_str(&format!("樣本（共 {} 行）：\n", total_rows));
    for row in sample_rows {
        content.push_str(&format!("{}\n", row));
    }

    Ok(vec![CsvChunk {
        clean_content: content.trim().to_string(),
    }])
}

/// 讀取 CSV 的完整資料 (按需載入用)
pub fn read_csv_full(file_path: &str) -> Result<String, AppError> {
    let file =
        File::open(file_path).map_err(|e| AppError::Capture(format!("CSV 開啟失敗: {}", e)))?;
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
