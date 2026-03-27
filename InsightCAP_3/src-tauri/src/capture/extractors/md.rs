use crate::error::AppError;
use std::fs;

pub fn extract_md(file_path: &str) -> Result<String, AppError> {
    fs::read_to_string(file_path).map_err(|e| AppError::Capture(e.to_string()))
}
