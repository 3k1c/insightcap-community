use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::capture::file_parser::FileChunk;

const MAX_CHUNK_CHARS: usize = 2400;
const OVERLAP_CHARS: usize = 240;

// Table chunks flush before this many chars to leave room for header re-injection
const MAX_TABLE_CHUNK_CHARS: usize = MAX_CHUNK_CHARS - 200;

#[derive(Debug, Clone, Serialize)]
pub struct IngestChunk {
    pub content: String,
    pub content_type: String,
    pub knowledge_type: String,
    pub chunk_strategy: String,
    pub metadata_json: String,
}

pub fn chunks_for_file_chunk(file_chunk: &FileChunk) -> Vec<IngestChunk> {
    // Fix #6: skip indexing when OCR failed — content carries no semantic value
    if file_chunk.status == "ocr_failed" {
        return vec![];
    }

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

    // Fix #6: estimated_chars renamed to estimated_tokens with meaningful calculation
    let char_count = content.chars().count();
    let estimated_tokens = if content.is_ascii() {
        char_count / 4 // English: ~4 chars per token
    } else {
        char_count * 2 / 3 // CJK: ~1.5 chars per token
    };

    object.insert("source_type".to_string(), json!(file_chunk.source_type));
    object.insert(
        "source_chunk_type".to_string(),
        json!(file_chunk.chunk_type),
    );
    object.insert("char_count".to_string(), json!(char_count));
    object.insert("estimated_tokens".to_string(), json!(estimated_tokens));
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

// Fix #3: replaced corrupted CJK byte sequences with correct Unicode strings
fn looks_like_pattern(lower: &str) -> bool {
    let ascii_markers = [
        "must ",
        "should ",
        "always ",
        "never ",
        "rule",
        "principle",
        "best practice",
    ];
    let cjk_markers = [
        "必須", "應該", "規則", "原則", "最佳實踐", "禁止", "限制", "準則",
    ];
    ascii_markers.iter().any(|m| lower.contains(m))
        || cjk_markers.iter().any(|m| lower.contains(m))
}

fn has_headings(lines: &[&str]) -> bool {
    lines
        .iter()
        .take(80)
        .filter(|l| is_heading(l.trim()))
        .count()
        >= 2
}

// Fix #5: tightened heuristic to reduce false positives on short prose sentences
fn is_heading(line: &str) -> bool {
    if line.starts_with('#') {
        return true;
    }
    let char_count = line.chars().count();
    // Tightened upper bound (90 → 60) and added lower bound (> 3 → >= 5)
    // Exclude lines ending with '：' or ':' (label lines like "說明：")
    // Exclude lines that are purely numeric (page numbers, list indices)
    char_count >= 5
        && char_count <= 60
        && !line.ends_with('.')
        && !line.ends_with(',')
        && !line.ends_with('：')
        && !line.ends_with(':')
        && !line.contains('\t')
        && !line.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ' '))
}

// Fix #4: rewritten log event splitter — flush when the *next* event would overflow,
// not based on a fixed 600-char minimum for the current buffer.
fn split_log_events(content: &str) -> Vec<SplitPart> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in content.lines() {
        let starts_new_event = has_timestamp_prefix(line);
        let would_overflow = !current.is_empty()
            && current.chars().count() + line.len() + 1 > MAX_CHUNK_CHARS;

        if starts_new_event && !current.trim().is_empty() && would_overflow {
            if current.chars().count() > MAX_CHUNK_CHARS {
                chunks.extend(split_semantic_windows(&current));
            } else {
                chunks.push(SplitPart {
                    content: current.trim().to_string(),
                    has_overlap: false,
                });
            }
            current.clear();
        }
        current.push_str(line);
        current.push('\n');
    }

    if !current.trim().is_empty() {
        // Last batch may still be oversized if a single event is huge; delegate to windows
        if current.chars().count() > MAX_CHUNK_CHARS {
            chunks.extend(split_semantic_windows(&current));
        } else {
            chunks.push(SplitPart {
                content: current.trim().to_string(),
                has_overlap: false,
            });
        }
    }

    chunks
}

// Fix #2: table splitting now controlled by char count, not line count.
// Line count check kept only as a fast early-exit for small tables.
fn split_table_groups(content: &str) -> Vec<SplitPart> {
    let lines: Vec<&str> = content.lines().collect();

    // Fast path: small tables that definitely fit in one chunk
    if content.chars().count() <= MAX_TABLE_CHUNK_CHARS {
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
        // Flush when adding this row would exceed the char budget
        if current.chars().count() + line.len() + 1 > MAX_TABLE_CHUNK_CHARS {
            chunks.push(SplitPart {
                content: current.trim().to_string(),
                has_overlap: false,
            });
            current.clear();
            push_table_header(&mut current, &header, separator);
        }
        current.push_str(line);
        current.push('\n');
    }

    // Emit final chunk if it has rows beyond just the header (and optional separator)
    let header_lines = if separator.is_some() { 2 } else { 1 };
    if current.lines().count() > header_lines {
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

// Fix #1: added CJK sentence-ending punctuation alongside ASCII equivalents,
// and added CJK comma/pause marks as weak fallback before newline.
fn sentence_boundary(chars: &[char], start: usize, hard_end: usize) -> Option<usize> {
    let min = start + ((hard_end - start) * 70 / 100);

    // Priority 1: strong sentence-ending punctuation (ASCII + CJK)
    for idx in (min..hard_end).rev() {
        if matches!(chars[idx], '.' | '!' | '?' | ';' | '。' | '！' | '？' | '；') {
            return Some(idx + 1);
        }
    }

    // Priority 2: newline, then weak punctuation (commas)
    for idx in (min..hard_end).rev() {
        if matches!(chars[idx], '\n' | '，' | ',') {
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

    // Fix #1: new test — CJK sentence boundary must be respected
    #[test]
    fn sentence_boundary_respects_chinese_punctuation() {
        let sentence = "這是一個完整的中文句子，用來測試語意切分是否正確運作。";
        let content = sentence.repeat(120);
        let chunks = chunks_for_file_chunk(&file_chunk(content, "plain_text", "text"));

        assert!(chunks.len() > 1);
        // Each chunk must end at a CJK sentence boundary, not mid-character
        for chunk in &chunks {
            let last = chunk.content.chars().last().unwrap();
            assert!(
                matches!(last, '。' | '！' | '？' | '；' | '，') || chunk.content.ends_with('\n'),
                "chunk ended mid-sentence with: {:?}",
                last
            );
        }
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

    // Fix #2: new test — wide tables must not exceed MAX_CHUNK_CHARS even if row count is low
    #[test]
    fn wide_table_chunks_respect_char_limit() {
        // Each row is ~200 chars; 40 rows would be ~8000 chars (well over 2400)
        let mut content = String::from("| Col1 | Col2 | Col3 | Col4 | Col5 |\n|---|---|---|---|---|\n");
        for i in 0..40 {
            content.push_str(&format!(
                "| {:>30} | {:>30} | {:>30} | {:>30} | {:>30} |\n",
                i, i * 2, i * 3, i * 4, i * 5
            ));
        }

        let chunks = chunks_for_file_chunk(&file_chunk(content, "xlsx", "document"));

        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(
                chunk.content.chars().count() <= MAX_CHUNK_CHARS,
                "chunk exceeded MAX_CHUNK_CHARS: {} chars",
                chunk.content.chars().count()
            );
        }
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

    // Fix #4: new test — log events must not be merged across timestamp boundaries when oversized
    #[test]
    fn log_events_split_at_timestamp_boundaries_when_oversized() {
        let mut content = String::new();
        for i in 0..30 {
            content.push_str(&format!(
                "2026-04-27 10:{:02}:00 [info] event_{}\n{}\n",
                i,
                i,
                "payload data ".repeat(20) // ~260 chars per event
            ));
        }

        let chunks = chunks_for_file_chunk(&file_chunk(content, "log", "text"));

        // All chunks must respect the char limit
        for chunk in &chunks {
            assert!(
                chunk.content.chars().count() <= MAX_CHUNK_CHARS,
                "log chunk exceeded limit: {} chars",
                chunk.content.chars().count()
            );
        }
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

    // Fix #6: new test — estimated_tokens replaces estimated_chars and has a meaningful value
    #[test]
    fn metadata_includes_estimated_tokens_not_estimated_chars() {
        let chunks = chunks_for_file_chunk(&file_chunk(
            "Revenue details about Q3 performance.".to_string(),
            "pdf",
            "document",
        ));
        let data = metadata(&chunks[0]);

        assert!(data.get("estimated_tokens").is_some(), "estimated_tokens missing");
        assert!(data.get("estimated_chars").is_none(), "estimated_chars should be removed");
        assert!(data["estimated_tokens"].as_u64().unwrap() > 0);
    }

    // Fix #6: new test — ocr_failed chunks are not indexed
    #[test]
    fn ocr_failed_chunks_are_skipped() {
        let file_chunk = FileChunk {
            content: "[OCR failed] image.png".to_string(),
            chunk_type: "image".to_string(),
            source_type: "image".to_string(),
            metadata: json!({ "source_type": "image" }),
            image_path: Some("/path/to/image.png".to_string()),
            status: "ocr_failed".to_string(),
        };

        let chunks = chunks_for_file_chunk(&file_chunk);
        assert!(chunks.is_empty(), "ocr_failed chunks should not be indexed");
    }

    #[test]
    fn stress_test_extreme_inputs() {
        // Case 1: Extremely long single line (100k chars)
        let long_line = "a".repeat(100_000);
        let chunks = chunks_for_file_chunk(&file_chunk(long_line, "plain_text", "text"));
        assert!(!chunks.is_empty());
        for c in &chunks {
            assert!(c.content.chars().count() <= MAX_CHUNK_CHARS + 100); // Allow some buffer for overlap
        }

        // Case 2: Massive log file (10k events)
        let mut log_content = String::new();
        for i in 0..10_000 {
            log_content.push_str(&format!("2026-04-27 12:00:00 [INFO] event {}\n", i));
        }
        let chunks = chunks_for_file_chunk(&file_chunk(log_content, "log", "text"));
        assert!(!chunks.is_empty());

        // Case 3: Massive wide table
        let mut table_content = String::from("| Head |\n|---|\n");
        for i in 0..5_000 {
            table_content.push_str(&format!("| Row {} with very long content {} |\n", i, "x".repeat(100)));
        }
        let chunks = chunks_for_file_chunk(&file_chunk(table_content, "xlsx", "document"));
        assert!(!chunks.is_empty());

        // Case 4: Pure whitespace and special characters
        let weird_content = " \n\t\u{200b}\u{feff}".repeat(1000);
        let chunks = chunks_for_file_chunk(&file_chunk(weird_content, "plain_text", "text"));
        // Should be empty after clean_text
        assert!(chunks.is_empty());

        // Case 5: Deep/Massive Markdown headers
        let mut md_content = String::new();
        for i in 0..500 {
            md_content.push_str(&format!("{} Header {}\nContent for {}\n\n", "#".repeat((i % 6) + 1), i, i));
        }
        let chunks = chunks_for_file_chunk(&file_chunk(md_content, "markdown", "text"));
        assert!(!chunks.is_empty());
    }
}
