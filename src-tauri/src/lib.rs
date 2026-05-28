mod commands;
mod state;

use std::sync::{Arc, Mutex, RwLock};

use state::{AppState, ScanManager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "memento=info,memento_lib=info".into()),
        )
        .init();

    let app_config = memento::config::load().expect("Failed to load configuration");
    let db_path = memento::config::db_path().expect("Failed to determine database path");
    std::fs::create_dir_all(db_path.parent().unwrap()).expect("Failed to create app data directory");
    let conn = memento::db::init_db(&db_path).expect("Failed to initialize database");

    let app_state = AppState {
        db: Arc::new(Mutex::new(conn)),
        config: Arc::new(RwLock::new(app_config)),
        scan_manager: Arc::new(Mutex::new(ScanManager::new())),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_scan_roots,
            commands::start_scan,
            commands::cancel_scan,
            commands::get_scan_status,
            commands::get_library_stats,
            commands::find_exact_duplicates,
            commands::find_near_duplicates,
            commands::get_duplicate_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
