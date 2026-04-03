use crate::settings::store::GeneralSettings;
use zhconv::Variant;

pub struct LanguageNormalizer;

impl Default for LanguageNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageNormalizer {
    pub fn new() -> Self {
        Self
    }

    /// 根據用戶設定標準化內文語系
    pub fn normalize(&self, content: &str, settings: &GeneralSettings) -> String {
        let variant = match settings.language.as_str() {
            "zh-TW" => Variant::ZhTW,
            "zh-CN" => Variant::ZhCN,
            "en" => Variant::ZhTW,
            _ => return content.to_string(),
        };
        zhconv::zhconv(content, variant)
    }

    /// 強制轉換為繁體（用於 RAG 向量化對齊）
    pub fn to_traditional(&self, content: &str) -> String {
        zhconv::zhconv(content, Variant::ZhTW)
    }

    /// 強制轉換為簡體（用於 RAG 互通）
    pub fn to_simplified(&self, content: &str) -> String {
        zhconv::zhconv(content, Variant::ZhCN)
    }
}

lazy_static::lazy_static! {
    pub static ref NORMALIZER: LanguageNormalizer = LanguageNormalizer::new();
}
