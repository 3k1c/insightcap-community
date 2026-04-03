use crate::error::AppError;

pub fn extract_txt(file_path: &str) -> Result<String, AppError> {
    crate::capture::encoding::read_text_file(file_path).map_err(|e| AppError::Capture(e))
}
