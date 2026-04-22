use std::fs;
use std::path::Path;

pub fn read_text_file(path: impl AsRef<Path>) -> Result<String, String> {
    let bytes = fs::read(path.as_ref()).map_err(|e| format!("Failed to read file: {}", e))?;

    if let Ok(s) = std::str::from_utf8(&bytes) {
        return Ok(s.to_string());
    }

    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(&bytes, true);
    let encoding = detector.guess(None, true);

    let (decoded, _encoding_used, had_errors) = encoding.decode(&bytes);
    if had_errors {
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    } else {
        Ok(decoded.into_owned())
    }
}
