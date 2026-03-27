// InsightCAP v2 — Phase 1 基礎建設
// lib.rs：Phase 1 骨架版，只含基礎模組

pub mod auth;
pub mod background;
pub mod capture;
pub mod commands;
pub mod db;
pub mod knowledge_source;
pub mod providers;
pub mod services;
pub mod settings;
pub mod utils;
pub mod vector_store;
pub mod error;
pub mod http_server;

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use serde_json::json;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let app_handle = window.app_handle().clone();

                    tauri::async_runtime::spawn(async move {
                        let pool = app_handle.state::<sqlx::SqlitePool>();
                        let settings_json = sqlx::query_scalar::<_, String>(
                            "SELECT value FROM settings WHERE key = 'general'"
                        )
                        .fetch_optional(pool.inner())
                        .await
                        .unwrap_or(None);

                        let minimize_to_tray = if let Some(json) = settings_json {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
                                val["minimizeToTray"].as_bool().unwrap_or(true)
                            } else {
                                true
                            }
                        } else {
                            true
                        };

                        if minimize_to_tray {
                            if let Some(win) = app_handle.get_webview_window("main") {
                                let _ = win.hide();
                            }
                        } else {
                            app_handle.exit(0);
                        }
                    });
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();

            // 1. 取得 app_data_dir 和 kb_path
            let app_data_dir = handle
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");

            // 確保 app_data_dir 存在
            let _ = std::fs::create_dir_all(&app_data_dir);

            // 讀取 bootstrap.json
            let bootstrap_kb_path = db::connection::read_bootstrap(&app_data_dir);

            // 確定有效的 KB 路徑
            // - 若 bootstrap.json 存在且路徑可存取 → 使用之
            // - 否則使用 app_data_dir/insightcap_v2（避免與舊 ref DB 衝突）
            let effective_kb_path = if let Some(ref kb_str) = bootstrap_kb_path {
                let p = std::path::PathBuf::from(kb_str);
                if p.exists() || std::fs::create_dir_all(&p).is_ok() {
                    p
                } else {
                    eprintln!("[KB-FALLBACK] Path unreachable: {:?}", p);
                    app_data_dir.join("insightcap_v2")
                }
            } else {
                // 首次安裝：使用獨立路徑避免與 ref 的 DB 衝突
                app_data_dir.join("insightcap_v2_pending")
            };

            let _ = std::fs::create_dir_all(&effective_kb_path);

            // 2. 嘗試從 Keychain 取得 db_key
            let db_key_hex: Option<String> = keyring::Entry::new("insightcap", "auto_login_key")
                .ok()
                .and_then(|e| e.get_password().ok());
            let db_key_ref = db_key_hex.as_deref();

            // 3. 初始化 DB（首次安裝時建立空 DB）
            let pool = tauri::async_runtime::block_on(
                db::connection::init_db(&effective_kb_path, db_key_ref)
            ).unwrap_or_else(|e| {
                eprintln!("[DB] init failed at {:?}: {}. Creating fresh DB in pending dir.", effective_kb_path, e);
                tauri::async_runtime::block_on(
                    db::connection::init_db(&app_data_dir.join("insightcap_v2_pending"), None)
                ).expect("Failed to create pending DB")
            });

            println!("[SETUP] Database initialized at {:?}", effective_kb_path);

            // 4. 確保 knowledge settings 存在
            {
                let pool_ref = pool.clone();
                let kb_str = effective_kb_path.to_string_lossy().to_string();
                tauri::async_runtime::block_on(async move {
                    let row: Option<(String,)> =
                        sqlx::query_as("SELECT value FROM settings WHERE key = 'knowledge'")
                            .fetch_optional(&pool_ref)
                            .await
                            .unwrap_or(None);
                    if row.is_none() {
                        let s = json!({
                            "kbPath": kb_str,
                            "autoClassifyEnabled": true,
                            "autoSpaceMode": "suggest"
                        });
                        let now = chrono::Utc::now().to_rfc3339();
                        let _ = sqlx::query(
                            "INSERT INTO settings (key, value, updated_at) VALUES ('knowledge', ?, ?) \
                             ON CONFLICT(key) DO UPDATE SET value = excluded.value"
                        )
                        .bind(s.to_string())
                        .bind(now)
                        .execute(&pool_ref)
                        .await;
                    }
                });
            }

            // 5. 管理 Pool 和 AppState
            app.manage(pool.clone());
            app.manage(db::AppState::new(pool.clone(), effective_kb_path.clone()));

            // 啟動背景任務
            let processor_pool = pool.clone();
            tauri::async_runtime::spawn(async move {
                background::capture_processor::start_capture_processor(processor_pool).await;
            });
            background::pattern_promotion::start_pattern_promotion_worker(app.handle().clone());
            background::space_recluster::start_recluster_worker(app.handle().clone());
            background::conversation_scheduler::start_scheduler(app.handle().clone());
            background::ocr_worker::start_ocr_worker(app.handle().clone());
            background::cloud_sync_watcher::start_cloud_sync_watcher(app.handle().clone(), effective_kb_path.clone());

            // 啟動 HTTP 服務 (Phase 6 基礎)
            let http_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                http_server::start_api_server(http_app).await;
            });

            // 6. 系統托盤
            let show_i = MenuItemBuilder::with_id("show", "Show/Hide").build(app)?;
            let quit_i = MenuItemBuilder::with_id("quit", "Quit InsightCAP").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&show_i)
                .separator()
                .item(&quit_i)
                .build()?;

            let _tray = TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .expect("default window icon missing")
                        .clone()
                )
                .tooltip("InsightCAP")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // 7. 註冊快捷鍵
            let hotkey_settings = tauri::async_runtime::block_on(async {
                let s = settings::store::get_settings(&pool).await.unwrap_or_default();
                s.hotkeys
            });

            // 將設定字串解析為 Shortcut 物件（例如 "Ctrl+Alt+F"）
            let shortcut_str = hotkey_settings.capture_clipboard;
            match shortcut_str.parse::<Shortcut>() {
                Ok(shortcut) => {
                    app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
                        if event.state() == ShortcutState::Pressed {
                            let handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = capture::trigger_capture(handle).await {
                                    eprintln!("[HOTKEY] Capture failed: {}", e);
                                }
                            });
                        }
                    })?;
                    println!("[HOTKEY] Registered shortcut: {}", shortcut_str);
                }
                Err(e) => {
                    eprintln!("[HOTKEY] Failed to parse shortcut '{}': {:?}", shortcut_str, e);
                }
            }

            println!("\n{}", "=".repeat(50));
            println!("🚀 InsightCAP v2 — Phase 1 READY!");
            println!("📅 Startup: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
            println!("{}\n", "=".repeat(50));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Auth
            commands::auth_commands::get_auth_status,
            commands::auth_commands::setup_auth,
            commands::auth_commands::try_auto_login,
            commands::auth_commands::login,
            commands::auth_commands::get_lock_status,
            commands::auth_commands::change_password,
            commands::auth_commands::confirm_new_recovery,
            commands::auth_commands::recover_with_mnemonic,
            commands::auth_commands::get_pending_recovery,
            commands::auth_commands::generate_recovery_phrase,
            commands::auth_commands::restart_app,
            // Settings
            commands::settings_commands::get_settings,
            commands::settings_commands::save_settings,
            commands::settings_commands::initialize_workspace,
            commands::settings_commands::switch_kb_path,
            commands::settings_commands::test_ollama,
            commands::settings_commands::test_provider_connection,
            // Knowledge
            commands::knowledge_commands::get_sources,
            commands::knowledge_commands::get_captures,
            commands::knowledge_commands::get_pending_patterns,
            commands::knowledge_commands::confirm_pattern,
            commands::knowledge_commands::quick_capture,
            // Conversation
            commands::conversation_commands::get_conversations,
            commands::conversation_commands::create_conversation,
            commands::conversation_commands::get_messages,
            commands::conversation_commands::add_message,
            commands::conversation_commands::summarize_conversation,
            // Tag
            commands::tag_commands::get_all_tags,
            commands::tag_commands::suggest_tags,
            // Space
            commands::space_commands::get_all_spaces,
            // Enterprise
            knowledge_source::enterprise::load_external_kb,
            knowledge_source::enterprise::get_external_kbs,
            knowledge_source::enterprise::remove_external_kb,
            // RAG
            commands::rag_commands::rag_query,
            // Editor
            commands::editor_commands::open_document,
            commands::editor_commands::save_document,
            commands::editor_commands::create_document,
            commands::editor_commands::write_binary_file,
            commands::editor_commands::export_document,
            commands::editor_commands::read_image_base64,
            commands::editor_commands::copy_image_to_assets,
            commands::editor_commands::save_editor_to_knowledge,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
