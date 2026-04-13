/// 系統托盤狀態管理與動態圖示合成
///
/// 圖示：反白月亮+星星（32×32，透明背景），右下角疊 10×10 狀態圓點。
///   Idle       → 綠色  (閑置)
///   Processing → 黃色  (擷取處理中)
///   Done       → 藍色  (完成，3 秒後自動回 Idle)
///   Error      → 紅色  (發生錯誤)

use image::{Rgba, RgbaImage};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

// ─── 狀態定義 ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayStatus {
    Idle,
    Processing,
    Done,
    Error,
}

impl TrayStatus {
    fn dot_color(self) -> Rgba<u8> {
        match self {
            TrayStatus::Idle       => Rgba([34,  197, 94,  255]), // 綠
            TrayStatus::Processing => Rgba([234, 179, 8,   255]), // 黃
            TrayStatus::Done       => Rgba([59,  130, 246, 255]), // 藍
            TrayStatus::Error      => Rgba([239, 68,  68,  255]), // 紅
        }
    }
}

// ─── 共享狀態 ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TrayState {
    inner: Arc<Mutex<TrayStatus>>,
}

impl TrayState {
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(TrayStatus::Idle)) }
    }

    pub fn get(&self) -> TrayStatus {
        *self.inner.lock().unwrap()
    }

    pub fn set(&self, status: TrayStatus) {
        *self.inner.lock().unwrap() = status;
    }
}

// ─── 繪圖工具 ─────────────────────────────────────────────────────────────────

/// 在 `img` 上畫一個抗鋸齒實心圓，圓心 (cx, cy)，半徑 r，顏色 color。
fn draw_circle(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, color: Rgba<u8>) {
    let x0 = ((cx - r - 1.0).floor() as i32).max(0) as u32;
    let y0 = ((cy - r - 1.0).floor() as i32).max(0) as u32;
    let x1 = ((cx + r + 1.0).ceil() as u32).min(img.width() - 1);
    let y1 = ((cy + r + 1.0).ceil() as u32).min(img.height() - 1);

    for py in y0..=y1 {
        for px in x0..=x1 {
            let fx = px as f32 + 0.5;
            let fy = py as f32 + 0.5;
            let dist = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let alpha = ((r - dist + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
            if alpha == 0 { continue; }
            let bg = *img.get_pixel(px, py);
            let a = alpha as f32 / 255.0;
            img.put_pixel(px, py, Rgba([
                (color[0] as f32 * a + bg[0] as f32 * (1.0 - a)) as u8,
                (color[1] as f32 * a + bg[1] as f32 * (1.0 - a)) as u8,
                (color[2] as f32 * a + bg[2] as f32 * (1.0 - a)) as u8,
                (alpha as f32 + bg[3] as f32 * (1.0 - a)) as u8,
            ]));
        }
    }
}


// ─── InsightCAP Logo 圖示 ────────────────────────────────────────────────────
//
// 直接嵌入由 SVG 渲染好的 PNG，品質遠優於手繪像素。
// tray-icon-32.png 在 build 前由 gen-all.cjs 產生。

static TRAY_PNG: &[u8] = include_bytes!("../icons/tray-icon-32.png");

fn make_logo_icon() -> RgbaImage {
    // 從嵌入 PNG 解碼，品質遠優於手繪像素
    image::load_from_memory(TRAY_PNG)
        .expect("failed to decode tray-icon-32.png")
        .to_rgba8()
}

// ─── 圖示合成 ─────────────────────────────────────────────────────────────────

/// 供 lib.rs 初始化托盤時使用
pub fn compose_icon_pub(status: TrayStatus) -> RgbaImage {
    compose_icon(status)
}

fn compose_icon(status: TrayStatus) -> RgbaImage {
    let mut base = make_logo_icon();
    let size = 32u32;

    // 圓點：10×10，右下角留 1px 邊距
    let dot_size: f32 = 5.0; // 半徑（直徑 10）
    let margin: f32 = 1.5;
    let cx = size as f32 - dot_size - margin;
    let cy = size as f32 - dot_size - margin;
    let color = status.dot_color();

    // 白色描邊（讓圓點在深色/淺色工作列都看得清楚）
    draw_circle(&mut base, cx, cy, dot_size + 1.0, Rgba([0, 0, 0, 200]));
    draw_circle(&mut base, cx, cy, dot_size, color);

    base
}

// ─── 公開 API ─────────────────────────────────────────────────────────────────

pub fn set_tray_status(app: &AppHandle, status: TrayStatus) {
    let tray_state = app.state::<TrayState>();
    tray_state.set(status);
    apply_tray_icon(app, status);

    if status == TrayStatus::Done {
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            let state = app_clone.state::<TrayState>();
            if state.get() == TrayStatus::Done {
                state.set(TrayStatus::Idle);
                apply_tray_icon(&app_clone, TrayStatus::Idle);
            }
        });
    }
}

fn apply_tray_icon(app: &AppHandle, status: TrayStatus) {
    let img = compose_icon(status);
    let w = img.width();
    let h = img.height();
    let rgba = img.into_raw();

    let icon = tauri::image::Image::new_owned(rgba, w, h);
    if let Some(tray) = app.tray_by_id("main_tray") {
        let _ = tray.set_icon(Some(icon));
    }
}
