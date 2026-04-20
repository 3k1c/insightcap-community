//! # RTF 提取器
//!
//! 實作 RTF 文字提取。
//! 見 Phase3.1.md P3.1-11。

use crate::error::AppError;
use regex::Regex;
use rtf_parser::RtfDocument;

pub struct RtfChunk {
    pub clean_content: String,
}

/// 提取 RTF 內容並按段落分割
pub async fn extract_rtf(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
) -> Result<Vec<RtfChunk>, AppError> {
    let content = crate::capture::encoding::read_text_file(file_path)
        .map_err(|e| AppError::Capture(format!("RTF 讀取失敗: {}", e)))?;

    // rtf-parser 會自動過濾掉不具備文字內容的 ControlWord（包含 \par, \line），
    // 若段落樣式相同更會合併 StyleBlock，導致段落邊界遺失。
    // 解法：在傳入前利用一個不可能自然出現的佔位符替換獨立的 \par 和 \line。
    // 注意必須避免替換到 \pard 等其他指令。
    // 由於 Rust regex 不支援 lookahead，我們匹配 \par 後接一個非字母字元或字串結尾。
    let re_par = Regex::new(r"\\par([^a-zA-Z]|$)").unwrap();
    let re_line = Regex::new(r"\\line([^a-zA-Z]|$)").unwrap();

    // 替換時保留後續字元 ($1)
    let preprocessed = re_par.replace_all(&content, "\\par [[PAR_BREAK]]$1");
    let preprocessed = re_line.replace_all(&preprocessed, "\\line [[PAR_BREAK]]$1");

    // 解析 RTF
    let document = RtfDocument::try_from(preprocessed.as_ref())
        .map_err(|e| AppError::Capture(format!("RTF 解析失敗: {}", e)))?;

    // 取得所有純文字
    let clean_text = document.get_text();

    // 按段落佔位符和原生 \n 進行分割
    let mut chunks = Vec::new();
    for paragraph in clean_text.split("[[PAR_BREAK]]") {
        let p_str: &str = paragraph;
        for sub_paragraph in p_str.split('\n') {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_extract_rtf() {
        let rtf_doc = r#"{\rtf1\ansi\ansicpg1252\deff0\nouicompat{\fonttbl{\f0\fnil\fcharset0 Calibri;}}
{\*\generator Riched20 10.0.19041}\viewkind4\uc1 
\pard\sa200\sl276\slmult1\f0\fs22\lang9 Hello, \b World\b0 !\par
This is a test document.\par
And this is the third paragraph.\par
}
"#;
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("test_extract.rtf");
        let mut file = File::create(&path).unwrap();
        file.write_all(rtf_doc.as_bytes()).unwrap();

        let chunks = extract_rtf("kb_path", path.to_str().unwrap(), "test")
            .await
            .unwrap();

        // 雖然 \pard 開頭也有 \par，但後接 'd' 所以不應該被替換
        // 預期會有 3 個段落
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].clean_content, "Hello, World!");
        assert_eq!(chunks[1].clean_content, "This is a test document.");
        assert_eq!(chunks[2].clean_content, "And this is the third paragraph.");
    }
}
