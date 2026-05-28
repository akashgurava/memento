use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;

use super::schema::AppConfig;

/// Header comment written at the top of generated config files.
const CONFIG_HEADER: &str = "\
# Memento configuration
#
# Paths: On Windows, use forward slashes (D:/Photos) or single-quoted paths.
# Double-quoted strings treat backslashes as escape characters.
";

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
            tracing::info!("LOAD_CONFIG: CREATED_DEFAULTS. path: {}", self.path.display());
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent)?;
            }
            let config = AppConfig::default();
            self.save(&config)?;
            return Ok(config);
        }

        tracing::debug!("LOAD_CONFIG: START. path: {}", self.path.display());
        let content = fs::read_to_string(&self.path)?;
        let config: AppConfig = serde_yml::from_str(&content)?;
        Ok(config)
    }

    fn save(&self, config: &AppConfig) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let yaml = serde_yml::to_string(config)?;
        let content = format!("{}{}", CONFIG_HEADER, yaml);
        fs::write(&self.path, content)?;
        Ok(())
    }
}
