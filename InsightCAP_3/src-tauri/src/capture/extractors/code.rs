//! # 程式碼類提取器
//!
//! 支援多種程式語言副檔名，實作按函數/類別邊界分割大文件。
//! 見 Phase3.1.md P3.1-09。

use crate::error::AppError;
use regex::Regex;
use std::fs;
use std::path::Path;

pub struct CodeChunk {
    pub language: String,
    pub clean_content: String,
}

/// 提取程式碼內容
pub async fn extract_code(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
) -> Result<Vec<CodeChunk>, AppError> {
    let path = Path::new(file_path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown");

    let language = map_extension_to_language(&ext);
    let content = fs::read_to_string(file_path)
        .map_err(|e| AppError::Capture(format!("程式碼讀取失敗: {}", e)))?;

    // 門檻值：約 512 token (以 2000 字元估計)
    if content.len() < 2000 {
        return Ok(vec![CodeChunk {
            language: language.clone(),
            clean_content: format!("[程式碼: {} | {}]\n{}", filename, language, content),
        }]);
    }

    // 按語言特性初步分割
    let chunks = split_code_by_boundaries(&content, &ext, &language, filename);
    Ok(chunks)
}

fn map_extension_to_language(ext: &str) -> String {
    match ext {
        "py" => "Python".to_string(),
        "js" | "jsx" => "JavaScript".to_string(),
        "ts" | "tsx" => "TypeScript".to_string(),
        "rs" => "Rust".to_string(),
        "go" => "Go".to_string(),
        "java" => "Java".to_string(),
        "cpp" | "cc" | "cxx" | "h" | "hpp" => "C/C++".to_string(),
        "c" => "C".to_string(),
        "swift" => "Swift".to_string(),
        "rb" => "Ruby".to_string(),
        "php" => "PHP".to_string(),
        _ => "Plain Text".to_string(),
    }
}

fn split_code_by_boundaries(
    content: &str,
    ext: &str,
    language: &str,
    filename: &str,
) -> Vec<CodeChunk> {
    let pattern = match ext {
        "py" => r"^(def\s+|class\s+)",
        "js" | "ts" | "jsx" | "tsx" => {
            r"^(function\s+|class\s+|const\s+[\w\d_]+\s*=\s*(async\s+)?(\(.*\)|[\w\d_]+)\s*=>)"
        }
        "rs" => r"^(fn\s+|impl\s+|struct\s+|enum\s+|trait\s+)",
        "swift" => r"^(func\s+|class\s+|struct\s+|enum\s+|extension\s+)",
        "java" | "cpp" | "c" | "php" => {
            r"^(class\s+|public\s+|private\s+|protected\s+|static\s+|void\s+|[\w\d_]+\s+[\w\d_]+\s*\(.*\)\s*\{)"
        }
        _ => r"(\n\n)", // 預設按空行分割
    };

    let re = Regex::new(pattern).unwrap_or_else(|_| Regex::new(r"(\n\n)").unwrap());

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for line in content.lines() {
        // 如果偵測到新邊界且當前緩存已有內容，則切分
        if re.is_match(line) && !current_chunk.trim().is_empty() {
            chunks.push(CodeChunk {
                language: language.to_string(),
                clean_content: format!(
                    "[程式碼: {} | {}]\n{}",
                    filename,
                    language,
                    current_chunk.trim()
                ),
            });
            current_chunk = String::new();
        }
        current_chunk.push_str(line);
        current_chunk.push('\n');
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(CodeChunk {
            language: language.to_string(),
            clean_content: format!(
                "[程式碼: {} | {}]\n{}",
                filename,
                language,
                current_chunk.trim()
            ),
        });
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_extract_short_code_file() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("main.rs");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "fn main() {{\n    println!(\"Hello World\");\n}}").unwrap();

        let chunks = extract_code("kb_path", path.to_str().unwrap(), "main").await.unwrap();
        
        // Small file < 2000 chars should produce exactly 1 chunk
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].language, "Rust");
        assert!(chunks[0].clean_content.contains("[程式碼: main.rs | Rust]"));
        assert!(chunks[0].clean_content.contains("fn main()"));
    }

    #[tokio::test]
    async fn test_extract_large_code_file_splits_by_boundary() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("app.js");
        let mut file = File::create(&path).unwrap();
        
        // Make sure it exceeds 2000 chars to trigger boundary splitting
        let padding = "/* ".to_string() + &"padding ".repeat(300) + "*/\n";
        
        // Write content with identifiable boundaries
        writeln!(file, "{}", padding).unwrap();
        writeln!(file, "function firstFunc() {{\n    console.log(1);\n}}\n").unwrap();
        writeln!(file, "class MyClass {{\n    constructor() {{}}\n}}\n").unwrap();
        writeln!(file, "const arrowFunc = async () => {{\n    return 1;\n}}\n").unwrap();

        let chunks = extract_code("kb_path", path.to_str().unwrap(), "app").await.unwrap();
        
        // At least 3 chunks (might be 4 if padding ends up in its own initial chunk)
        assert!(chunks.len() >= 3);
        
        // All chunks should be JavaScript
        for chunk in &chunks {
            assert_eq!(chunk.language, "JavaScript");
            assert!(chunk.clean_content.contains("[程式碼: app.js | JavaScript]"));
        }
        
        // Check if our specific functions were extracted
        let content_concat = chunks.iter().map(|c| c.clean_content.as_str()).collect::<Vec<_>>().join("\n---chunk---\n");
        assert!(content_concat.contains("function firstFunc()"));
        assert!(content_concat.contains("class MyClass"));
        assert!(content_concat.contains("const arrowFunc = async () =>"));
    }
}
