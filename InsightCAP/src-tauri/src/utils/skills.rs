use std::fs;
use std::path::PathBuf;

/// 從 skills 目錄中的 Markdown 文件加載 Prompt。
/// 邏輯：讀取文件，並提取第一個三反引號 (```) 包夾的區塊。
/// 如果沒有找到區塊，則返回整個文件內容作為 fallback。
pub fn load_skill_prompt(skill_name: &str) -> String {
    let mut skill_path = PathBuf::from("skills").join(format!("{}.md", skill_name));

    if !skill_path.exists() {
        skill_path = PathBuf::from("src-tauri")
            .join("skills")
            .join(format!("{}.md", skill_name));
    }

    // 如果還是找不到，嘗試從執行檔所在的目錄開始找 (打包後的環境)
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
                "[SKILLS] ❌ Failed to load skill {}: {} (CWD: {:?}, Checked Path: {:?})",
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
                continue; // 跳過開頭的三反引號
            } else {
                break; // 找到結束的三反引號，結束提取
            }
        }
        if in_code_block {
            result.push(line);
        }
    }

    if result.is_empty() {
        // Fallback: 如果沒找到代碼塊，返回整個內容
        return content.to_string();
    }

    result.join("\n").trim().to_string()
}
