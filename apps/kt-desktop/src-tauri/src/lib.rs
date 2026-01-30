//! k-Terminus Desktop - Tauri Backend

mod commands;
mod state;

use tauri::Manager;

pub use state::AppState;

/// Initialize and run the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize application state
            let state = AppState::new();
            app.manage(state);

            // Get the main window
            let window = app.get_webview_window("main").unwrap();

            // Set up event handlers
            #[cfg(debug_assertions)]
            {
                window.open_devtools();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::start_orchestrator,
            commands::stop_orchestrator,
            commands::list_machines,
            commands::get_machine,
            commands::list_sessions,
            commands::create_session,
            commands::kill_session,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
