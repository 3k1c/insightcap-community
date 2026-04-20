#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

pub mod postprocess;
pub mod preprocess;

/// 統一的 OCR 入口 (ADR-025：原生系統 OCR 替代 GLM-OCR)
/// 對外介面: 接收二進制圖片資料
/// 輸出: 識別出的文字字串
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
        // 暫不支持的平台回退
        Err("原生 OCR 目前僅支援 Windows 與 macOS".to_string())
    }
}
