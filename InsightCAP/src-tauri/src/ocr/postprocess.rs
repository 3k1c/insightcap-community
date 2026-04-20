/// 文字後處理模組（第三層 OCR 增強）
///
/// 在 OCR 輸出後、寫入 clean_content 前，套用規則修正管線：
/// 字符修正 → 斷行修正 → 雜訊去除 → 標點標準化
/// 速度極快（毫秒級），無需新增依賴（regex crate 已存在）。
use lazy_static::lazy_static;
use regex::Regex;

pub enum OcrLanguage {
    Chinese,
    English,
    Mixed,
}

/// 根據文字中中文字符比例自動偵測語言
pub fn detect_language(text: &str) -> OcrLanguage {
    let total = text.chars().filter(|c| !c.is_whitespace()).count();
    if total == 0 {
        return OcrLanguage::Mixed;
    }
    let chinese = text
        .chars()
        .filter(|c| *c >= '\u{4e00}' && *c <= '\u{9fff}')
        .count();
    let ratio = chinese as f32 / total as f32;
    if ratio > 0.5 {
        OcrLanguage::Chinese
    } else if ratio > 0.1 {
        OcrLanguage::Mixed
    } else {
        OcrLanguage::English
    }
}

/// 對 OCR 輸出文字套用規則修正管線
/// 依序執行：字符修正 → 斷行修正 → 雜訊去除 → 標點標準化
pub fn postprocess_ocr_text(raw_text: &str, language: OcrLanguage) -> String {
    let text = fix_character_confusion(raw_text);
    let text = match language {
        OcrLanguage::Chinese | OcrLanguage::Mixed => fix_chinese_line_breaks(&text),
        OcrLanguage::English => fix_english_line_breaks(&text),
    };
    let text = remove_page_artifacts(&text);
    normalize_whitespace(&text)
}

/// 字符混淆修正（僅修正有明確上下文的情況，避免誤改）
/// - 連續 rn 且前後均為小寫字母 → m
fn fix_character_confusion(text: &str) -> String {
    lazy_static! {
        static ref RN_RE: Regex = Regex::new(r"([a-z])rn([a-z])").unwrap();
    }
    RN_RE.replace_all(text, "${1}m${2}").to_string()
}

/// 中文斷行修正：去除中文字符之間的換行符
/// 中文不用空格分詞，換行通常是排版產生的，應合併為同一行
fn fix_chinese_line_breaks(text: &str) -> String {
    lazy_static! {
        // 中文字與中文字之間的單一換行 → 合併
        static ref ZH_NL: Regex =
            Regex::new(r"([\u4e00-\u9fff，。！？、；：「」『』（）【】])\n([\u4e00-\u9fff，。！？、；：「」『』（）【】])").unwrap();
        // 中文字後接英文/數字之間的換行也合併
        static ref ZH_EN_NL: Regex =
            Regex::new(r"([\u4e00-\u9fff])\n([a-zA-Z0-9])").unwrap();
    }
    let text = ZH_NL.replace_all(text, "$1$2");
    ZH_EN_NL.replace_all(&text, "$1$2").to_string()
}

/// 英文斷行修正：修復連字號換行（如 "connec-\ntion" → "connection"）
fn fix_english_line_breaks(text: &str) -> String {
    lazy_static! {
        static ref HYPHEN_NL: Regex = Regex::new(r"([a-z])-\n([a-z])").unwrap();
    }
    HYPHEN_NL.replace_all(text, "$1$2").to_string()
}

/// 去除頁碼與頁眉頁腳雜訊
/// 匹配：獨立成行且只含數字（1~4 位）的頁碼行
fn remove_page_artifacts(text: &str) -> String {
    lazy_static! {
        static ref PAGE_NUM: Regex = Regex::new(r"(?m)^\s*\d{1,4}\s*$").unwrap();
    }
    PAGE_NUM.replace_all(text, "").to_string()
}

/// 標準化空白：將三個以上連續空行合併為雙空行（保留段落分隔）
fn normalize_whitespace(text: &str) -> String {
    lazy_static! {
        static ref MULTI_NL: Regex = Regex::new(r"\n{3,}").unwrap();
    }
    MULTI_NL.replace_all(text.trim(), "\n\n").to_string()
}
