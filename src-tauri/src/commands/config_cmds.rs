use tauri::State;

use memento::config::{self, AppConfig};
use crate::state::AppState;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.read().map_err(|e| e.to_string())?;
    Ok(config.clone())
}

#[tauri::command]
pub fn set_scan_roots(roots: Vec<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut config = state.config.write().map_err(|e| e.to_string())?;
    config.scan.roots = roots;
    config::save(&config).map_err(|e| e.to_string())?;
    Ok(())
}
