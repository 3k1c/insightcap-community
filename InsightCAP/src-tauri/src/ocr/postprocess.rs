use lazy_static::lazy_static;
use regex::Regex;

pub enum OcrLanguage {
    Chinese,
    English,
    Mixed,
}

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

pub fn postprocess_ocr_text(raw_text: &str, language: OcrLanguage) -> String {
    let text = fix_character_confusion(raw_text);
    let text = match language {
        OcrLanguage::Chinese | OcrLanguage::Mixed => fix_chinese_line_breaks(&text),
        OcrLanguage::English => fix_english_line_breaks(&text),
    };
    let text = remove_page_artifacts(&text);
    normalize_whitespace(&text)
}

fn fix_character_confusion(text: &str) -> String {
    lazy_static! {
        static ref RN_RE: Regex = Regex::new(r"([a-z])rn([a-z])").unwrap();
    }
    RN_RE.replace_all(text, "${1}m${2}").to_string()
}

fn fix_chinese_line_breaks(text: &str) -> String {
    lazy_static! {
        static ref ZH_NL: Regex = Regex::new(r"([\u4e00-\u9fff])\n([\u4e00-\u9fff])").unwrap();
        static ref ZH_EN_NL: Regex = Regex::new(r"([\u4e00-\u9fff])\n([a-zA-Z0-9])").unwrap();
    }
    let text = ZH_NL.replace_all(text, "$1$2");
    ZH_EN_NL.replace_all(&text, "$1$2").to_string()
}

fn fix_english_line_breaks(text: &str) -> String {
    lazy_static! {
        static ref HYPHEN_NL: Regex = Regex::new(r"([a-z])-\n([a-z])").unwrap();
    }
    HYPHEN_NL.replace_all(text, "$1$2").to_string()
}

fn remove_page_artifacts(text: &str) -> String {
    lazy_static! {
        static ref PAGE_NUM: Regex = Regex::new(r"(?m)^\s*\d{1,4}\s*$").unwrap();
    }
    PAGE_NUM.replace_all(text, "").to_string()
}

fn normalize_whitespace(text: &str) -> String {
    lazy_static! {
        static ref MULTI_NL: Regex = Regex::new(r"\n{3,}").unwrap();
    }
    MULTI_NL.replace_all(text.trim(), "\n\n").to_string()
}
