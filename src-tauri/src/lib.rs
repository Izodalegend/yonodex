mod commands;
mod crypto;
mod db;
mod tor;

use commands::AppState;
use tauri::Manager;
use tor::TorState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .manage(AppState::new())
        .manage(TorState::new())
        .invoke_handler(tauri::generate_handler![
            commands::db_is_unlocked,
            commands::db_is_initialized,
            commands::unlock_db,
            commands::lock_db,
            commands::save_wallet_config,
            commands::load_wallet_config,
            commands::clear_wallet_config,
            commands::save_trade,
            commands::load_trades,
            commands::delete_trade,
            commands::save_password_hint,
            commands::load_password_hint,
            commands::reset_local_data,
            tor::tor_start,
            tor::tor_stop,
            tor::tor_is_running,
            tor::tor_get_onion_address,
            tor::tor_get_state,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Whitepaper Layer 4, Section 6.1: Tor is the primary transport.
            // Auto-start on launch - no user action required, no opt-out for P2P.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<TorState>();
                match tor::start(&handle, &state) {
                    Ok(_) => {
                        if let Err(e) =
                            tor::wait_for_socks_port(std::time::Duration::from_secs(180))
                        {
                            log::error!("tor bootstrap timed out: {e}");
                        } else {
                            log::info!("tor bootstrapped - SOCKS port open");
                        }
                    }
                    Err(e) => log::error!("tor auto-start failed: {e}"),
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}