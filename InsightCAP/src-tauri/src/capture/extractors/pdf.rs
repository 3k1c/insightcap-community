//! # PDF 提取器
//!
//! 逐頁判斷文字型或圖片型（掃描頁），並支援混合模式。
//! 支援根據檔案大小與頁數自動分流至背景 OCR。
//! 見 Phase3.1.md P3.1-02。

use crate::error::AppError;
use crate::providers::llm::vision::VisionConfig;
use pdfium_render::prelude::*;

/// 依序嘗試以下路徑來載入 pdfium 動態函式庫：
/// 1. 執行檔同目錄（dev: target/debug/，prod: 安裝目錄）
/// 2. 系統 PATH
fn bind_pdfium() -> Result<Box<dyn PdfiumLibraryBindings>, PdfiumError> {
    // 取得執行檔目錄
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    if let Some(dir) = exe_dir {
        let result = Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path(dir.to_string_lossy().as_ref()),
        );
        if result.is_ok() {
            return result;
        }
    }

    // 最後嘗試系統函式庫
    Pdfium::bind_to_system_library()
}

pub struct PdfChunk {
    pub page_num: usize,
    pub clean_content: String,
    pub image_path: Option<String>,
    pub chunk_type: String, // "text" | "image"
    pub status: String,     // "processed" | "pending_ocr"
}

/// 提取 PDF 所有頁面，並對掃描頁或包含圖片的頁面進行即時 OCR
/// 若提供 vision config 且模型通過 probe，則用 vision model 增強掃描頁辨識
pub async fn extract_pdf(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
    vision: Option<&VisionConfig>,
) -> Result<Vec<PdfChunk>, AppError> {
    let file_path_owned = file_path.to_string();

    let doc_content = tokio::task::spawn_blocking(
        move || -> Result<Vec<(usize, Option<Vec<u8>>, String, String)>, AppError> {
            let pdfium = Pdfium::new(
                bind_pdfium()
                    .map_err(|e| AppError::Capture(format!("Pdfium 綁定失敗: {}", e)))?,
            );

            let doc = pdfium
                .load_pdf_from_file(&file_path_owned, None)
                .map_err(|e| AppError::Capture(format!("PDF 載入失敗: {}", e)))?;

            let mut extracted = Vec::new();

            for (index, page) in doc.pages().iter().enumerate() {
                let page_num = index + 1;
                let mut native_text = String::new();

                if let Ok(page_text) = page.text() {
                    native_text = page_text.all().trim().to_string();
                }

                // 偵測是否包含圖片物件
                let has_images = page
                    .objects()
                    .iter()
                    .any(|obj| obj.as_image_object().is_some());

                if native_text.is_empty() && !has_images {
                    // 空白頁
                    extracted.push((
                        page_num,
                        None,
                        "[空白頁]".to_string(),
                        "processed".to_string(),
                    ));
                } else if !native_text.is_empty() && !has_images {
                    // 純文字頁 -> 直接完成
                    extracted.push((page_num, None, native_text, "processed".to_string()));
                } else {
                    // 掃描頁或混合頁 -> 即時渲染供 OCR（在 spawn_blocking 內安全執行）
                    let render_config = PdfRenderConfig::new()
                        .set_target_width(2000)
                        .set_maximum_height(3000)
                        .set_clear_color(PdfColor::WHITE);

                    let mut img_bytes = None;
                    if let Ok(bitmap) = page.render_with_config(&render_config) {
                        let dynamic_image = bitmap.as_image();
                        use std::io::Cursor;
                        let mut bytes: Vec<u8> = Vec::new();
                        if dynamic_image
                            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
                            .is_ok()
                        {
                            img_bytes = Some(bytes);
                        }
                    }
                    extracted.push((page_num, img_bytes, native_text, "processed".to_string()));
                }
            }
            Ok(extracted)
        },
    )
    .await
    .map_err(|e| AppError::Capture(format!("執行緒池錯誤: {}", e)))??;

    let mut chunks = Vec::new();

    for (page_num, img_bytes, native_text, _status) in doc_content {
        if let Some(bytes) = img_bytes {
            let ocr_text = match crate::ocr::perform_ocr(&bytes).await {
                Ok(raw) => {
                    let lang = crate::ocr::postprocess::detect_language(&raw);
                    crate::ocr::postprocess::postprocess_ocr_text(&raw, lang)
                }
                Err(_) => "[OCR 失敗]".to_string(),
            };

            // Vision model 增強：若可用，以 vision 結果取代 OCR
            let final_ocr = if let Some(vc) = vision {
                match crate::providers::llm::vision::try_vision_enhance(
                    vc,
                    &bytes,
                    crate::providers::llm::vision::ocr_only_prompt(),
                )
                .await
                {
                    Some(vision_text) => {
                        println!("[PDF] Vision 增強成功 (第 {} 頁)", page_num);
                        vision_text
                    }
                    None => ocr_text,
                }
            } else {
                ocr_text
            };

            // 合併原生文字與 OCR/Vision 文字
            let merged = if native_text.is_empty() {
                format!("[第 {} 頁]\n{}", page_num, final_ocr)
            } else {
                format!(
                    "[第 {} 頁][原生文字]\n{}\n[OCR 內容]\n{}",
                    page_num, native_text, final_ocr
                )
            };

            chunks.push(PdfChunk {
                page_num,
                clean_content: merged,
                image_path: None,
                chunk_type: "image".to_string(),
                status: "processed".to_string(),
            });
        } else {
            // 純文字頁面
            chunks.push(PdfChunk {
                page_num,
                clean_content: format!("[第 {} 頁]\n{}", page_num, native_text),
                image_path: None,
                chunk_type: "text".to_string(),
                status: "processed".to_string(),
            });
        }
    }

    Ok(chunks)
}

/// 渲染指定頁面為圖片位元組（用於背景 OCR）
pub async fn render_specific_page(file_path: &str, page_num: usize) -> Result<Vec<u8>, AppError> {
    let file_path_owned = file_path.to_string();

    let img_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, AppError> {
        let pdfium = Pdfium::new(
            bind_pdfium()
                .map_err(|e| AppError::Capture(format!("Pdfium 綁定失敗: {}", e)))?,
        );

        let doc = pdfium
            .load_pdf_from_file(&file_path_owned, None)
            .map_err(|e| AppError::Capture(format!("PDF 載入失敗: {}", e)))?;

        let page = doc
            .pages()
            .get((page_num - 1) as u16)
            .map_err(|_| AppError::Capture(format!("頁碼 {} 超出範圍", page_num)))?;

        let render_config = PdfRenderConfig::new()
            .set_target_width(2000)
            .set_maximum_height(3000)
            .set_clear_color(PdfColor::WHITE);

        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| AppError::Capture(format!("渲染失敗: {}", e)))?;

        let dynamic_image = bitmap.as_image();
        use std::io::Cursor;
        let mut bytes: Vec<u8> = Vec::new();
        dynamic_image
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .map_err(|e| AppError::Capture(format!("編碼失敗: {}", e)))?;

        Ok(bytes)
    })
    .await
    .map_err(|e| AppError::Capture(format!("執行緒池錯誤: {}", e)))??;

    Ok(img_bytes)
}
