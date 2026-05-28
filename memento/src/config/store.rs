use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;

use super::schema::AppConfig;

/// Trait for loading and saving application configuration.
pub trait ConfigStore {
    /// Load configuration, creating defaults if missing.
    fn load(&self) -> Result<AppConfig>;

    /// Persist configuration.
    fn save(&self, config: &AppConfig) -> Result<()>;
}

/// Filesystem-backed config store.
pub struct FsConfigStore {
    path: PathBuf,
}

impl FsConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl ConfigStore for FsConfigStore {
    fn load(&self) -> Result<AppConfig> {
        if !self.path.exists() {
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent)?;
            }
            let config = AppConfig::default();
            self.save(&config)?;
            return Ok(config);
        }

        let content = fs::read_to_string(&self.path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }

    fn save(&self, config: &AppConfig) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(config)?;
        fs::write(&self.path, content)?;
        Ok(())
    }
}
