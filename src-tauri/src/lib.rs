mod commands;
mod crypto;
mod db;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::db_is_unlocked,
            commands::unlock_db,
            commands::lock_db,
            commands::save_wallet_config,
            commands::load_wallet_config,
            commands::clear_wallet_config,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}