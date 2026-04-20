//! # 編碼感知文字讀取
//!
//! 先嘗試 UTF-8，失敗時用 chardetng 偵測編碼再以 encoding_rs 轉換。
//! 最後以 lossy 模式保底，確保任何檔案都能讀進來。

use std::fs;
use std::path::Path;

/// 讀取文字檔案，自動處理非 UTF-8 編碼（Big5、GBK、Shift-JIS 等）。
pub fn read_text_file(path: impl AsRef<Path>) -> Result<String, String> {
    let bytes = fs::read(path.as_ref()).map_err(|e| format!("Failed to read file: {}", e))?;

    // 快速路徑：UTF-8 合法就直接回傳
    if let Ok(s) = std::str::from_utf8(&bytes) {
        return Ok(s.to_string());
    }

    // 偵測編碼
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(&bytes, true);
    let encoding = detector.guess(None, true);

    let (decoded, _encoding_used, had_errors) = encoding.decode(&bytes);
    if had_errors {
        // 仍有無法轉換的位元組 → lossy 保底
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    } else {
        Ok(decoded.into_owned())
    }
}
