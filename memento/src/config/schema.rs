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
    pub level2: Level2Config,
    #[serde(default)]
    pub level3: Level3Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level2Config {
    #[serde(default = "default_batch_size_500")]
    pub batch_size: usize,
    #[serde(default)]
    pub parallelism: usize, // 0 = all cores
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level3Config {
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
            level2: Level2Config::default(),
            level3: Level3Config::default(),
        }
    }
}

impl Default for Level2Config {
    fn default() -> Self {
        Self {
            batch_size: 500,
            parallelism: 0,
        }
    }
}

impl Default for Level3Config {
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
