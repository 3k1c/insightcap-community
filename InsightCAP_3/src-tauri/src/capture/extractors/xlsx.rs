//! # Excel 提取器 (懶加載)
//!
//! 實作 xlsx 表頭索引、隨機樣本提取與總行數統計。
//! 見 Phase3.1.md P3.1-04。

use crate::error::AppError;
use calamine::{open_workbook, Data, Reader, Xlsx};
use rand::seq::SliceRandom;
use std::path::Path;

pub struct XlsxChunk {
    pub sheet_name: String,
    pub clean_content: String,
}

/// 提取 xlsx 內容 (索引結構與部分樣本)
pub async fn extract_xlsx(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
) -> Result<Vec<XlsxChunk>, AppError> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(AppError::Capture(format!("檔案不存在: {}", file_path)));
    }

    let mut excel: Xlsx<_> = open_workbook(file_path)
        .map_err(|e| AppError::Capture(format!("Excel 開啟失敗: {}", e)))?;

    let sheet_names = excel.sheet_names();
    let mut chunks = Vec::new();

    for sheet_name in sheet_names {
        if let Ok(range) = excel.worksheet_range(&sheet_name) {
            let total_rows = range.height();
            if total_rows == 0 {
                continue;
            }

            // 1. 提取表頭 (預設第一行)
            let mut headers = Vec::new();
            if let Some(first_row) = range.rows().next() {
                for cell in first_row {
                    headers.push(cell.to_string());
                }
            }
            let header_str = headers.join(" | ");

            // 2. 隨機抽取 5 行樣本 (扣除表頭，從 index 1 開始)
            let data_rows_count = if total_rows > 1 { total_rows - 1 } else { 0 };
            let mut sample_rows = Vec::new();

            if data_rows_count > 0 {
                let mut all_indices: Vec<usize> = (1..total_rows).collect();
                let mut rng = rand::rng();
                all_indices.shuffle(&mut rng);

                let sample_size = std::cmp::min(5, data_rows_count);
                let selected_indices = &all_indices[0..sample_size];

                for &idx in selected_indices {
                    if let Some(row) = range.rows().nth(idx) {
                        let row_str: Vec<String> =
                            row.iter().map(|c: &Data| c.to_string()).collect();
                        sample_rows.push(row_str.join(" | "));
                    }
                }
            }

            // 3. 組合 clean_content
            let mut content = format!("[Sheet: {}]\n", sheet_name);
            content.push_str(&format!("欄位：{}\n", header_str));
            content.push_str(&format!("樣本（共 {} 行）：\n", data_rows_count));
            for row in sample_rows {
                content.push_str(&format!("{}\n", row));
            }

            chunks.push(XlsxChunk {
                sheet_name,
                clean_content: content.trim().to_string(),
            });
        }
    }

    Ok(chunks)
}

/// 讀取 xlsx 的完整資料 (按需載入用)
pub fn read_xlsx_full(file_path: &str) -> Result<String, AppError> {
    let mut excel: Xlsx<_> = open_workbook(file_path)
        .map_err(|e| AppError::Capture(format!("Excel 開啟失敗: {}", e)))?;

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
