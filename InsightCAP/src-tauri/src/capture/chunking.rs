use serde::Serialize;

use crate::capture::file_parser::FileChunk;

const MAX_CHUNK_CHARS: usize = 2400;
const OVERLAP_CHARS: usize = 240;

#[derive(Debug, Clone, Serialize)]
pub struct IngestChunk {
    pub content: String,
    pub content_type: String,
    pub knowledge_type: String,
    pub chunk_strategy: String,
    pub metadata_json: String,
}

#[derive(Debug, Serialize)]
struct ChunkMetadata {
    source_chunk_type: String,
    estimated_chars: usize,
}

pub fn chunks_for_file_chunk(file_chunk: &FileChunk) -> Vec<IngestChunk> {
    let content = clean_text(&file_chunk.content);
    if content.is_empty() {
        return vec![];
    }

    let profile = profile_content(&content, &file_chunk.chunk_type);
    let parts = match profile.chunk_strategy.as_str() {
        "log_event" => split_log_events(&content),
        "table" => split_table_groups(&content),
        "heading" => split_by_heading(&content),
        _ => split_semantic_windows(&content),
    };

    parts
        .into_iter()
        .filter_map(|part| {
            let clean = clean_text(&part);
            if clean.is_empty() {
                return None;
            }

            let metadata = ChunkMetadata {
                source_chunk_type: file_chunk.chunk_type.clone(),
                estimated_chars: clean.chars().count(),
            };

            Some(IngestChunk {
                content: clean,
                content_type: profile.content_type.clone(),
                knowledge_type: profile.knowledge_type.clone(),
                chunk_strategy: profile.chunk_strategy.clone(),
                metadata_json: serde_json::to_string(&metadata)
                    .unwrap_or_else(|_| "{}".to_string()),
            })
        })
        .collect()
}

pub fn chunks_for_text(content: &str, source_type: &str) -> Vec<IngestChunk> {
    let file_chunk = FileChunk {
        content: content.to_string(),
        chunk_type: source_type.to_string(),
        image_path: None,
        status: "processed".to_string(),
    };
    chunks_for_file_chunk(&file_chunk)
}

struct ContentProfile {
    content_type: String,
    knowledge_type: String,
    chunk_strategy: String,
}

fn profile_content(content: &str, chunk_type: &str) -> ContentProfile {
    let lower = content.to_lowercase();
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

    if chunk_type == "image" {
        return ContentProfile {
            content_type: "image_ocr".to_string(),
            knowledge_type: "data".to_string(),
            chunk_strategy: "semantic".to_string(),
        };
    }

    if looks_like_log(&lines, &lower) {
        return ContentProfile {
            content_type: "log".to_string(),
            knowledge_type: "log".to_string(),
            chunk_strategy: "log_event".to_string(),
        };
    }

    if looks_like_table(&lines) {
        return ContentProfile {
            content_type: "table".to_string(),
            knowledge_type: "data".to_string(),
            chunk_strategy: "table".to_string(),
        };
    }

    if looks_like_code(&lines, &lower) {
        return ContentProfile {
            content_type: "code".to_string(),
            knowledge_type: "data".to_string(),
            chunk_strategy: "semantic".to_string(),
        };
    }

    if looks_like_pattern(&lower) {
        return ContentProfile {
            content_type: "prose".to_string(),
            knowledge_type: "pattern".to_string(),
            chunk_strategy: "heading".to_string(),
        };
    }

    let chunk_strategy = if has_headings(&lines) {
        "heading"
    } else {
        "semantic"
    };

    ContentProfile {
        content_type: "prose".to_string(),
        knowledge_type: "data".to_string(),
        chunk_strategy: chunk_strategy.to_string(),
    }
}

fn clean_text(input: &str) -> String {
    let normalized = input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{00a0}', " ");

    let mut out = String::new();
    let mut blank_count = 0usize;

    for line in normalized.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                out.push('\n');
            }
        } else {
            blank_count = 0;
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    out.trim().to_string()
}

fn looks_like_log(lines: &[&str], lower: &str) -> bool {
    let mut hits = 0usize;
    for line in lines.iter().take(80) {
        let l = line.trim();
        if l.contains("[error]")
            || l.contains("[warn]")
            || l.contains("[info]")
            || l.contains(" error ")
            || l.contains(" warn ")
            || l.starts_with("error")
            || l.starts_with("warn")
            || l.contains("stack trace")
            || l.contains("trace_id")
            || has_timestamp_prefix(l)
        {
            hits += 1;
        }
    }

    hits >= 3 || lower.contains("stack trace") || lower.contains("traceback")
}

fn has_timestamp_prefix(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.len() < 10 {
        return false;
    }
    bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes[0..4].iter().all(u8::is_ascii_digit)
}

fn looks_like_table(lines: &[&str]) -> bool {
    let sample = lines.iter().take(25).collect::<Vec<_>>();
    if sample.len() < 3 {
        return false;
    }

    let pipe_rows = sample
        .iter()
        .filter(|l| l.matches('|').count() >= 2)
        .count();
    let comma_rows = sample
        .iter()
        .filter(|l| l.matches(',').count() >= 3)
        .count();
    let tab_rows = sample
        .iter()
        .filter(|l| l.matches('\t').count() >= 2)
        .count();

    pipe_rows >= 3 || comma_rows >= 3 || tab_rows >= 3
}

fn looks_like_code(lines: &[&str], lower: &str) -> bool {
    let code_markers = [
        "function ",
        "const ",
        "let ",
        "class ",
        "impl ",
        "fn ",
        "def ",
        "import ",
        "export ",
        "public ",
        "private ",
        "=>",
        "::",
        "{",
        "}",
    ];
    let marker_hits = code_markers.iter().filter(|m| lower.contains(*m)).count();
    let indented = lines
        .iter()
        .filter(|l| l.starts_with("    ") || l.starts_with('\t'))
        .count();
    marker_hits >= 3 || (marker_hits >= 1 && indented >= 4)
}

fn looks_like_pattern(lower: &str) -> bool {
    let markers = [
        "must ",
        "should ",
        "always ",
        "never ",
        "rule",
        "principle",
        "best practice",
        "標準做法",
        "必須",
        "不要",
        "規則",
        "原則",
    ];
    markers.iter().any(|m| lower.contains(m))
}

fn has_headings(lines: &[&str]) -> bool {
    lines
        .iter()
        .take(80)
        .filter(|l| is_heading(l.trim()))
        .count()
        >= 2
}

fn is_heading(line: &str) -> bool {
    if line.starts_with('#') {
        return true;
    }
    let char_count = line.chars().count();
    char_count > 3
        && char_count <= 90
        && !line.ends_with('.')
        && !line.ends_with(',')
        && !line.contains('\t')
}

fn split_log_events(content: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in content.lines() {
        let starts_event = has_timestamp_prefix(line) && !current.trim().is_empty();
        if starts_event && current.chars().count() >= 600 {
            chunks.extend(split_semantic_windows(&current));
            current.clear();
        }
        current.push_str(line);
        current.push('\n');
    }

    if !current.trim().is_empty() {
        chunks.extend(split_semantic_windows(&current));
    }

    chunks
}

fn split_table_groups(content: &str) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= 40 {
        return vec![content.to_string()];
    }

    let header = lines.first().copied().unwrap_or("");
    let mut chunks = Vec::new();
    let mut current = String::new();
    current.push_str(header);
    current.push('\n');

    for line in lines.iter().skip(1) {
        current.push_str(line);
        current.push('\n');
        if current.lines().count() >= 40 {
            chunks.push(current.trim().to_string());
            current.clear();
            current.push_str(header);
            current.push('\n');
        }
    }

    if current.lines().count() > 1 {
        chunks.push(current.trim().to_string());
    }

    chunks
}

fn split_by_heading(content: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut current = String::new();

    for line in content.lines() {
        if is_heading(line.trim()) && current.chars().count() >= 500 {
            sections.extend(split_semantic_windows(&current));
            current.clear();
        }
        current.push_str(line);
        current.push('\n');
    }

    if !current.trim().is_empty() {
        sections.extend(split_semantic_windows(&current));
    }

    sections
}

fn split_semantic_windows(content: &str) -> Vec<String> {
    let paragraphs: Vec<String> = content
        .split("\n\n")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if paragraphs.is_empty() {
        return split_by_chars(content);
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for paragraph in paragraphs {
        let next_len = current.chars().count() + paragraph.chars().count() + 2;
        if !current.is_empty() && next_len > MAX_CHUNK_CHARS {
            chunks.push(current.trim().to_string());
            current = overlap_tail(&current);
        }
        current.push_str(&paragraph);
        current.push_str("\n\n");
    }

    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }

    chunks
}

fn split_by_chars(content: &str) -> Vec<String> {
    let chars: Vec<char> = content.chars().collect();
    if chars.len() <= MAX_CHUNK_CHARS {
        return vec![content.trim().to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + MAX_CHUNK_CHARS).min(chars.len());
        chunks.push(
            chars[start..end]
                .iter()
                .collect::<String>()
                .trim()
                .to_string(),
        );
        if end == chars.len() {
            break;
        }
        start = end.saturating_sub(OVERLAP_CHARS);
    }
    chunks
}

fn overlap_tail(content: &str) -> String {
    let chars: Vec<char> = content.chars().collect();
    if chars.len() <= OVERLAP_CHARS {
        return content.to_string();
    }
    chars[chars.len() - OVERLAP_CHARS..]
        .iter()
        .collect::<String>()
}
