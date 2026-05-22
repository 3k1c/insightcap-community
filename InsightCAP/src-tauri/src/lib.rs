pub mod auth;
pub mod background;
pub mod capture;
pub mod commands;
pub mod db;
pub mod error;
pub mod http_server;
pub mod knowledge_source;
pub mod ocr;
pub mod processing_tasks;
pub mod prompts;
pub mod providers;
pub mod services;
pub mod settings;
pub mod tray_status;
pub mod utils;
pub mod vector_store;
pub mod whisper_transcribe;

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WebviewWindow,
};

#[tauri::command]
fn set_zoom(window: WebviewWindow, factor: f64) -> Result<(), String> {
    window.set_zoom(factor).map_err(|e| e.to_string())
}
use serde_json::json;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .skip_initial_state("quick-capture")
                .build()
        )
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
            // 確保 quick-capture 視窗在啟動時一定是隱藏的
            // （防止 window-state plugin 在全新安裝時意外顯示）
            if let Some(qc_win) = app.get_webview_window("quick-capture") {
                let _ = qc_win.hide();
            }

            let handle = app.handle().clone();

            let app_data_dir = handle
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");

            let _ = std::fs::create_dir_all(&app_data_dir);

            let bootstrap_kb_path = db::connection::read_bootstrap(&app_data_dir);

            let effective_kb_path = if let Some(ref kb_str) = bootstrap_kb_path {
                let p = std::path::PathBuf::from(kb_str);
                if p.exists() || std::fs::create_dir_all(&p).is_ok() {
                    p
                } else {
                    eprintln!("[KB-FALLBACK] Path unreachable: {:?}", p);
                    app_data_dir.join("insightcap_v2")
                }
            } else {
                app_data_dir.join("insightcap_v2_pending")
            };

            let _ = std::fs::create_dir_all(&effective_kb_path);

            let db_key_hex: Option<String> = keyring::Entry::new("insightcap", "auto_login_key")
                .ok()
                .and_then(|e| e.get_password().ok());
            let db_key_ref = db_key_hex.as_deref();

            let startup_db = tauri::async_runtime::block_on(db::connection::init_startup_db(
                &app_data_dir,
                effective_kb_path.clone(),
                db_key_ref,
            ))
            .expect("Fallback database initialization failed. Check app data path permissions.");
            let pool = startup_db.pool;
            let effective_kb_path = startup_db.kb_path;
            let startup_issue = startup_db.startup_issue;

            println!("[SETUP] Database initialized at {:?}", effective_kb_path);

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

            let embedder: Arc<dyn providers::embedding::Embedder> = {
                let model_name = "MultilingualE5Small";
                Arc::new(providers::embedding::fastembed::LazyFastEmbedder::new(model_name))
            };

            let vector_store = vector_store::local::VectorStore::load_or_create(
                &effective_kb_path,
                embedder.dimension(),
            ).unwrap_or_else(|e| {
                eprintln!("[SETUP] VectorStore load failed: {}. Using empty index", e);
                vector_store::local::VectorStore::load_or_create(
                    &effective_kb_path,
                    384,
                ).expect("Failed to create VectorStore")
            });

            let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
            let shutdown_tx = std::sync::Arc::new(shutdown_tx);

            app.manage(pool.clone());
            app.manage(db::AppState::new(
                pool.clone(),
                effective_kb_path.clone(),
                startup_issue,
                vector_store,
                embedder,
                shutdown_tx,
            ));
            app.manage(tray_status::TrayState::new());
            app.manage(processing_tasks::ProcessingTaskState::new());

            let processor_pool = pool.clone();
            let processor_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                background::capture_processor::start_capture_processor(processor_pool, processor_app, shutdown_rx).await;
            });
            background::pattern_promotion::start_pattern_promotion_worker(app.handle().clone());
            background::space_recluster::start_recluster_worker(app.handle().clone());
            background::conversation_scheduler::start_scheduler(app.handle().clone());
            background::ocr_worker::start_ocr_worker(app.handle().clone());
            background::deep_synthesis_engine::start_deep_synthesis_worker(app.handle().clone());
            background::reminder_scheduler::start_reminder_scheduler(app.handle().clone());
            background::telegram_bot::start_telegram_bot(app.handle().clone());

            {
                let cleanup_pool = pool.clone();
                tauri::async_runtime::spawn(async move {
                    let cutoff = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();
                    match sqlx::query(
                        "DELETE FROM memory_chunks WHERE pending_confirm = 1 AND created_at < ?"
                    )
                    .bind(&cutoff)
                    .execute(&cleanup_pool)
                    .await {
                        Ok(r) => {
                            if r.rows_affected() > 0 {
                                println!("[Cleanup] Auto-removed {} pending chunks older than 30 days", r.rows_affected());
                            }
                        }
                        Err(e) => eprintln!("[Cleanup] pending chunk cleanup failed: {}", e),
                    }
                });
            }

            background::cloud_sync_watcher::start_cloud_sync_watcher(app.handle().clone(), effective_kb_path.clone());

            let http_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                http_server::start_api_server(http_app).await;
            });

            let show_i = MenuItemBuilder::with_id("show", "Show/Hide").build(app)?;
            let quit_i = MenuItemBuilder::with_id("quit", "Quit InsightCAP").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&show_i)
                .separator()
                .item(&quit_i)
                .build()?;

            let initial_icon = {
                let img = tray_status::compose_icon_pub(tray_status::TrayStatus::Idle);
                let w = img.width();
                let h = img.height();
                tauri::image::Image::new_owned(img.into_raw(), w, h)
            };

            let _tray = TrayIconBuilder::with_id("main_tray")
                .icon(initial_icon)
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

            let hotkey_settings = tauri::async_runtime::block_on(async {
                let s = settings::store::get_settings(&pool).await.unwrap_or_default();
                s.hotkeys
            });

            if let Err(e) = capture::keyboard::apply_global_hotkeys(app.handle(), &hotkey_settings) {
                eprintln!("[HOTKEY] Failed to apply shortcuts: {}", e);
            }

            println!("\n{}", "=".repeat(50));
            println!("InsightCAP v2 - Phase 1 READY!");
            println!("Startup: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
            println!("{}\n", "=".repeat(50));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_commands::get_auth_status,
            commands::auth_commands::get_startup_status,
            commands::auth_commands::setup_auth,
            commands::auth_commands::try_auto_login,
            commands::auth_commands::login,
            commands::auth_commands::get_lock_status,
            commands::auth_commands::change_password,
            commands::auth_commands::confirm_new_recovery,
            commands::auth_commands::recover_with_mnemonic,
            commands::auth_commands::unlock_migrated_with_password,
            commands::auth_commands::unlock_migrated_with_mnemonic,
            commands::auth_commands::get_pending_recovery,
            commands::auth_commands::reset_recovery_phrase,
            commands::auth_commands::generate_recovery_phrase,
            commands::auth_commands::verify_password,
            commands::auth_commands::restart_app,
            commands::settings_commands::get_settings,
            commands::settings_commands::save_settings,
            commands::settings_commands::pause_global_hotkeys,
            commands::settings_commands::resume_global_hotkeys,
            commands::settings_commands::initialize_workspace,
            commands::settings_commands::switch_kb_path,
            commands::settings_commands::test_ollama,
            commands::settings_commands::test_provider_connection,
            commands::settings_commands::test_model_connection,
            commands::settings_commands::get_chat_llm_supports_thinking,
            commands::capture_commands::quick_capture,
            commands::capture_commands::ingest_file,
            commands::capture_commands::cancel_processing_task,
            commands::capture_commands::create_temp_chunk,
            commands::knowledge_commands::get_sources,
            commands::knowledge_commands::get_captures,
            commands::knowledge_commands::get_sources_timeline,
            commands::knowledge_commands::get_source_preview_by_title,
            commands::knowledge_commands::create_editor_document,
            commands::knowledge_commands::read_editor_document,
            commands::knowledge_commands::save_editor_document,
            commands::knowledge_commands::delete_source,
            commands::knowledge_commands::get_captures_detail,
            commands::knowledge_commands::create_manual_capture,
            commands::knowledge_commands::update_capture,
            commands::knowledge_commands::delete_capture,
            commands::knowledge_commands::process_source,
            commands::knowledge_commands::get_repository_stats,
            commands::knowledge_commands::rebuild_kb_index,
            commands::knowledge_commands::rebuild_source_tags,
            commands::knowledge_commands::export_kb,
            commands::knowledge_commands::import_kb,
            commands::knowledge_commands::delete_kb,
            commands::knowledge_commands::repair_missing_local_copies,
            commands::knowledge_commands::run_knowledge_stress_test,
            commands::memory_commands::confirm_memory_chunk,
            commands::memory_commands::get_pending_memory_chunks,
            commands::memory_commands::get_pending_patterns,
            commands::memory_commands::confirm_pattern,
            commands::memory_commands::batch_confirm_memory_chunks,
            commands::memory_commands::cleanup_expired_pending_chunks,
            commands::memory_commands::update_memory_chunk,
            commands::conversation_commands::get_conversations,
            commands::conversation_commands::create_conversation,
            commands::conversation_commands::get_messages,
            commands::conversation_commands::add_message,
            commands::conversation_commands::summarize_conversation,
            commands::conversation_commands::enqueue_summary,
            commands::conversation_commands::rename_conversation,
            commands::conversation_commands::auto_title_conversation,
            commands::conversation_commands::decide_reminder_ack,
            commands::conversation_commands::delete_conversation,
            commands::conversation_commands::update_conversation,
            commands::project_commands::get_projects,
            commands::project_commands::get_project_conversations,
            commands::project_commands::create_project,
            commands::project_commands::update_project,
            commands::project_commands::delete_project,
            commands::project_commands::update_project_sort_order,
            commands::project_commands::move_conversation_to_project,
            commands::tag_commands::get_all_tags,
            commands::tag_commands::suggest_tags,
            commands::tag_commands::get_source_ids_by_tag,
            commands::space_commands::get_all_spaces,
            commands::space_commands::get_space_insight,
            commands::space_commands::trigger_space_recluster,
            commands::space_commands::get_space_knowledge_guide,
            commands::space_commands::save_space_knowledge_guide,
            commands::space_commands::regenerate_space_knowledge_guide,
            commands::space_commands::create_manual_space,
            commands::space_commands::delete_space,
            commands::decision_commands::create_decision,
            commands::decision_commands::get_due_decisions,
            commands::decision_commands::get_project_decisions,
            commands::decision_commands::report_decision_outcome,
            commands::decision_commands::dismiss_decision,
            commands::chunk_relation_commands::get_chunk_relations,
            commands::editor_commands::open_document,
            commands::editor_commands::open_file_in_system,
            commands::editor_commands::save_document,
            commands::editor_commands::create_document,
            commands::editor_commands::write_binary_file,
            commands::editor_commands::export_document,
            commands::editor_commands::export_pdf_text,
            commands::editor_commands::export_pdf_document,
            commands::editor_commands::read_image_base64,
            commands::editor_commands::copy_image_to_assets,
            commands::editor_commands::copy_editor_image_to_assets,
            commands::editor_commands::save_editor_to_knowledge,
            knowledge_source::enterprise::load_external_kb,
            knowledge_source::enterprise::get_external_kbs,
            knowledge_source::enterprise::remove_external_kb,
            commands::bilibili_auth::open_bilibili_login,
            commands::rag_commands::rag_query,
            commands::rag_commands::editor_ai_rewrite_stream,
            commands::rag_commands::rag_query_stream,
            commands::reminder_commands::get_active_reminders,
            commands::reminder_commands::get_pending_reminders,
            commands::reminder_commands::create_reminder,
            commands::reminder_commands::confirm_reminder,
            commands::reminder_commands::update_reminder_status,
            commands::reminder_commands::snooze_reminder,
            commands::reminder_commands::trigger_urgent_reminder_check,
            commands::reminder_commands::trigger_test_reminder,
            commands::reminder_commands::clear_pending_notifications,
            commands::reminder_commands::test_telegram_notification,
            commands::reminder_commands::telegram_get_allowed_user_ids,
            commands::reminder_commands::debug_list_notifications,
            commands::reminder_commands::get_system_time_info,
            commands::reminder_commands::verify_db_time_format,
            commands::reminder_commands::manual_extract_reminders,
            commands::reminder_commands::check_reminder_health,
            commands::reminder_commands::get_project_timeline,
            whisper_transcribe::whisper_binary_status,
            whisper_transcribe::whisper_model_status,
            whisper_transcribe::whisper_download_model,
            whisper_transcribe::whisper_delete_model,
            capture::video_parser::set_whisper_model_preference,
            set_zoom,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
