use lazy_static::lazy_static;
use regex::Regex;

pub fn clean_window_title(title: &str) -> String {
    if title.trim().is_empty() {
        return String::new();
    }

    let mut cleaned = title.to_string();

    lazy_static! {
        static ref MODIFIED_RE: Regex = Regex::new(r"(?i)\s*[-\u2014]\s*(modified)\s*$").unwrap();
        static ref PREFIX_RE: Regex = Regex::new(r"^[-\*\s]+").unwrap();

        static ref APPS_RE: Regex = Regex::new(r"(?i)\s*[-\u2014]\s*(visual studio code|vscode|insightcap|adobe acrobat.*|waterfox|google chrome|chrome|microsoft.?edge|edge|safari|firefox|brave|opera|vivaldi)\s*$").unwrap();

        static ref DOMAIN_RE: Regex = Regex::new(r"(?i)\s*[-\u2014|]\s*[^-\s|]+\.(com|net|tw|hk|org|io)\s*$").unwrap();

        static ref EXTENSION_RE: Regex = Regex::new(r"(?i)(.+?\.(?:pdf|docx|xlsx|doc|txt|png|jpg|mp4))(?:\s*[-\u2014].*)?$").unwrap();
    }

    cleaned = MODIFIED_RE.replace(&cleaned, "").to_string();
    cleaned = PREFIX_RE.replace(&cleaned, "").to_string();

    let mut last_len = cleaned.len();
    loop {
        cleaned = APPS_RE.replace(&cleaned, "").to_string();
        if cleaned.len() == last_len {
            break;
        }
        last_len = cleaned.len();
    }

    cleaned = DOMAIN_RE.replace(&cleaned, "").to_string();

    if let Some(caps) = EXTENSION_RE.captures(&cleaned) {
        if let Some(m) = caps.get(1) {
            cleaned = m.as_str().to_string();
        }
    }

    cleaned.trim().to_string()
}
