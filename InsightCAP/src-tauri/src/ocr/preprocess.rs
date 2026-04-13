/// 圖像前處理模組（第二層 OCR 增強）
///
/// 在送 OCR 之前對掃描頁圖像做基本前處理：
/// 灰階化 → Otsu 二值化 → 對比度增強
/// 使用現有 `image` crate，無需新增依賴。

use image::{DynamicImage, GrayImage, Luma};
use std::io::Cursor;

/// 對 PDF 掃描頁圖像進行前處理，回傳處理後的 PNG bytes
/// 輸入：原始頁面圖像 bytes（PNG 或 JPEG）
/// 輸出：二值化後的圖像 bytes（PNG）
/// 若前處理失敗，回傳 Err，呼叫方應降級使用原始圖像
pub fn preprocess_for_ocr(image_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("圖像載入失敗: {}", e))?;

    // 1. 灰階化（去除顏色干擾）
    let gray = img.to_luma8();

    // 2. Otsu 自適應二值化（黑白化，去除背景灰階）
    let threshold = otsu_threshold(&gray);
    let binary = binarize(&gray, threshold);

    // 3. 輕度亮度增強（提升文字邊緣清晰度）
    let enhanced = DynamicImage::ImageLuma8(binary).brighten(15);

    // 4. 編碼為 PNG bytes
    let mut bytes = Vec::new();
    enhanced
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(|e| format!("圖像編碼失敗: {}", e))?;

    Ok(bytes)
}

/// Otsu 閾值計算（自動二值化閾值）
/// 最大化前景與背景的類間方差
fn otsu_threshold(gray: &GrayImage) -> u8 {
    let mut histogram = [0u32; 256];
    for pixel in gray.pixels() {
        histogram[pixel.0[0] as usize] += 1;
    }

    let total = (gray.width() as u64) * (gray.height() as u64);
    if total == 0 {
        return 128;
    }

    let mut sum = 0u64;
    for (i, &count) in histogram.iter().enumerate() {
        sum += i as u64 * count as u64;
    }

    let mut sum_b = 0u64;
    let mut w_b = 0u64;
    let mut max_variance = 0f64;
    let mut threshold = 128u8;

    for t in 0..256usize {
        w_b += histogram[t] as u64;
        if w_b == 0 {
            continue;
        }

        let w_f = total - w_b;
        if w_f == 0 {
            break;
        }

        sum_b += t as u64 * histogram[t] as u64;
        let mean_b = sum_b as f64 / w_b as f64;
        let mean_f = (sum - sum_b) as f64 / w_f as f64;

        let variance = w_b as f64 * w_f as f64 * (mean_b - mean_f).powi(2);
        if variance > max_variance {
            max_variance = variance;
            threshold = t as u8;
        }
    }

    threshold
}

/// 以給定閾值對灰階圖像二值化
/// 高於閾值 → 白（255），低於等於閾值 → 黑（0）
fn binarize(gray: &GrayImage, threshold: u8) -> GrayImage {
    let (width, height) = gray.dimensions();
    let mut binary = GrayImage::new(width, height);
    for (x, y, pixel) in gray.enumerate_pixels() {
        let value = if pixel.0[0] > threshold { 255u8 } else { 0u8 };
        binary.put_pixel(x, y, Luma([value]));
    }
    binary
}
