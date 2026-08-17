pub mod attachments;
pub mod commands;
pub mod config;
pub mod dto;
pub mod ollama;
pub mod platform;
pub mod prompt;
pub mod settings;
pub mod skills;
pub mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let state = AppState::initialize(app.handle())?;
            app.manage(state);
            Ok(())
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
            commands::confirm_sensitive_import,
            commands::remove_attachment,
            commands::clear_attachments,
            commands::compose_preview,
            commands::compose_and_copy,
            commands::set_native_notifications_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run CC Panel");
}
