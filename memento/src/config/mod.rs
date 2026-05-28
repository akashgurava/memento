mod paths;
pub mod schema;
mod store;

pub use paths::{app_data_dir, db_path, db_path_relative_to};
pub use schema::AppConfig;
pub use store::{ConfigStore, FsConfigStore};

use std::path::Path;

use crate::error::Result;

/// Load config from a specific path (convenience wrapper).
pub fn load_from(path: &Path) -> Result<AppConfig> {
    FsConfigStore::new(path).load()
}

/// Save config to a specific path (convenience wrapper).
pub fn save_to(config: &AppConfig, path: &Path) -> Result<()> {
    FsConfigStore::new(path).save(config)
}
