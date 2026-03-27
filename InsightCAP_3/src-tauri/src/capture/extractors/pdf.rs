//! # PDF 提取器
//!
//! 逐頁判斷文字型或圖片型（掃描頁），並支援混合模式。
//! 支援根據檔案大小與頁數自動分流至背景 OCR。
//! 見 Phase3.1.md P3.1-02。

use crate::error::AppError;
use pdfium_render::prelude::*;

pub struct PdfChunk {
    pub page_num: usize,
    pub clean_content: String,
    pub image_path: Option<String>,
    pub chunk_type: String, // "text" | "image"
    pub status: String,     // "processed" | "pending_ocr"
}

const OFFLOAD_PAGE_THRESHOLD: usize = 10; // 超過 10 頁則分流
const OFFLOAD_SIZE_THRESHOLD: u64 = 10 * 1024 * 1024; // 超過 10MB 則分流

/// 提取 PDF 所有頁面，並對掃描頁或包含圖片的頁面進行 OCR
pub async fn extract_pdf(
    _kb_path: &str,
    file_path: &str,
    _file_stem: &str,
) -> Result<Vec<PdfChunk>, AppError> {
    let file_path_owned = file_path.to_string(); // Clone file_path for the blocking task

    // 獲取文件大小
    let file_size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);

    let doc_content = tokio::task::spawn_blocking(
        move || -> Result<Vec<(usize, Option<Vec<u8>>, String, String)>, AppError> {
            let pdfium = Pdfium::new(
                Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
                    .or_else(|_| Pdfium::bind_to_system_library())
                    .map_err(|e| AppError::Capture(format!("Pdfium 綁定失敗: {}", e)))?,
            );

            let doc = pdfium
                .load_pdf_from_file(&file_path_owned, None)
                .map_err(|e| AppError::Capture(format!("PDF 載入失敗: {}", e)))?;

            let mut extracted = Vec::new();
            let page_count = doc.pages().len() as usize;
            let should_defer =
                page_count > OFFLOAD_PAGE_THRESHOLD || file_size > OFFLOAD_SIZE_THRESHOLD;

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
                    // 掃描頁或混合頁 -> 需要 OCR
                    if should_defer {
                        // 推遲 OCR
                        let placeholder = if native_text.is_empty() {
                            format!("[第 {} 頁][掃描圖片，等待背景 OCR...]", page_num)
                        } else {
                            // 混合模式：先給原生文字，標記待補充圖片內容
                            format!(
                                "[第 {} 頁][原生文字與圖片]\n{}\n\n[等待背景 OCR 補充圖片內容...]",
                                page_num, native_text
                            )
                        };
                        extracted.push((page_num, None, placeholder, "pending_ocr".to_string()));
                    } else {
                        // 即時 OCR (小文件)
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
            }
            Ok(extracted)
        },
    )
    .await
    .map_err(|e| AppError::Capture(format!("執行緒池錯誤: {}", e)))??;

    let mut chunks = Vec::new();

    for (page_num, img_bytes, native_text, status) in doc_content {
        if status == "pending_ocr" {
            chunks.push(PdfChunk {
                page_num,
                clean_content: native_text, // 帶有標記的內容
                image_path: None,
                chunk_type: "hybrid".to_string(),
                status,
            });
        } else if let Some(bytes) = img_bytes {
            // 已完成即時 OCR -> Phase 2 改由 vision_llm 處理，此處先給佔位
            let ocr_text = "[背景 OCR 任務已排隊]".to_string();

            // 合併原生文字與 OCR 文字
            let merged = if native_text.is_empty() {
                format!("[第 {} 頁]\n{}", page_num, ocr_text)
            } else {
                format!(
                    "[第 {} 頁][原生文字]\n{}\n[OCR 內容]\n{}",
                    page_num, native_text, ocr_text
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
            Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
                .or_else(|_| Pdfium::bind_to_system_library())
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
