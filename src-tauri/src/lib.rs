pub mod attachments;
pub mod commands;
pub mod config;
pub mod conversations;
pub mod dto;
pub mod model_profiles;
pub mod ollama;
pub mod platform;
pub mod product;
pub mod prompt;
pub mod sessions;
pub mod settings;
pub mod skills;
pub mod state;

use state::AppState;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let state = AppState::initialize(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                if let Some(state) = window.try_state::<AppState>() {
                    tauri::async_runtime::block_on(async {
                        let _ = state.sessions.force_shutdown().await;
                    });
                }
            }
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                if paths.is_empty() || paths.len() > 10 {
                    return;
                }
                if let Some(state) = window.try_state::<AppState>() {
                    if let Some(grant) = commands::grant_dropped_attachments(&state, paths.to_vec())
                    {
                        let _ = window.emit(
                            "cc-panel://attachment-drop",
                            serde_json::json!({ "grant": grant }),
                        );
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::choose_project_root,
            commands::clear_project_root,
            commands::choose_additional_root,
            commands::remove_additional_root,
            commands::refresh_skills,
            commands::set_skill_override,
            commands::get_model_status,
            commands::set_user_model,
            commands::clear_user_model,
            commands::get_ollama_status,
            commands::save_ollama_preferences,
            commands::enhance_prompt,
            commands::pick_and_import_attachments,
            commands::import_dropped_attachments,
            commands::preview_attachment,
            commands::confirm_sensitive_import,
            commands::remove_attachment,
            commands::clear_attachments,
            commands::compose_preview,
            commands::compose_and_copy,
            commands::list_model_profiles,
            commands::save_model_profile,
            commands::prompt_and_save_model_profile,
            commands::delete_model_profile,
            commands::select_model_profile,
            commands::restore_model_profile_selection,
            commands::list_conversations,
            commands::run_demo_sandbox,
            commands::delete_conversation,
            commands::rename_conversation,
            commands::set_conversation_favorite,
            commands::set_conversation_archived,
            commands::start_claude_session,
            commands::send_claude_message,
            commands::stop_claude_session,
            commands::respond_to_permission,
            commands::retry_permission,
            commands::list_permission_rules,
            commands::save_permission_rule,
            commands::delete_permission_rule,
            commands::load_conversation_history,
            commands::set_native_notifications_enabled,
            commands::get_project_memory,
            commands::save_project_memory,
            commands::run_environment_check,
            commands::repair_environment_check,
            commands::collect_diagnostics,
            commands::check_for_updates,
            commands::download_update,
            commands::launch_update,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run CC Panel");
}
