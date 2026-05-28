use std::path::{Path, PathBuf};

use crate::error::{ConfigError, Result};

const APP_DIR_NAME: &str = "xyz.225274.memento";
const DB_FILE: &str = "memento.duckdb";

/// Get the application data directory (platform-specific, for Tauri GUI).
pub fn app_data_dir() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| ConfigError::invalid("Could not determine config directory"))?;
    Ok(base.join(APP_DIR_NAME))
}

/// Get the DB path in the global app data directory.
pub fn db_path() -> Result<PathBuf> {
    Ok(app_data_dir()?.join(DB_FILE))
}

/// Get the DB path relative to a config file's directory.
pub fn db_path_relative_to(config_path: &Path) -> PathBuf {
    config_path.parent().unwrap_or(Path::new(".")).join(DB_FILE)
}
