use std::{
    collections::HashSet,
    fs::File,
    path::{Path, PathBuf},
};

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
/// Configuration for parallel processing.
#[serde(default)]
struct MetadataBatchConfig {
    batch_size: u32,
    #[serde(default)]
    parallelism: u32,
}

impl Default for MetadataBatchConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_METADATA_BATCH_SIZE,
            parallelism: DEFAULT_PARALLELISM,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
/// Configuration for parallel processing.
struct HashBatchConfig {
    batch_size: u32,
    parallelism: u32,
}

impl Default for HashBatchConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_HASH_BATCH_SIZE,
            parallelism: DEFAULT_PARALLELISM,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
/// Configuration for scan operations.
struct ScanConfig {
    /// Root directories to scan.
    roots: Vec<String>,
    /// Extensions to consider as images.
    image_extensions: HashSet<String>,
    /// Extensions to consider as videos.
    video_extensions: HashSet<String>,
    /// Parallel processing configuration for metadata extraction.
    metadata: MetadataBatchConfig,
    /// Parallel processing configuration for file hashing.
    hash: HashBatchConfig,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            image_extensions: DEFAULT_PHOTO_EXT.iter().map(|s| (*s).to_owned()).collect(),
            video_extensions: DEFAULT_VIDEO_EXT.iter().map(|s| (*s).to_owned()).collect(),
            metadata: MetadataBatchConfig::default(),
            hash: HashBatchConfig::default(),
        }
    }
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
/// Application configuration.
/// This resolves to the final configuration after loading and merging.
///
/// Order of precedence:
/// 1. CLI Args(If "cli" feature enabled and running as CLI)
/// 2. User configuration file(defaults to ./memento.yaml in cli)
/// 3. Default values
pub struct AppConfig {
    schema_version: u32,
    db_path: PathBuf,
    scan: ScanConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            db_path: PathBuf::from(DEFAULT_DB_PATH),
            scan: ScanConfig::default(),
        }
    }
}

impl AppConfig {
    /// Load configuration from a YAML file.
    /// If file is missing some fields, they will be filled with default values.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, MementoError> {
        serde_yaml::from_reader(File::open(path.as_ref()).map_err(|e| {
            MementoError::config_error(path.as_ref().to_string_lossy().to_string(), e.to_string())
        })?)
        .map_err(|e| {
            MementoError::config_error(path.as_ref().to_string_lossy().to_string(), e.to_string())
        })
    }

    /// Set the root directories to scan.
    pub fn set_roots(&mut self, roots: Vec<String>) {
        self.scan.roots = roots;
    }

    /// Set the database path.
    pub fn set_db_path(&mut self, db_path: PathBuf) {
        self.db_path = db_path;
    }

    /// Set the image extensions.
    pub fn set_image_extensions(&mut self, extensions: HashSet<String>) {
        self.scan.image_extensions = extensions;
    }

    /// Set the video extensions.
    pub fn set_video_extensions(&mut self, extensions: HashSet<String>) {
        self.scan.video_extensions = extensions;
    }

    pub fn roots(&self) -> Result<Vec<PathBuf>, MementoError> {
        self.scan
            .roots
            .iter()
            .map(|r| {
                shellexpand::full(r)
                    .map(|expanded| PathBuf::from(expanded.into_owned()))
                    .map_err(|e| MementoError::config_error(r.clone(), e.to_string()))
            })
            .collect()
    }

    pub fn image_extensions(&self) -> &HashSet<String> {
        &self.scan.image_extensions
    }

    pub fn video_extensions(&self) -> &HashSet<String> {
        &self.scan.video_extensions
    }

    pub fn db_path(&self) -> PathBuf {
        PathBuf::from(&self.db_path)
    }
}

// -- CLI (feature-gated) -----------------------------------------------------

#[cfg(feature = "cli")]
#[derive(clap::Args, Default)]
pub struct CliMetadataBatch {
    /// Metadata scan batch size
    #[arg(long = "metadata-batch-size")]
    pub metadata_batch_size: Option<u32>,

    /// Metadata scan parallelism (0 = all cores)
    #[arg(long = "metadata-parallelism")]
    pub metadata_parallelism: Option<u32>,
}

#[cfg(feature = "cli")]
#[derive(clap::Args, Default)]
pub struct CliHashBatch {
    /// Hash scan batch size
    #[arg(long = "hash-batch-size")]
    pub hash_batch_size: Option<u32>,

    /// Hash scan parallelism (0 = all cores)
    #[arg(long = "hash-parallelism")]
    pub hash_parallelism: Option<u32>,
}

#[cfg(feature = "cli")]
#[derive(clap::Parser, Default)]
#[command(name = "memento", about = "Photo library deduplication engine")]
pub struct Cli {
    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Path to database file
    #[arg(long)]
    db_path: Option<PathBuf>,

    #[command(flatten)]
    scan: CliScan,
}

#[cfg(feature = "cli")]
#[derive(clap::Args, Default)]
struct CliScan {
    /// Scan root directories
    #[arg(long)]
    roots: Option<Vec<String>>,

    #[command(flatten)]
    metadata: CliMetadataBatch,

    #[command(flatten)]
    hash: CliHashBatch,
}

impl AppConfig {
    #[cfg(feature = "cli")]
    pub fn from_cli(cli: Cli) -> Result<Self, MementoError> {
        let mut config = match cli.config {
            Some(ref path) => Self::from_file(path)?,
            None => Self::default(),
        };

        if let Some(db_path) = cli.db_path {
            config.db_path = db_path;
        }
        if let Some(roots) = cli.scan.roots {
            config.scan.roots = roots;
        }
        if let Some(v) = cli.scan.metadata.metadata_batch_size {
            config.scan.metadata.batch_size = v;
        }
        if let Some(v) = cli.scan.metadata.metadata_parallelism {
            config.scan.metadata.parallelism = v;
        }
        if let Some(v) = cli.scan.hash.hash_batch_size {
            config.scan.hash.batch_size = v;
        }
        if let Some(v) = cli.scan.hash.hash_parallelism {
            config.scan.hash.parallelism = v;
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn yaml_file(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("Failed to create temp file");
        f.write_all(content.as_bytes())
            .expect("Failed to write to temp file");
        f
    }

    #[test]
    fn from_file_fills_defaults_for_missing_fields() {
        let f = yaml_file("schema_version: 1\n");
        let config = AppConfig::from_file(f).unwrap();

        assert_eq!(config.db_path, PathBuf::from(DEFAULT_DB_PATH));
        assert_eq!(config.scan.roots, Vec::<String>::new());
        assert_eq!(config.scan.image_extensions.len(), DEFAULT_PHOTO_EXT.len());
        assert_eq!(config.scan.video_extensions.len(), DEFAULT_VIDEO_EXT.len());
        assert_eq!(config.scan.metadata.batch_size, DEFAULT_METADATA_BATCH_SIZE);
        assert_eq!(config.scan.metadata.parallelism, DEFAULT_PARALLELISM);
        assert_eq!(config.scan.hash.batch_size, DEFAULT_HASH_BATCH_SIZE);
        assert_eq!(config.scan.hash.parallelism, DEFAULT_PARALLELISM);
    }

    #[test]
    fn from_file_partial_override_preserves_sibling_defaults() {
        let f = yaml_file("db_path: /custom.db\nscan:\n  metadata:\n    batch_size: 999\n");
        let config = AppConfig::from_file(f).unwrap();

        assert_eq!(config.db_path, PathBuf::from("/custom.db"));
        assert_eq!(config.scan.metadata.batch_size, 999);
        assert_eq!(config.scan.metadata.parallelism, DEFAULT_PARALLELISM);
        assert_eq!(config.scan.hash.batch_size, DEFAULT_HASH_BATCH_SIZE);
        assert_eq!(config.scan.hash.parallelism, DEFAULT_PARALLELISM);
        assert_eq!(config.scan.image_extensions.len(), DEFAULT_PHOTO_EXT.len());
    }

    #[test]
    fn from_file_custom_roots_and_extensions() {
        let f = yaml_file(
            "scan:\n  roots:\n    - /photos\n    - /backup\n  image_extensions:\n    - jpg\n    - png\n  video_extensions:\n    - mp4\n",
        );
        let config = AppConfig::from_file(f).unwrap();

        assert_eq!(config.scan.roots, vec!["/photos", "/backup"]);
        assert_eq!(
            config.scan.image_extensions,
            HashSet::from(["jpg".to_owned(), "png".to_owned()])
        );
        assert_eq!(
            config.scan.video_extensions,
            HashSet::from(["mp4".to_owned()])
        );
    }

    #[test]
    fn from_file_nonexistent_path_returns_error() {
        let result = AppConfig::from_file("/nonexistent/path.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn from_file_invalid_yaml_returns_error() {
        let f = yaml_file("{{{{not valid yaml");
        let result = AppConfig::from_file(f);
        assert!(result.is_err());
    }

    #[test]
    fn roots_expands_tilde() {
        let f = yaml_file("scan:\n  roots:\n    - ~/Pictures\n");
        let config = AppConfig::from_file(f).unwrap();
        let roots = config.roots().unwrap();

        assert_eq!(roots.len(), 1);
        assert!(roots[0].is_absolute());
        assert!(!roots[0].to_string_lossy().contains('~'));
    }

    #[test]
    #[cfg(feature = "cli")]
    fn cli_defaults_match_app_defaults() {
        let config = AppConfig::from_cli(Cli::default()).unwrap();
        assert_eq!(config.db_path, PathBuf::from(DEFAULT_DB_PATH));
        assert_eq!(config.scan.metadata.batch_size, DEFAULT_METADATA_BATCH_SIZE);
        assert_eq!(config.scan.hash.batch_size, DEFAULT_HASH_BATCH_SIZE);
        assert_eq!(config.scan.metadata.parallelism, DEFAULT_PARALLELISM);
        assert_eq!(config.scan.hash.parallelism, DEFAULT_PARALLELISM);
    }

    #[test]
    #[cfg(feature = "cli")]
    fn cli_overrides_file_values() {
        let f = yaml_file(
            "db_path: /from/file.db\nscan:\n  roots:\n    - /original\n  metadata:\n    batch_size: 999\n    parallelism: 4\n",
        );
        let cli = Cli {
            config: Some(f.path().to_path_buf()),
            db_path: Some("/from/cli.db".into()),
            scan: CliScan {
                roots: Some(vec!["/override".into()]),
                metadata: CliMetadataBatch {
                    metadata_batch_size: Some(1),
                    metadata_parallelism: None,
                },
                hash: CliHashBatch {
                    hash_batch_size: Some(50),
                    hash_parallelism: Some(2),
                },
            },
        };

        let config = AppConfig::from_cli(cli).unwrap();
        assert_eq!(config.db_path, PathBuf::from("/from/cli.db"));
        assert_eq!(config.scan.roots, vec!["/override"]);
        assert_eq!(config.scan.metadata.batch_size, 1);
        assert_eq!(config.scan.metadata.parallelism, 4); // not overridden by CLI
        assert_eq!(config.scan.hash.batch_size, 50);
        assert_eq!(config.scan.hash.parallelism, 2);
    }

    #[test]
    #[cfg(feature = "cli")]
    fn cli_without_file_uses_defaults() {
        let cli = Cli {
            config: None,
            db_path: Some("/cli-only.db".into()),
            scan: CliScan::default(),
        };

        let config = AppConfig::from_cli(cli).unwrap();
        assert_eq!(config.db_path, PathBuf::from("/cli-only.db"));
        assert_eq!(config.scan.metadata.batch_size, DEFAULT_METADATA_BATCH_SIZE);
    }

    #[test]
    #[cfg(feature = "cli")]
    fn cli_none_values_dont_override_file() {
        let f = yaml_file("db_path: /from/file.db\nscan:\n  hash:\n    batch_size: 77\n");
        let cli = Cli {
            config: Some(f.path().to_path_buf()),
            db_path: None,
            scan: CliScan::default(),
        };

        let config = AppConfig::from_cli(cli).unwrap();
        assert_eq!(config.db_path, PathBuf::from("/from/file.db"));
        assert_eq!(config.scan.hash.batch_size, 77);
    }
}
