use crate::settings::store::HotkeySettings;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::str::FromStr;
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

const DEFAULT_CAPTURE_HOTKEY: &str = "ctrl+alt+f";
const DEFAULT_QUICK_INPUT_HOTKEY: &str = "ctrl+alt+g";
const QUICK_INPUT_WIDTH: f64 = 700.0;
const QUICK_INPUT_HEIGHT: f64 = 80.0;

pub fn handle_shortcut_event(app: &tauri::AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }

    let quick_input_sc = app
        .try_state::<crate::db::AppState>()
        .and_then(|state| {
            tauri::async_runtime::block_on(async {
                sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'hotkeys'")
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .and_then(|v| {
                        v.get("quickInput")
                            .and_then(|k| k.as_str())
                            .map(|s| s.to_lowercase())
                    })
            })
        })
        .unwrap_or_else(|| DEFAULT_QUICK_INPUT_HOTKEY.to_string());

    let is_quick_input = Shortcut::from_str(&quick_input_sc)
        .map(|sc| &sc == shortcut)
        .unwrap_or(false);

    if is_quick_input {
        let app_handle = app.clone();
        show_quick_input_window(&app_handle);
    } else {
        println!("\n[HOTKEY] Global shortcut triggered.");
        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::capture::trigger_capture(app_handle).await {
                eprintln!("[CAPTURE] Background capture failed: {}", e);
            }
        });
    }
}

pub fn show_quick_input_window(app: &tauri::AppHandle) {
    let Some(qc_window) = app.get_webview_window("quick-capture") else {
        eprintln!("[HOTKEY] Quick Input window not found.");
        return;
    };

    match qc_window.is_visible() {
        Ok(is_visible) if should_hide_quick_input(is_visible) => {
            if log_window_result("hide", qc_window.hide()) {
                println!("[HOTKEY] Quick Input window hidden.");
            }
            return;
        }
        Err(e) => eprintln!(
            "[HOTKEY] Quick Input visibility check before toggle failed: {}",
            e
        ),
        _ => {}
    }

    let mut ok = true;
    ok &= log_window_result("unminimize", qc_window.unminimize());
    ok &= log_window_result(
        "set size",
        qc_window.set_size(LogicalSize::new(QUICK_INPUT_WIDTH, QUICK_INPUT_HEIGHT)),
    );
    ok &= log_window_result("center", center_quick_input_window(&qc_window));
    ok &= log_window_result("always on top", qc_window.set_always_on_top(true));
    ok &= log_window_result("show", qc_window.show());
    ok &= log_window_result("focus", qc_window.set_focus());
    ok &= log_window_result(
        "emit show-quick-capture",
        qc_window.emit("show-quick-capture", ()),
    );

    match qc_window.is_visible() {
        Ok(true) if ok => println!("[HOTKEY] Quick Input window shown."),
        Ok(true) => println!("[HOTKEY] Quick Input window visible with warnings."),
        Ok(false) => eprintln!("[HOTKEY] Quick Input window show requested but still not visible."),
        Err(e) => eprintln!("[HOTKEY] Quick Input visibility check failed: {}", e),
    }
}

fn should_hide_quick_input(is_visible: bool) -> bool {
    is_visible
}

fn log_window_result(action: &str, result: tauri::Result<()>) -> bool {
    if let Err(e) = result {
        eprintln!("[HOTKEY] Quick Input {} failed: {}", action, e);
        false
    } else {
        true
    }
}

pub fn apply_global_hotkeys(
    app: &tauri::AppHandle,
    hotkeys: &HotkeySettings,
) -> Result<(), String> {
    let capture_shortcut = parse_hotkey(&hotkeys.capture_clipboard).map_err(|e| {
        format!(
            "Invalid capture shortcut '{}': {}",
            hotkeys.capture_clipboard, e
        )
    })?;
    let quick_input_shortcut = parse_hotkey(&hotkeys.quick_input).map_err(|e| {
        format!(
            "Invalid quick input shortcut '{}': {}",
            hotkeys.quick_input, e
        )
    })?;

    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Failed to unregister existing shortcuts: {}", e))?;

    app.global_shortcut()
        .on_shortcut(capture_shortcut, move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::capture::trigger_capture(handle).await {
                        eprintln!("[HOTKEY] Capture failed: {}", e);
                    }
                });
            }
        })
        .map_err(|e| {
            format!(
                "Failed to register capture shortcut '{}': {}",
                hotkeys.capture_clipboard, e
            )
        })?;

    app.global_shortcut()
        .on_shortcut(quick_input_shortcut, move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                crate::capture::keyboard::show_quick_input_window(app);
            }
        })
        .map_err(|e| {
            format!(
                "Failed to register quick input shortcut '{}': {}",
                hotkeys.quick_input, e
            )
        })?;

    println!(
        "[HOTKEY] Applied shortcuts: capture={}, quick_input={}",
        hotkeys.capture_clipboard, hotkeys.quick_input
    );

    Ok(())
}

fn parse_hotkey(hotkey: &str) -> Result<Shortcut, tauri_plugin_global_shortcut::Error> {
    Shortcut::from_str(hotkey).map_err(tauri_plugin_global_shortcut::Error::from)
}

fn center_quick_input_window<R: tauri::Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    let monitor = window.primary_monitor()?.or(window.current_monitor()?);

    if let Some(monitor) = monitor {
        let work_area = monitor.work_area();
        let scale_factor = monitor.scale_factor();
        let window_size = (
            (QUICK_INPUT_WIDTH * scale_factor).round() as u32,
            (QUICK_INPUT_HEIGHT * scale_factor).round() as u32,
        );
        let (x, y) = quick_input_center_position(
            (work_area.position.x, work_area.position.y),
            (work_area.size.width, work_area.size.height),
            window_size,
        );

        window.set_position(PhysicalPosition::new(x, y))
    } else {
        window.center()
    }
}

fn quick_input_center_position(
    monitor_position: (i32, i32),
    monitor_size: (u32, u32),
    window_size: (u32, u32),
) -> (i32, i32) {
    let x_offset = monitor_size.0.saturating_sub(window_size.0) / 2;
    let y_offset = monitor_size.1.saturating_sub(window_size.1) / 2;

    (
        monitor_position.0 + x_offset as i32,
        monitor_position.1 + y_offset as i32,
    )
}

pub fn register_global_hotkey(app: &tauri::App, pool: &sqlx::SqlitePool) {
    let (capture_hotkey, quick_input_hotkey) = tauri::async_runtime::block_on(async {
        let json_str =
            sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'hotkeys'")
                .fetch_optional(pool)
                .await
                .unwrap_or(None)
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());

        let capture = json_str
            .as_ref()
            .and_then(|v| v.get("captureClipboard"))
            .and_then(|k| k.as_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| DEFAULT_CAPTURE_HOTKEY.to_string());

        let quick_input = json_str
            .as_ref()
            .and_then(|v| v.get("quickInput"))
            .and_then(|k| k.as_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| DEFAULT_QUICK_INPUT_HOTKEY.to_string());

        (capture, quick_input)
    });

    let register_one =
        |app: &tauri::App, hotkey: &str, fallback: &str| match Shortcut::from_str(hotkey) {
            Ok(sc) => {
                if let Err(e) = app.global_shortcut().register(sc) {
                    eprintln!(
                        "[HOTKEY] Failed to register '{}': {}. Using default.",
                        hotkey, e
                    );
                    if let Ok(default_sc) = Shortcut::from_str(fallback) {
                        let _ = app.global_shortcut().register(default_sc);
                    }
                } else {
                    println!(
                        "[SETUP] Global shortcut '{}' registered successfully.",
                        hotkey
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "[HOTKEY] Invalid hotkey '{}': {}. Using default.",
                    hotkey, e
                );
                if let Ok(default_sc) = Shortcut::from_str(fallback) {
                    let _ = app.global_shortcut().register(default_sc);
                }
            }
        };

    register_one(app, &capture_hotkey, DEFAULT_CAPTURE_HOTKEY);
    register_one(app, &quick_input_hotkey, DEFAULT_QUICK_INPUT_HOTKEY);
}

pub fn simulate_copy() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    let _ = enigo.key(Key::Alt, Direction::Release);
    let _ = enigo.key(Key::Shift, Direction::Release);
    let _ = enigo.key(Key::Meta, Direction::Release);
    let _ = enigo.key(Key::Control, Direction::Release);
    std::thread::sleep(std::time::Duration::from_millis(50));

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| format!("Press Ctrl error: {}", e))?;
    std::thread::sleep(std::time::Duration::from_millis(15));

    enigo
        .key(Key::Unicode('c'), Direction::Click)
        .map_err(|e| format!("Click C error: {}", e))?;
    std::thread::sleep(std::time::Duration::from_millis(15));

    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| format!("Release Ctrl error: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_input_position_centers_on_monitor() {
        assert_eq!(
            quick_input_center_position((100, 200), (1600, 900), (700, 80)),
            (550, 610)
        );
    }

    #[test]
    fn quick_input_toggle_hides_when_window_is_visible() {
        assert!(should_hide_quick_input(true));
    }

    #[test]
    fn hotkey_parser_accepts_settings_page_format() {
        assert!(parse_hotkey("CommandOrControl+Alt+F").is_ok());
    }
}
