pub mod schema;

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{ConfigError, Result};
pub use schema::AppConfig;

const APP_DIR_NAME: &str = "xyz.225274.memento";
const CONFIG_FILE: &str = "config.toml";
const DB_FILE: &str = "memento.duckdb";

/// Get the application data directory (platform-specific, for Tauri GUI)
pub fn app_data_dir() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| ConfigError::invalid("Could not determine config directory"))?;
    Ok(base.join(APP_DIR_NAME))
}

/// Get the DB path relative to the global app data directory
pub fn db_path() -> Result<PathBuf> {
    Ok(app_data_dir()?.join(DB_FILE))
}

/// Get the DB path relative to a config file's directory
pub fn db_path_relative_to(config_path: &Path) -> PathBuf {
    config_path.parent().unwrap_or(Path::new(".")).join(DB_FILE)
}

/// Load config from the global app data directory, creating defaults if missing
pub fn load() -> Result<AppConfig> {
    let dir = app_data_dir()?;
    let config_path = dir.join(CONFIG_FILE);

    if !config_path.exists() {
        fs::create_dir_all(&dir)?;
        let config = AppConfig::default();
        save(&config)?;
        return Ok(config);
    }

    let content = fs::read_to_string(&config_path)?;
    let config: AppConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Load config from a specific path, creating defaults if missing
pub fn load_from(path: &Path) -> Result<AppConfig> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let config = AppConfig::default();
        save_to(&config, path)?;
        return Ok(config);
    }

    let content = fs::read_to_string(path)?;
    let config: AppConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Save config to the global app data directory
pub fn save(config: &AppConfig) -> Result<()> {
    let dir = app_data_dir()?;
    fs::create_dir_all(&dir)?;
    let config_path = dir.join(CONFIG_FILE);
    save_to(config, &config_path)
}

/// Save config to a specific path
pub fn save_to(config: &AppConfig, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}
