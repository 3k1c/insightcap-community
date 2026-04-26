use serde_json::{json, Value};

pub fn normalize_text_basics(input: &str) -> String {
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{feff}', "")
        .replace('\u{200b}', "")
        .replace('\u{200c}', "")
        .replace('\u{200d}', "")
        .replace('\u{00a0}', " ")
}

pub fn collapse_blank_lines(input: &str) -> String {
    let mut out = String::new();
    let mut blank_count = 0usize;

    for line in input.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                out.push('\n');
            }
        } else {
            blank_count = 0;
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }

    out.trim().to_string()
}

pub fn strip_noise_lines(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut start = 0usize;
    let mut end = lines.len();

    while start < end && is_noise_line(lines[start]) {
        start += 1;
    }
    while end > start && is_noise_line(lines[end - 1]) {
        end -= 1;
    }

    lines[start..end].join("\n")
}

pub fn normalize_extracted_markdown(input: &str) -> String {
    collapse_blank_lines(&strip_noise_lines(&normalize_text_basics(input)))
}

pub fn format_table_as_markdown(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let header = markdown_row(&rows[0]);
    let separator = markdown_row(&vec!["---".to_string(); rows[0].len()]);
    let body = rows
        .iter()
        .skip(1)
        .map(|row| markdown_row(row))
        .collect::<Vec<_>>();

    std::iter::once(header)
        .chain(std::iter::once(separator))
        .chain(body)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn extract_heading_context(content: &str) -> Option<String> {
    content
        .lines()
        .find_map(|line| line.trim().strip_prefix('#').map(str::trim))
        .filter(|heading| !heading.is_empty())
        .map(ToString::to_string)
}

pub fn base_metadata(source_type: &str) -> Value {
    json!({
        "source_type": source_type,
    })
}

fn markdown_row(cells: &[String]) -> String {
    format!(
        "| {} |",
        cells
            .iter()
            .map(|cell| cell.replace('|', "\\|").trim().to_string())
            .collect::<Vec<_>>()
            .join(" | ")
    )
}

fn is_noise_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    let repeated = trimmed
        .chars()
        .all(|c| matches!(c, '-' | '=' | '*' | '_' | '─'));
    repeated && trimmed.chars().count() >= 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_markdown_blocks_while_cleaning_noise() {
        let input = "\u{feff}---\r\n# Title\r\n\r\n```rust\r\nfn main() {\r\n    println!(\"ok\");\r\n}\r\n```\r\n\r\n===\r\n";
        let cleaned = normalize_extracted_markdown(input);

        assert!(cleaned.starts_with("# Title"));
        assert!(cleaned.contains("```rust\nfn main() {\n    println!"));
        assert!(!cleaned.contains('\u{feff}'));
        assert!(!cleaned.ends_with("==="));
    }

    #[test]
    fn formats_rows_as_markdown_table() {
        let rows = vec![
            vec!["Region".to_string(), "Revenue".to_string()],
            vec!["North".to_string(), "100".to_string()],
        ];
        let table = format_table_as_markdown(&rows);

        assert_eq!(
            table,
            "| Region | Revenue |\n| --- | --- |\n| North | 100 |"
        );
    }
}
