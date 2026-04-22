use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::str::FromStr;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

const DEFAULT_CAPTURE_HOTKEY: &str = "ctrl+alt+f";
const DEFAULT_QUICK_INPUT_HOTKEY: &str = "ctrl+alt+g";

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
    if let Some(qc_window) = app.get_webview_window("quick-capture") {
        let _ = qc_window.show();
        let _ = qc_window.unminimize();
        let _ = qc_window.set_focus();
        let _ = qc_window.emit("show-quick-capture", ());
        println!("[HOTKEY] Quick Input window shown.");
    }
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
