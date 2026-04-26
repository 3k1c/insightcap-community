use serde::Serialize;
use serde_json::{json, Map, Value};

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

pub fn chunks_for_file_chunk(file_chunk: &FileChunk) -> Vec<IngestChunk> {
    let content = clean_text(&file_chunk.content, file_chunk.source_type == "code");
    if content.is_empty() {
        return vec![];
    }

    let profile = profile_content(&content, file_chunk);
    let parts = match profile.chunk_strategy.as_str() {
        "log_event" => split_log_events(&content),
        "table_serialization" => split_table_groups(&content),
        "markdown_structure" | "structure_aware" => split_by_heading(&content),
        "slide_atomic" => split_slide_atomic(&content),
        _ => split_semantic_windows(&content),
    };
    let split_count = parts.len();

    parts
        .into_iter()
        .enumerate()
        .filter_map(|(split_index, part)| {
            let clean = clean_text(&part.content, file_chunk.source_type == "code");
            if clean.is_empty() {
                return None;
            }

            let metadata = build_metadata(
                file_chunk,
                &clean,
                split_index,
                split_count,
                part.has_overlap,
            );

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
        source_type: source_type.to_string(),
        metadata: json!({
            "source_type": source_type,
        }),
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

#[derive(Debug, Clone)]
struct SplitPart {
    content: String,
    has_overlap: bool,
}

fn profile_content(content: &str, file_chunk: &FileChunk) -> ContentProfile {
    let lower = content.to_lowercase();
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    let source_type = file_chunk.source_type.as_str();

    if source_type == "image" || file_chunk.chunk_type == "image" {
        return ContentProfile {
            content_type: "image_ocr".to_string(),
            knowledge_type: "data".to_string(),
            chunk_strategy: "semantic_fallback".to_string(),
        };
    }

    if source_type == "log" || looks_like_log(&lines, &lower) {
        return ContentProfile {
            content_type: "log".to_string(),
            knowledge_type: "log".to_string(),
            chunk_strategy: "log_event".to_string(),
        };
    }

    if matches!(source_type, "csv" | "xlsx") || looks_like_table(&lines) {
        return ContentProfile {
            content_type: "table".to_string(),
            knowledge_type: "data".to_string(),
            chunk_strategy: "table_serialization".to_string(),
        };
    }

    if source_type == "pptx" {
        return ContentProfile {
            content_type: "slide".to_string(),
            knowledge_type: "data".to_string(),
            chunk_strategy: "slide_atomic".to_string(),
        };
    }

    if source_type == "markdown" {
        return ContentProfile {
            content_type: "prose".to_string(),
            knowledge_type: "data".to_string(),
            chunk_strategy: "markdown_structure".to_string(),
        };
    }

    if source_type == "code" || looks_like_code(&lines, &lower) {
        return ContentProfile {
            content_type: "code".to_string(),
            knowledge_type: "data".to_string(),
            chunk_strategy: "syntax_aware".to_string(),
        };
    }

    if source_type == "plain_text" {
        return ContentProfile {
            content_type: "prose".to_string(),
            knowledge_type: "data".to_string(),
            chunk_strategy: "semantic_fallback".to_string(),
        };
    }

    if looks_like_pattern(&lower) {
        return ContentProfile {
            content_type: "prose".to_string(),
            knowledge_type: "pattern".to_string(),
            chunk_strategy: "structure_aware".to_string(),
        };
    }

    let chunk_strategy = if matches!(source_type, "pdf" | "docx" | "rtf" | "epub" | "html")
        || has_headings(&lines)
    {
        "structure_aware"
    } else {
        "semantic_fallback"
    };

    ContentProfile {
        content_type: "prose".to_string(),
        knowledge_type: "data".to_string(),
        chunk_strategy: chunk_strategy.to_string(),
    }
}

fn clean_text(input: &str, preserve_indent: bool) -> String {
    let normalized = input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{feff}', "")
        .replace('\u{200b}', "")
        .replace('\u{200c}', "")
        .replace('\u{200d}', "")
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
            if preserve_indent {
                out.push_str(trimmed);
            } else {
                out.push_str(trimmed.trim_start());
            }
            out.push('\n');
        }
    }

    out.trim().to_string()
}

fn build_metadata(
    file_chunk: &FileChunk,
    content: &str,
    split_index: usize,
    split_count: usize,
    has_overlap: bool,
) -> Value {
    let mut object = match file_chunk.metadata.clone() {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    let (log_start_time, log_end_time) = extract_log_range(content);

    object.insert("source_type".to_string(), json!(file_chunk.source_type));
    object.insert(
        "source_chunk_type".to_string(),
        json!(file_chunk.chunk_type),
    );
    object.insert("char_count".to_string(), json!(content.chars().count()));
    object.insert(
        "estimated_chars".to_string(),
        json!(content.chars().count()),
    );
    object.insert("line_count".to_string(), json!(content.lines().count()));
    object.insert("strategy_version".to_string(), json!("chunk-preclean-v1"));
    object.insert("split_index".to_string(), json!(split_index));
    object.insert("split_count".to_string(), json!(split_count));
    object.insert("has_overlap".to_string(), json!(has_overlap));

    if let Some(heading) = first_markdown_heading(content) {
        object.insert("heading".to_string(), json!(heading));
    }
    if let Some(header) = table_header(content) {
        object.insert("table_header".to_string(), json!(header));
    }
    if let Some(start) = log_start_time {
        object.insert("log_start_time".to_string(), json!(start));
    }
    if let Some(end) = log_end_time {
        object.insert("log_end_time".to_string(), json!(end));
    }

    Value::Object(object)
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
        "璅???",
        "敹?",
        "銝?",
        "閬?",
        "??",
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

fn split_log_events(content: &str) -> Vec<SplitPart> {
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

fn split_table_groups(content: &str) -> Vec<SplitPart> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= 40 {
        return vec![SplitPart {
            content: content.to_string(),
            has_overlap: false,
        }];
    }

    let header =
        table_header(content).unwrap_or_else(|| lines.first().copied().unwrap_or("").to_string());
    let separator = lines
        .get(1)
        .filter(|line| is_markdown_separator(line))
        .copied();
    let mut chunks = Vec::new();
    let mut current = String::new();
    push_table_header(&mut current, &header, separator);

    let body_start = if separator.is_some() { 2 } else { 1 };
    for line in lines.iter().skip(body_start) {
        current.push_str(line);
        current.push('\n');
        if current.lines().count() >= 40 {
            chunks.push(SplitPart {
                content: current.trim().to_string(),
                has_overlap: false,
            });
            current.clear();
            push_table_header(&mut current, &header, separator);
        }
    }

    if current.lines().count() > if separator.is_some() { 2 } else { 1 } {
        chunks.push(SplitPart {
            content: current.trim().to_string(),
            has_overlap: false,
        });
    }

    chunks
}

fn split_slide_atomic(content: &str) -> Vec<SplitPart> {
    if content.chars().count() <= MAX_CHUNK_CHARS {
        return vec![SplitPart {
            content: content.to_string(),
            has_overlap: false,
        }];
    }
    split_semantic_windows(content)
}

fn split_by_heading(content: &str) -> Vec<SplitPart> {
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

fn split_semantic_windows(content: &str) -> Vec<SplitPart> {
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
        let p_len = paragraph.chars().count();

        // If single paragraph is too large, split it first
        if p_len > MAX_CHUNK_CHARS {
            // Flush current buffer if not empty
            if !current.is_empty() {
                chunks.push(SplitPart {
                    content: current.trim().to_string(),
                    has_overlap: false,
                });
                current.clear();
            }

            // Hard split the huge paragraph
            let mut sub_chunks = split_by_chars(&paragraph);
            if let Some(last) = sub_chunks.pop() {
                chunks.extend(sub_chunks);
                current = last.content;
                current.push_str("\n\n");
            }
            continue;
        }

        let next_len = current.chars().count() + p_len + 2;
        if !current.is_empty() && next_len > MAX_CHUNK_CHARS {
            chunks.push(SplitPart {
                content: current.trim().to_string(),
                has_overlap: false,
            });
            current = overlap_tail(&current);
        }
        current.push_str(&paragraph);
        current.push_str("\n\n");
    }

    if !current.trim().is_empty() {
        let has_overlap = !chunks.is_empty();
        chunks.push(SplitPart {
            content: current.trim().to_string(),
            has_overlap,
        });
    }

    chunks
}

fn split_by_chars(content: &str) -> Vec<SplitPart> {
    let chars: Vec<char> = content.chars().collect();
    if chars.len() <= MAX_CHUNK_CHARS {
        return vec![SplitPart {
            content: content.trim().to_string(),
            has_overlap: false,
        }];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let hard_end = (start + MAX_CHUNK_CHARS).min(chars.len());
        let end = if hard_end == chars.len() {
            hard_end
        } else {
            sentence_boundary(&chars, start, hard_end).unwrap_or(hard_end)
        };
        chunks.push(SplitPart {
            content: chars[start..end]
                .iter()
                .collect::<String>()
                .trim()
                .to_string(),
            has_overlap: start > 0,
        });
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

fn sentence_boundary(chars: &[char], start: usize, hard_end: usize) -> Option<usize> {
    let min = start + ((hard_end - start) * 70 / 100);
    for idx in (min..hard_end).rev() {
        if matches!(chars[idx], '.' | '!' | '?' | ';') {
            return Some(idx + 1);
        }
    }
    for idx in (min..hard_end).rev() {
        if chars[idx] == '\n' {
            return Some(idx + 1);
        }
    }
    None
}

fn first_markdown_heading(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix('#')
            .map(str::trim)
            .filter(|heading| !heading.is_empty())
            .map(ToString::to_string)
    })
}

fn table_header(content: &str) -> Option<String> {
    content
        .lines()
        .find(|line| line.matches('|').count() >= 2 && !is_markdown_separator(line))
        .map(|line| line.trim().to_string())
}

fn is_markdown_separator(line: &str) -> bool {
    let trimmed = line.trim().trim_matches('|').trim();
    !trimmed.is_empty() && trimmed.chars().all(|c| matches!(c, '-' | ':' | '|' | ' '))
}

fn push_table_header(current: &mut String, header: &str, separator: Option<&str>) {
    current.push_str(header);
    current.push('\n');
    if let Some(separator) = separator {
        current.push_str(separator);
        current.push('\n');
    }
}

fn extract_log_range(content: &str) -> (Option<String>, Option<String>) {
    let timestamps = content
        .lines()
        .filter_map(extract_timestamp_prefix)
        .collect::<Vec<_>>();

    (timestamps.first().cloned(), timestamps.last().cloned())
}

fn extract_timestamp_prefix(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !has_timestamp_prefix(trimmed) {
        return None;
    }
    Some(trimmed.chars().take(19).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn file_chunk(content: String, source_type: &str, chunk_type: &str) -> FileChunk {
        FileChunk {
            content,
            chunk_type: chunk_type.to_string(),
            source_type: source_type.to_string(),
            metadata: json!({
                "heading": "Revenue",
                "page_number": 3,
                "sheet_name": "Q3",
                "slide_number": 2,
            }),
            image_path: None,
            status: "processed".to_string(),
        }
    }

    fn metadata(chunk: &IngestChunk) -> Value {
        serde_json::from_str(&chunk.metadata_json).unwrap()
    }

    #[test]
    fn sentence_boundary_split_does_not_cut_inside_sentence() {
        let sentence =
            "This sentence should stay intact because chunking must prefer semantic stops. ";
        let content = sentence.repeat(80);
        let chunks = chunks_for_file_chunk(&file_chunk(content, "plain_text", "text"));

        assert!(chunks.len() > 1);
        assert!(chunks[0].content.ends_with('.'));
        assert_eq!(chunks[0].chunk_strategy, "semantic_fallback");
    }

    #[test]
    fn markdown_heading_stays_with_following_body() {
        let content = "# Revenue\n\nRevenue increased because renewal expansion improved.\n\n# Cost\n\nCosts stayed flat.";
        let chunks = chunks_for_file_chunk(&file_chunk(content.to_string(), "markdown", "text"));

        assert_eq!(chunks[0].chunk_strategy, "markdown_structure");
        assert!(chunks[0].content.contains("# Revenue"));
        assert!(chunks[0].content.contains("Revenue increased"));
        assert!(metadata(&chunks[0]).get("heading").is_some());
    }

    #[test]
    fn large_table_chunks_keep_header() {
        let mut content = String::from("| Region | Product | Revenue |\n|---|---|---|\n");
        for i in 0..120 {
            content.push_str(&format!("| North | SKU{} | {} |\n", i, i * 10));
        }

        let chunks = chunks_for_file_chunk(&file_chunk(content, "csv", "document"));

        assert!(chunks.len() > 1);
        assert!(chunks
            .iter()
            .all(|chunk| chunk.content.starts_with("| Region | Product | Revenue |")));
        assert_eq!(chunks[0].chunk_strategy, "table_serialization");
    }

    #[test]
    fn log_metadata_includes_start_and_end_timestamp() {
        let content = "2026-04-27 10:00:00 [info] started\nline one\n2026-04-27 10:01:00 [error] failed\nline two";
        let chunks = chunks_for_file_chunk(&file_chunk(content.to_string(), "log", "text"));
        let data = metadata(&chunks[0]);

        assert_eq!(chunks[0].chunk_strategy, "log_event");
        assert_eq!(data["log_start_time"], "2026-04-27 10:00:00");
        assert_eq!(data["log_end_time"], "2026-04-27 10:01:00");
    }

    #[test]
    fn code_chunk_preserves_prefix_and_indentation() {
        let content =
            "# Language: Rust | File: src/lib.rs\n\nfn main() {\n    println!(\"ok\");\n}";
        let chunks = chunks_for_file_chunk(&file_chunk(content.to_string(), "code", "document"));

        assert_eq!(chunks[0].chunk_strategy, "syntax_aware");
        assert!(chunks[0].content.contains("# Language: Rust"));
        assert!(chunks[0].content.contains("    println!"));
    }

    #[test]
    fn extractor_metadata_is_merged_into_chunk_metadata() {
        let chunks = chunks_for_file_chunk(&file_chunk(
            "Revenue details.".to_string(),
            "pdf",
            "document",
        ));
        let data = metadata(&chunks[0]);

        assert_eq!(data["source_type"], "pdf");
        assert_eq!(data["page_number"], 3);
        assert_eq!(data["sheet_name"], "Q3");
        assert_eq!(data["slide_number"], 2);
        assert!(data["char_count"].as_u64().unwrap() > 0);
        assert!(data["line_count"].as_u64().unwrap() > 0);
    }
}
