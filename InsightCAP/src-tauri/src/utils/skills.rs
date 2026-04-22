use std::fs;
use std::path::PathBuf;

pub fn load_skill_prompt(skill_name: &str) -> String {
    let mut skill_path = PathBuf::from("skills").join(format!("{}.md", skill_name));

    if !skill_path.exists() {
        skill_path = PathBuf::from("src-tauri")
            .join("skills")
            .join(format!("{}.md", skill_name));
    }

    if !skill_path.exists() {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                skill_path = exe_dir.join("skills").join(format!("{}.md", skill_name));
            }
        }
    }

    match fs::read_to_string(&skill_path) {
        Ok(content) => extract_prompt_from_markdown(&content),
        Err(e) => {
            let cwd = std::env::current_dir().unwrap_or_default();
            eprintln!(
                "[SKILLS] Failed to load skill {}: {} (CWD: {:?}, Checked Path: {:?})",
                skill_name, e, cwd, skill_path
            );
            String::new()
        }
    }
}

fn extract_prompt_from_markdown(content: &str) -> String {
    let mut result = Vec::new();
    let mut in_code_block = false;

    for line in content.lines() {
        if line.trim().starts_with("```") {
            if !in_code_block {
                in_code_block = true;
                continue;
            }
            break;
        }

        if in_code_block {
            result.push(line);
        }
    }

    if result.is_empty() {
        return content.to_string();
    }

    result.join("\n").trim().to_string()
}
