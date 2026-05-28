use std::path::{Path, PathBuf};

use figment::providers::{Format, Serialized, Yaml};
use figment::Figment;
use serde::{Deserialize, Serialize};

use crate::MementoError;

const DEFAULT_PHOTO_EXT: &[&str] = &[
    "jpg", "jpeg", "png", "tiff", "tif", "heic", "heif", "raw", "cr2", "cr3", "nef", "arw", "orf",
    "rw2", "dng", "webp", "avif", "gif", "bmp", "psd",
];
const DEFAULT_VIDEO_EXT: &[&str] = &[
    "mp4", "mov", "avi", "mkv", "m4v", "wmv", "flv", "webm", "3gp", "mts", "m2ts", "ts",
];
const DEFAULT_DB_PATH: &str = "./memento.duckdb";
const DEFAULT_METADATA_BATCH_SIZE: u32 = 500;
const DEFAULT_HASH_BATCH_SIZE: u32 = 100;
const DEFAULT_PARALLELISM: u32 = 0;

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    schema_version: u32,
    db_path: String,
    scan: ScanConfig,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
struct ScanConfig {
    roots: Vec<String>,
    image_extensions: Vec<String>,
    video_extensions: Vec<String>,
    metadata: BatchConfig,
    hash: BatchConfig,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
struct BatchConfig {
    batch_size: u32,
    parallelism: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            db_path: DEFAULT_DB_PATH.to_owned(),
            scan: ScanConfig::default(),
        }
    }
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            image_extensions: DEFAULT_PHOTO_EXT.iter().map(|s| (*s).to_owned()).collect(),
            video_extensions: DEFAULT_VIDEO_EXT.iter().map(|s| (*s).to_owned()).collect(),
            metadata: BatchConfig {
                batch_size: DEFAULT_METADATA_BATCH_SIZE,
                parallelism: DEFAULT_PARALLELISM,
            },
            hash: BatchConfig {
                batch_size: DEFAULT_HASH_BATCH_SIZE,
                parallelism: DEFAULT_PARALLELISM,
            },
        }
    }
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_METADATA_BATCH_SIZE,
            parallelism: DEFAULT_PARALLELISM,
        }
    }
}

// -- CLI (feature-gated) -----------------------------------------------------

#[cfg(feature = "cli")]
#[derive(clap::Parser, Serialize, Default)]
#[command(name = "memento", about = "Photo library deduplication engine")]
pub struct Cli {
    /// Path to config file
    #[arg(short, long)]
    #[serde(skip)]
    config: Option<PathBuf>,

    /// Path to database file
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    db_path: Option<String>,

    #[command(flatten)]
    scan: CliScan,
}

#[cfg(feature = "cli")]
#[derive(clap::Args, Serialize, Default)]
struct CliScan {
    /// Scan root directories
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    roots: Option<Vec<String>>,

    #[command(flatten)]
    metadata: CliMetadataBatch,

    #[command(flatten)]
    hash: CliHashBatch,
}

#[cfg(feature = "cli")]
#[derive(clap::Args, Serialize, Default)]
struct CliMetadataBatch {
    /// Metadata scan batch size
    #[arg(long = "metadata-batch-size")]
    #[serde(rename = "batch_size", skip_serializing_if = "Option::is_none")]
    metadata_batch_size: Option<u32>,

    /// Metadata scan parallelism (0 = all cores)
    #[arg(long = "metadata-parallelism")]
    #[serde(rename = "parallelism", skip_serializing_if = "Option::is_none")]
    metadata_parallelism: Option<u32>,
}

#[cfg(feature = "cli")]
#[derive(clap::Args, Serialize, Default)]
struct CliHashBatch {
    /// Hash scan batch size
    #[arg(long = "hash-batch-size")]
    #[serde(rename = "batch_size", skip_serializing_if = "Option::is_none")]
    hash_batch_size: Option<u32>,

    /// Hash scan parallelism (0 = all cores)
    #[arg(long = "hash-parallelism")]
    #[serde(rename = "parallelism", skip_serializing_if = "Option::is_none")]
    hash_parallelism: Option<u32>,
}

// -- Resolution ---------------------------------------------------------------

impl AppConfig {
    /// Resolve config: CLI args > config file > defaults.
    #[cfg(feature = "cli")]
    pub fn from_cli(cli: Cli) -> Result<Self, MementoError> {
        let mut figment = Figment::from(Serialized::defaults(Self::default()));

        if let Some(ref path) = cli.config {
            figment = figment.merge(Yaml::file(path));
        }

        figment = figment.merge(Serialized::globals(&cli));

        figment.extract().map_err(|e| MementoError::ConfigError {
            path: cli
                .config
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            error: e.to_string(),
        })
    }

    fn resolved_db_path(&self, config_dir: &Path) -> PathBuf {
        let db = PathBuf::from(&self.db_path);
        if db.is_absolute() {
            db
        } else {
            config_dir.join(db)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn yaml_file(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn defaults_when_no_file_no_cli() {
        let config = AppConfig::from_cli(Cli::default()).unwrap();
        assert_eq!(config.db_path, DEFAULT_DB_PATH);
        assert_eq!(config.scan.metadata.batch_size, DEFAULT_METADATA_BATCH_SIZE);
        assert_eq!(config.scan.hash.batch_size, DEFAULT_HASH_BATCH_SIZE);
    }

    #[test]
    fn file_overrides_defaults() {
        let f = yaml_file("db_path: /from/file.db\nscan:\n  metadata:\n    batch_size: 999\n");
        let cli = Cli {
            config: Some(f.path().to_path_buf()),
            ..Default::default()
        };

        let config = AppConfig::from_cli(cli).unwrap();
        assert_eq!(config.db_path, "/from/file.db");
        assert_eq!(config.scan.metadata.batch_size, 999);
        assert_eq!(config.scan.hash.batch_size, DEFAULT_HASH_BATCH_SIZE);
    }

    #[test]
    fn cli_overrides_file() {
        let f = yaml_file("db_path: /from/file.db\nscan:\n  metadata:\n    batch_size: 999\n");
        let cli = Cli {
            config: Some(f.path().to_path_buf()),
            db_path: Some("/from/cli.db".into()),
            scan: CliScan {
                metadata: CliMetadataBatch {
                    metadata_batch_size: Some(1),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let config = AppConfig::from_cli(cli).unwrap();
        assert_eq!(config.db_path, "/from/cli.db");
        assert_eq!(config.scan.metadata.batch_size, 1);
    }
}
