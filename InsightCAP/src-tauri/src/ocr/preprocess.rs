use image::{DynamicImage, GrayImage, Luma};
use std::io::Cursor;

pub fn preprocess_for_ocr(image_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let gray = img.to_luma8();

    let threshold = otsu_threshold(&gray);
    let binary = binarize(&gray, threshold);

    let enhanced = DynamicImage::ImageLuma8(binary).brighten(15);

    let mut bytes = Vec::new();
    enhanced
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(bytes)
}

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

fn binarize(gray: &GrayImage, threshold: u8) -> GrayImage {
    let (width, height) = gray.dimensions();
    let mut binary = GrayImage::new(width, height);
    for (x, y, pixel) in gray.enumerate_pixels() {
        let value = if pixel.0[0] > threshold { 255u8 } else { 0u8 };
        binary.put_pixel(x, y, Luma([value]));
    }
    binary
}
