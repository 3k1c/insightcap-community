use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

pub async fn recognize_text_from_bytes(image_data: &[u8]) -> Result<String, String> {
    // 為了避免阻塞非同步執行緒，使用 spawn_blocking 包裝同步呼叫
    let data = image_data.to_vec();
    tokio::task::spawn_blocking(move || {
        let stream = InMemoryRandomAccessStream::new().map_err(|e: windows::core::Error| {
            format!("建立 InMemoryRandomAccessStream 失敗: {}", e)
        })?;

        let writer = DataWriter::CreateDataWriter(&stream)
            .map_err(|e: windows::core::Error| format!("建立 DataWriter 失敗: {}", e))?;

        writer
            .WriteBytes(&data)
            .map_err(|e: windows::core::Error| format!("寫入圖片資料失敗: {}", e))?;

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
            .map_err(|e: windows::core::Error| format!("Stream Seek 失敗: {}", e))?;

        let decoder = BitmapDecoder::CreateAsync(&stream)
            .map_err(|e: windows::core::Error| format!("建立 BitmapDecoder 失敗: {}", e))?
            .get()
            .map_err(|e: windows::core::Error| format!("解析圖片失敗 (可能是格式不符): {}", e))?;

        // 確保將圖片解碼轉為 OcrEngine 要求的 Bgra8 和 Premultiplied 格式，避免色彩錯亂導致識別失敗
        let bitmap = decoder
            .GetSoftwareBitmapConvertedAsync(
                BitmapPixelFormat::Bgra8,
                BitmapAlphaMode::Premultiplied,
            )
            .map_err(|e: windows::core::Error| format!("獲取並轉換 SoftwareBitmap 失敗: {}", e))?
            .get()
            .map_err(|e: windows::core::Error| format!("等待 SoftwareBitmap 失敗: {}", e))?;

        // 明確指定繁體中文（zh-Hant）進行辨識，這樣才能正確處理中英混排，
        // 而不是依賴系統預設語言（可能被設為純英文，導致中文字眼完全消失和辨識錯亂）
        let language = Language::CreateLanguage(&HSTRING::from("zh-Hant"))
            .map_err(|e: windows::core::Error| format!("無法建立 Language 物件: {}", e))?;

        let engine =
            OcrEngine::TryCreateFromLanguage(&language).map_err(|e: windows::core::Error| {
                format!("初始化 OcrEngine 失敗 (這系統可能未安裝繁中語言包): {}", e)
            })?;

        let result = engine
            .RecognizeAsync(&bitmap)
            .map_err(|e: windows::core::Error| format!("開始識別失敗: {}", e))?
            .get()
            .map_err(|e: windows::core::Error| format!("等待識別結果失敗: {}", e))?;

        let text = result
            .Text()
            .map_err(|e: windows::core::Error| format!("獲取識別結果文字失敗: {}", e))?;

        Ok(text.to_string())
    })
    .await
    .map_err(|e| format!("執行緒錯誤: {}", e))?
}
