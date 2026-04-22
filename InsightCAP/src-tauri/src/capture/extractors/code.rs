use crate::error::AppError;
use regex::Regex;
use std::path::Path;

pub struct CodeChunk {
    pub language: String,
    pub clean_content: String,
}

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
    let content = crate::capture::encoding::read_text_file(file_path)
        .map_err(|e| AppError::Capture(format!("Failed to read code file: {}", e)))?;

    if content.len() < 2000 {
        return Ok(vec![CodeChunk {
            language: language.clone(),
            clean_content: format!("[Code File {} | {}]\n{}", filename, language, content),
        }]);
    }

    Ok(split_code_by_boundaries(
        &content, &ext, &language, filename,
    ))
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
        _ => r"(\n\n)",
    };
    let re = Regex::new(pattern).unwrap_or_else(|_| Regex::new(r"(\n\n)").unwrap());

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for line in content.lines() {
        if re.is_match(line) && !current_chunk.trim().is_empty() {
            chunks.push(CodeChunk {
                language: language.to_string(),
                clean_content: format!(
                    "[Code File {} | {}]\n{}",
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
                "[Code File {} | {}]\n{}",
                filename,
                language,
                current_chunk.trim()
            ),
        });
    }

    chunks
}
