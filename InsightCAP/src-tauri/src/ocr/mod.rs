#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

pub mod postprocess;
pub mod preprocess;

pub async fn perform_ocr(image_data: &[u8]) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        windows::recognize_text_from_bytes(image_data).await
    }

    #[cfg(target_os = "macos")]
    {
        macos::recognize_text_from_bytes(image_data).await
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("Native OCR currently supports only Windows and macOS".to_string())
    }
}
