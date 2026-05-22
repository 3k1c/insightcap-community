use crate::db::AppState;
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use printpdf::path::PaintMode;
use printpdf::*;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::process::Command;
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PdfExportBlock {
    Heading { level: u8, text: String, bold: bool, align: Option<PdfTextAlign> },
    Paragraph { text: String, bold: bool, align: Option<PdfTextAlign>, variant: Option<String> },
    ListItem { text: String, ordered: bool, index: usize, bold: bool, align: Option<PdfTextAlign> },
    Table { rows: Vec<Vec<PdfTableCell>> },
    Image { src: String, width: Option<String>, align: Option<PdfTextAlign> },
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PdfTextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Deserialize)]
pub struct PdfTableCell {
    text: String,
    bold: bool,
    background: Option<String>,
    align: Option<PdfTextAlign>,
}

#[tauri::command]
pub async fn open_document(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_file_in_system(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", &path]);
        c
    };

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(&path);
        c
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(&path);
        c
    };

    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn save_document(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = PathBuf::from(&path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_document(path: String, format: String) -> Result<(), String> {
    if let Some(parent) = PathBuf::from(&path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = if format == "md" { "" } else { "" };
    fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn write_binary_file(
    absolute_path: String,
    base64_content: String,
) -> Result<(), String> {
    if let Some(parent) = PathBuf::from(&absolute_path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let decoded = general_purpose::STANDARD
        .decode(base64_content)
        .map_err(|e| e.to_string())?;
    fs::write(&absolute_path, decoded).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_document(absolute_path: String, content: String) -> Result<(), String> {
    save_document(absolute_path, content).await
}

#[tauri::command]
pub async fn export_pdf_text(absolute_path: String, content: String) -> Result<(), String> {
    let blocks = normalize_pdf_paragraphs(&content)
        .into_iter()
        .map(|text| PdfExportBlock::Paragraph {
            text,
            bold: false,
            align: Some(PdfTextAlign::Left),
            variant: Some("text1".to_string()),
        })
        .collect();
    export_pdf_document(absolute_path, blocks).await
}

#[tauri::command]
pub async fn export_pdf_document(
    absolute_path: String,
    blocks: Vec<PdfExportBlock>,
) -> Result<(), String> {
    if let Some(parent) = PathBuf::from(&absolute_path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let (doc, mut page, mut layer) =
        PdfDocument::new("InsightCAP Export", Mm(210.0), Mm(297.0), "Layer 1");
    let font = load_first_pdf_font(&doc, pdf_text_font_candidates())
        .ok_or_else(|| "No embeddable CJK-capable PDF font found".to_string())?;
    let bold_font = load_first_pdf_font(&doc, pdf_bold_font_candidates())
        .unwrap_or_else(|| font.clone());

    let page_top = 276.0;
    let mut y = page_top;

    for block in blocks {
        match block {
            PdfExportBlock::Heading { level, text, align, .. } => {
                let size = heading_pdf_font_size(level);
                write_pdf_text_block(
                    &doc,
                    &mut page,
                    &mut layer,
                    &mut y,
                    &text,
                    size,
                    &bold_font,
                    39.0,
                    3.0,
                    2.0,
                    align.unwrap_or(PdfTextAlign::Left),
                );
            }
            PdfExportBlock::Paragraph { text, bold, align, variant } => {
                write_pdf_text_block(
                    &doc,
                    &mut page,
                    &mut layer,
                    &mut y,
                    &text,
                    paragraph_pdf_font_size(variant.as_deref()),
                    if bold { &bold_font } else { &font },
                    42.0,
                    1.0,
                    3.5,
                    align.unwrap_or(PdfTextAlign::Left),
                );
            }
            PdfExportBlock::ListItem {
                text,
                ordered,
                index,
                bold,
                align,
            } => {
                let marker = if ordered {
                    format!("{}.", index)
                } else {
                    "-".to_string()
                };
                write_pdf_text_block(
                    &doc,
                    &mut page,
                    &mut layer,
                    &mut y,
                    &format!("{} {}", marker, text),
                    11.0,
                    if bold { &bold_font } else { &font },
                    40.0,
                    0.5,
                    2.0,
                    align.unwrap_or(PdfTextAlign::Left),
                );
            }
            PdfExportBlock::Table { rows } => {
                write_pdf_table(
                    &doc,
                    &mut page,
                    &mut layer,
                    &mut y,
                    &rows,
                    &font,
                    &bold_font,
                );
            }
            PdfExportBlock::Image { src, width, align } => {
                write_pdf_image_block(
                    &doc,
                    &mut page,
                    &mut layer,
                    &mut y,
                    &src,
                    width.as_deref(),
                    align.unwrap_or(PdfTextAlign::Center),
                )?;
            }
        }
    }

    doc.save(&mut BufWriter::new(
        File::create(&absolute_path).map_err(|e| e.to_string())?,
    ))
    .map_err(|e| e.to_string())
}

fn write_pdf_text_block(
    doc: &PdfDocumentReference,
    page: &mut PdfPageIndex,
    layer: &mut PdfLayerIndex,
    y: &mut f64,
    text: &str,
    font_size: f32,
    font: &IndirectFontRef,
    wrap_units: f32,
    before: f32,
    after: f32,
    align: PdfTextAlign,
) {
    *y -= before as f64;
    let line_height = (font_size * 0.56).max(6.0) as f64;
    for line in wrap_pdf_line(text, wrap_units) {
        ensure_pdf_space(doc, page, layer, y, line_height);
        let current_layer = doc.get_page(*page).get_layer(*layer);
        current_layer.set_fill_color(pdf_black());
        current_layer.use_text(
            &line,
            font_size,
            Mm(aligned_pdf_x(&line, wrap_units, align, 18.0, 174.0)),
            Mm(*y as f32),
            font,
        );
        *y -= line_height;
    }
    *y -= after as f64;
}

fn write_pdf_table(
    doc: &PdfDocumentReference,
    page: &mut PdfPageIndex,
    layer: &mut PdfLayerIndex,
    y: &mut f64,
    rows: &[Vec<PdfTableCell>],
    font: &IndirectFontRef,
    bold_font: &IndirectFontRef,
) {
    if rows.is_empty() {
        return;
    }
    *y -= 2.0;
    let margin_x = 18.0_f32;
    let table_width = 174.0_f32;
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let column_width = table_width / column_count as f32;
    let cell_font_size = 9.0_f32;
    let cell_line_height = 5.2_f64;
    let cell_wrap_units = ((column_width - 4.0) * 0.26).max(6.0);

    for (row_index, row) in rows.iter().enumerate() {
        let wrapped_cells: Vec<Vec<String>> = (0..column_count)
            .map(|column_index| {
                row.get(column_index)
                    .map(|cell| wrap_pdf_table_cell(&cell.text, cell_wrap_units))
                    .unwrap_or_else(|| vec![String::new()])
            })
            .collect();
        let max_lines = wrapped_cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let row_height = max_lines as f64 * cell_line_height + 5.0;

        ensure_pdf_space(doc, page, layer, y, row_height + 2.0);
        let current_layer = doc.get_page(*page).get_layer(*layer);
        for column_index in 0..column_count {
            let x = margin_x + column_index as f32 * column_width;
            if let Some(text) = row.get(column_index) {
                if let Some(background) = text.background.as_deref().and_then(parse_hex_color) {
                    current_layer.set_fill_color(background);
                    current_layer.add_rect(
                        Rect::new(
                            Mm(x),
                            Mm((*y - row_height) as f32),
                            Mm(x + column_width),
                            Mm(*y as f32),
                        )
                        .with_mode(PaintMode::Fill),
                    );
                }
                current_layer.set_fill_color(pdf_black());
                for (line_index, rendered_text) in wrapped_cells[column_index].iter().enumerate() {
                    current_layer.use_text(
                        rendered_text,
                        cell_font_size,
                        Mm(table_cell_text_x(rendered_text, x, column_width, cell_wrap_units, text.align.unwrap_or(PdfTextAlign::Left))),
                        Mm((*y - 6.5 - line_index as f64 * cell_line_height) as f32),
                        if row_index == 0 || text.bold { bold_font } else { font },
                    );
                }
            }
            current_layer.add_rect(
                Rect::new(
                    Mm(x),
                    Mm((*y - row_height) as f32),
                    Mm(x + column_width),
                    Mm(*y as f32),
                )
                .with_mode(PaintMode::Stroke),
            );
        }
        *y -= row_height;
    }
    *y -= 4.0;
}

fn write_pdf_image_block(
    doc: &PdfDocumentReference,
    page: &mut PdfPageIndex,
    layer: &mut PdfLayerIndex,
    y: &mut f64,
    src: &str,
    width: Option<&str>,
    align: PdfTextAlign,
) -> Result<(), String> {
    let bytes = decode_pdf_image_source(src)?;
    let dynamic_image = printpdf::image_crate::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let pdf_image = Image::from_dynamic_image(&dynamic_image);

    let content_x = 18.0_f32;
    let content_width = 174.0_f32;
    let dpi = 300.0_f32;
    let base_width = pdf_image.image.width.0 as f32 * 25.4 / dpi;
    let base_height = pdf_image.image.height.0 as f32 * 25.4 / dpi;
    if base_width <= 0.0 || base_height <= 0.0 {
        return Ok(());
    }

    let requested_width = parse_pdf_image_width(width, content_width).unwrap_or(content_width);
    let mut rendered_width = requested_width.min(content_width).max(20.0);
    let mut scale = rendered_width / base_width;
    let max_height = 230.0_f32;
    let mut rendered_height = base_height * scale;
    if rendered_height > max_height {
        scale *= max_height / rendered_height;
        rendered_width = base_width * scale;
        rendered_height = max_height;
    }

    *y -= 3.0;
    ensure_pdf_space(doc, page, layer, y, rendered_height as f64 + 6.0);
    let x = match align {
        PdfTextAlign::Left => content_x,
        PdfTextAlign::Center => content_x + (content_width - rendered_width).max(0.0) / 2.0,
        PdfTextAlign::Right => content_x + (content_width - rendered_width).max(0.0),
    };
    let current_layer = doc.get_page(*page).get_layer(*layer);
    pdf_image.add_to_layer(
        current_layer,
        ImageTransform {
            translate_x: Some(Mm(x)),
            translate_y: Some(Mm((*y as f32) - rendered_height)),
            scale_x: Some(scale),
            scale_y: Some(scale),
            dpi: Some(dpi),
            ..Default::default()
        },
    );
    *y -= rendered_height as f64 + 4.0;
    Ok(())
}

fn ensure_pdf_space(
    doc: &PdfDocumentReference,
    page: &mut PdfPageIndex,
    layer: &mut PdfLayerIndex,
    y: &mut f64,
    needed: f64,
) {
    if *y - needed >= 18.0 {
        return;
    }
    let added = doc.add_page(Mm(210.0), Mm(297.0), "Layer 1");
    *page = added.0;
    *layer = added.1;
    *y = 276.0;
}

fn pdf_text_font_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\msjh.ttc",
            "C:\\Windows\\Fonts\\msjh_0.ttc",
            "C:\\Windows\\Fonts\\msjhl.ttc",
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\msyh_0.ttc",
            "C:\\Windows\\Fonts\\msyhl.ttc",
            "C:\\Windows\\Fonts\\SourceHanSansCN-Regular.otf",
            "C:\\Windows\\Fonts\\SourceHanSansCN-Normal.otf",
            "C:\\Windows\\Fonts\\SourceHanSansCN-Medium.otf",
            "C:\\Windows\\Fonts\\NotoSansTC-VF.ttf",
            "C:\\Windows\\Fonts\\NotoSansHK-VF.ttf",
            "C:\\Windows\\Fonts\\simsunb.ttf",
            "C:\\Windows\\Fonts\\simhei.ttf",
            "C:\\Windows\\Fonts\\DFHeiMedium-V30-UNI-H.ttf",
            "C:\\Windows\\Fonts\\DFHeiA1.ttf",
            "C:\\Windows\\Fonts\\kaiu.ttf",
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/STHeiti Medium.ttc",
        ]
    } else {
        &[
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/arphic/uming.ttc",
        ]
    }
}

fn pdf_bold_font_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "windows") {
        &[
            "C:\\Windows\\Fonts\\msjhbd.ttc",
            "C:\\Windows\\Fonts\\msjhbd_0.ttc",
            "C:\\Windows\\Fonts\\msyhbd.ttc",
            "C:\\Windows\\Fonts\\msyhbd_0.ttc",
            "C:\\Windows\\Fonts\\SourceHanSansCN-Bold.otf",
            "C:\\Windows\\Fonts\\SourceHanSansCN-Heavy.otf",
            "C:\\Windows\\Fonts\\SourceHanSansCN-Medium.otf",
            "C:\\Windows\\Fonts\\simsunb.ttf",
            "C:\\Windows\\Fonts\\simhei.ttf",
            "C:\\Windows\\Fonts\\DFHEIBD.TTF",
            "C:\\Windows\\Fonts\\DFHEI5-A.TTF",
            "C:\\Windows\\Fonts\\DFHeiMedium-V30-UNI-H.ttf",
            "C:\\Windows\\Fonts\\kaiu.ttf",
        ]
    } else if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/STHeiti Medium.ttc",
            "/System/Library/Fonts/PingFang.ttc",
        ]
    } else {
        &[
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        ]
    }
}

fn load_first_pdf_font(
    doc: &PdfDocumentReference,
    candidates: &[&str],
) -> Option<IndirectFontRef> {
    candidates
        .iter()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .find_map(|path| {
            let mut font_file = File::open(path).ok()?;
            doc.add_external_font(&mut font_file).ok()
        })
}

fn heading_pdf_font_size(level: u8) -> f32 {
    match level {
        1 => 16.0,
        2 => 14.0,
        3 => 12.5,
        4 => 11.5,
        _ => 11.0,
    }
}

fn paragraph_pdf_font_size(variant: Option<&str>) -> f32 {
    match variant {
        Some("text2") => 10.0,
        Some("text3") => 9.0,
        _ => 11.0,
    }
}

fn aligned_pdf_x(
    line: &str,
    wrap_units: f32,
    align: PdfTextAlign,
    margin_x: f32,
    content_width: f32,
) -> f32 {
    let line_width = estimate_pdf_units(line).min(wrap_units) / wrap_units * content_width;
    match align {
        PdfTextAlign::Left => margin_x,
        PdfTextAlign::Center => margin_x + (content_width - line_width).max(0.0) / 2.0,
        PdfTextAlign::Right => margin_x + (content_width - line_width).max(0.0),
    }
}

fn table_cell_text_x(text: &str, x: f32, width: f32, wrap_units: f32, align: PdfTextAlign) -> f32 {
    let text_width = estimate_pdf_units(text).min(wrap_units) / wrap_units * (width - 4.0);
    match align {
        PdfTextAlign::Left => x + 2.0,
        PdfTextAlign::Center => x + (width - text_width).max(0.0) / 2.0,
        PdfTextAlign::Right => x + width - text_width - 2.0,
    }
}

fn normalize_pdf_paragraphs(content: &str) -> Vec<String> {
    content
        .replace("\r\n", "\n")
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn wrap_pdf_line(line: &str, max_units: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut units = 0.0;

    for ch in line.chars() {
        let width = pdf_char_width(ch);
        if !current.is_empty() && units + width > max_units {
            lines.push(current);
            current = String::new();
            units = 0.0;
        }
        current.push(ch);
        units += width;
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn wrap_pdf_table_cell(text: &str, max_units: f32) -> Vec<String> {
    let lines: Vec<String> = normalize_pdf_paragraphs(text)
        .into_iter()
        .flat_map(|line| wrap_pdf_line(&line, max_units))
        .filter(|line| !line.is_empty())
        .collect();

    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

fn pdf_char_width(ch: char) -> f32 {
    if ch == '\t' {
        4.0
    } else if ch.is_ascii() {
        0.55
    } else {
        1.0
    }
}

fn estimate_pdf_units(text: &str) -> f32 {
    text.chars().map(pdf_char_width).sum()
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    Some(Color::Rgb(Rgb::new(r, g, b, None)))
}

fn pdf_black() -> Color {
    Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None))
}

fn parse_pdf_image_width(width: Option<&str>, content_width: f32) -> Option<f32> {
    let value = width?.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|number| content_width * number / 100.0);
    }

    value
        .strip_suffix("px")
        .unwrap_or(value)
        .trim()
        .parse::<f32>()
        .ok()
        .map(|px| (px * 0.264583).min(content_width))
}

fn decode_pdf_image_source(src: &str) -> Result<Vec<u8>, String> {
    if let Some(data) = src.strip_prefix("data:") {
        let (metadata, payload) = data
            .split_once(',')
            .ok_or_else(|| "Invalid image data URL".to_string())?;
        if metadata.contains(";base64") {
            return general_purpose::STANDARD
                .decode(payload.trim())
                .map_err(|e| e.to_string());
        }
        return Ok(payload.as_bytes().to_vec());
    }

    let path = src
        .strip_prefix("file:///")
        .or_else(|| src.strip_prefix("file://"))
        .unwrap_or(src);
    fs::read(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_table_cell_text_without_truncating_content() {
        let text = "居民集體行動，要求更名。最後選定的「藍田」";

        let lines = wrap_pdf_table_cell(text, 14.0);

        assert!(lines.len() > 1);
        assert_eq!(lines.join(""), text);
    }
}

#[tauri::command]
pub async fn read_image_base64(absolute_path: String) -> Result<String, String> {
    let bytes = fs::read(&absolute_path).map_err(|e| e.to_string())?;
    let b64 = general_purpose::STANDARD.encode(bytes);
    let ext = PathBuf::from(&absolute_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "image/png",
    };
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[tauri::command]
pub async fn copy_image_to_assets(
    doc_path: String,
    image_abs_path: String,
) -> Result<String, String> {
    let doc_buf = PathBuf::from(doc_path);
    let parent = doc_buf.parent().unwrap_or_else(|| std::path::Path::new(""));
    let assets_dir = parent.join("assets");
    fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;

    let img_buf = PathBuf::from(&image_abs_path);
    let file_name = img_buf.file_name().ok_or("Invalid image path")?;
    let unique_name = format!(
        "{}_{}",
        Uuid::now_v7()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>(),
        file_name.to_string_lossy()
    );

    let dest_path = assets_dir.join(&unique_name);
    fs::copy(&image_abs_path, &dest_path).map_err(|e| e.to_string())?;

    Ok(format!("./assets/{}", unique_name))
}

#[tauri::command]
pub async fn save_editor_to_knowledge(
    pool: State<'_, SqlitePool>,
    state: State<'_, AppState>,
    title: String,
    content: String,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};

    let clean_content = content.clone(); // Tiptap content is markdown

    let mut hasher = Sha256::new();
    hasher.update(clean_content.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());
    let now = Utc::now().to_rfc3339();

    let existing_source_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM sources WHERE title = ? AND type = 'editor'")
            .bind(&title)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let source_id = if let Some(id) = existing_source_id {
        sqlx::query("DELETE FROM captures WHERE source_id = ?")
            .bind(&id)
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query(
            "UPDATE sources SET content_hash = ?, updated_at = ?, clean_content = ? WHERE id = ?",
        )
        .bind(&content_hash)
        .bind(&now)
        .bind(&clean_content)
        .bind(&id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
        id
    } else {
        let new_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO sources (id, type, title, clean_content, content_hash, captured_at, updated_at) VALUES (?, 'editor', ?, ?, ?, ?, ?)"
        )
        .bind(&new_id)
        .bind(&title)
        .bind(&clean_content)
        .bind(&content_hash)
        .bind(&now)
        .bind(&now)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
        new_id
    };

    let doc_dir = state.kb_path.join(".insightcap").join("documents");
    fs::create_dir_all(&doc_dir).map_err(|e| e.to_string())?;
    let safe_name = title.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let file_name = format!("{}_{}.md", safe_name, &source_id[..8]);
    let doc_path = doc_dir.join(&file_name);
    fs::write(&doc_path, &clean_content).map_err(|e| e.to_string())?;

    let local_path_str = doc_path.to_string_lossy().to_string();
    sqlx::query("UPDATE sources SET local_doc_path = ? WHERE id = ?")
        .bind(&local_path_str)
        .bind(&source_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let chunks = split_markdown_by_headings(&clean_content);

    for (idx, chunk_text) in chunks.into_iter().enumerate() {
        if chunk_text.trim().is_empty() {
            continue;
        }

        let capture_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO captures (id, source_id, type, raw_content, clean_content, capture_method, chunk_index, status, created_at, updated_at) VALUES (?, ?, 'text', ?, ?, 'editor_export', ?, 'processed', ?, ?)"
        )
        .bind(&capture_id)
        .bind(&source_id)
        .bind(&chunk_text)
        .bind(&chunk_text)
        .bind(idx as i32)
        .bind(&now)
        .bind(&now)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

        let pool_clone = pool.inner().clone();
        let capture_id_clone = capture_id.clone();
        let chunk_text_clone = chunk_text.clone();
        let embedder_clone = state.embedder.clone();
        let vs_clone = state.vector_store.clone();
        tauri::async_runtime::spawn(async move {
            let tag_engine = crate::services::tag_engine::TagEngine::new(pool_clone.clone());
            let _ = tag_engine
                .process_new_capture(&capture_id_clone, &chunk_text_clone)
                .await;

            let space_engine = crate::services::space_engine::SpaceEngine::new(
                pool_clone,
                embedder_clone,
                vs_clone,
            );
            let _ = space_engine
                .assign_to_space(&capture_id_clone, &chunk_text_clone)
                .await;
        });
    }

    Ok(())
}

fn split_markdown_by_headings(markdown: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for line in markdown.lines() {
        if line.starts_with("# ") || line.starts_with("## ") || line.starts_with("### ") {
            if !current_chunk.trim().is_empty() {
                chunks.push(current_chunk.clone());
                current_chunk.clear();
            }
        }
        current_chunk.push_str(line);
        current_chunk.push('\n');
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk);
    }

    if chunks.is_empty() {
        chunks.push(markdown.to_string());
    }

    chunks
}
