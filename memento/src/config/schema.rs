use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub db_path: Option<String>,
    #[serde(default)]
    pub scan: ScanConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default = "default_image_extensions")]
    pub image_extensions: Vec<String>,
    #[serde(default = "default_video_extensions")]
    pub video_extensions: Vec<String>,
    #[serde(default)]
    pub metadata: MetadataScanConfig,
    #[serde(default)]
    pub hash: HashScanConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataScanConfig {
    #[serde(default = "default_batch_size_500")]
    pub batch_size: usize,
    #[serde(default)]
    pub parallelism: usize, // 0 = all cores
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashScanConfig {
    #[serde(default = "default_batch_size_100")]
    pub batch_size: usize,
    #[serde(default)]
    pub parallelism: usize, // 0 = all cores
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            db_path: None,
            scan: ScanConfig::default(),
        }
    }
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            image_extensions: default_image_extensions(),
            video_extensions: default_video_extensions(),
            metadata: MetadataScanConfig::default(),
            hash: HashScanConfig::default(),
        }
    }
}

impl Default for MetadataScanConfig {
    fn default() -> Self {
        Self {
            batch_size: 500,
            parallelism: 0,
        }
    }
}

impl Default for HashScanConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            parallelism: 0,
        }
    }
}

fn default_schema_version() -> u32 {
    1
}

fn default_batch_size_500() -> usize {
    500
}

fn default_batch_size_100() -> usize {
    100
}

fn default_image_extensions() -> Vec<String> {
    [
        "jpg", "jpeg", "png", "tiff", "tif", "heic", "heif", "raw", "cr2", "cr3", "nef", "arw",
        "orf", "rw2", "dng", "webp", "avif", "gif", "bmp", "psd",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_video_extensions() -> Vec<String> {
    [
        "mp4", "mov", "avi", "mkv", "m4v", "wmv", "flv", "webm", "3gp", "mts", "m2ts", "ts",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
