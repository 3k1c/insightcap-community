use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

pub async fn recognize_text_from_bytes(image_data: &[u8]) -> Result<String, String> {
    let data = image_data.to_vec();
    tokio::task::spawn_blocking(move || {
        let stream = InMemoryRandomAccessStream::new().map_err(|e: windows::core::Error| {
            format!("   InMemoryRandomAccessStream   : {}", e)
        })?;

        let writer = DataWriter::CreateDataWriter(&stream)
            .map_err(|e: windows::core::Error| format!("   DataWriter   : {}", e))?;

        writer
            .WriteBytes(&data)
            .map_err(|e: windows::core::Error| format!("        : {}", e))?;

        writer
            .StoreAsync()
            .map_err(|e: windows::core::Error| e.to_string())?
            .get()
            .map_err(|e: windows::core::Error| e.to_string())?;

        writer
            .FlushAsync()
            .map_err(|e: windows::core::Error| e.to_string())?
            .get()
            .map_err(|e: windows::core::Error| e.to_string())?;

        stream
            .Seek(0)
            .map_err(|e: windows::core::Error| format!("Stream Seek   : {}", e))?;

        let decoder = BitmapDecoder::CreateAsync(&stream)
            .map_err(|e: windows::core::Error| format!("   BitmapDecoder   : {}", e))?
            .get()
            .map_err(|e: windows::core::Error| format!("       (       ): {}", e))?;

        let bitmap = decoder
            .GetSoftwareBitmapConvertedAsync(
                BitmapPixelFormat::Bgra8,
                BitmapAlphaMode::Premultiplied,
            )
            .map_err(|e: windows::core::Error| format!("      SoftwareBitmap   : {}", e))?
            .get()
            .map_err(|e: windows::core::Error| format!("   SoftwareBitmap   : {}", e))?;

        let language = Language::CreateLanguage(&HSTRING::from("zh-Hant"))
            .map_err(|e: windows::core::Error| format!("     Language   : {}", e))?;

        let engine =
            OcrEngine::TryCreateFromLanguage(&language).map_err(|e: windows::core::Error| {
                format!("    OcrEngine    (             ): {}", e)
            })?;

        let result = engine
            .RecognizeAsync(&bitmap)
            .map_err(|e: windows::core::Error| format!("      : {}", e))?
            .get()
            .map_err(|e: windows::core::Error| format!("        : {}", e))?;

        let text = result
            .Text()
            .map_err(|e: windows::core::Error| format!("          : {}", e))?;

        Ok(text.to_string())
    })
    .await
    .map_err(|e| format!("     : {}", e))?
}
