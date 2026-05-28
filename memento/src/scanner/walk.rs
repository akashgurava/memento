//! Filesystem walking and file classification utilities.
//!
//! Provides a directory walker built on the [`ignore`] crate (respects .gitignore
//! patterns but configured to include hidden files) and helpers for classifying
//! files by extension and normalizing paths cross-platform.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::config::AppConfig;

/// A single file discovered during a directory walk.
///
/// Contains the metadata needed for change detection (size + mtime) without
/// reading file contents.
#[derive(Debug, Clone)]
pub struct WalkEntry {
    pub path: String,
    pub size_bytes: u64,
    pub mtime_secs: i64,
    pub mtime_nanos: i32,
    pub is_file: bool,
}

/// Classify a file extension into a type based on config
pub fn classify_extension(ext: &str, config: &AppConfig) -> &'static str {
    let ext_lower = ext.to_lowercase();
    if config.scan.image_extensions.iter().any(|e| e == &ext_lower) {
        "image"
    } else if config.scan.video_extensions.iter().any(|e| e == &ext_lower) {
        "video"
    } else {
        "other"
    }
}

/// Normalize a path to use forward slashes on all platforms.
/// This ensures paths stored in the DB are consistent cross-platform.
pub fn normalize_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.to_string()
    }
}

/// Expand a leading `~` to the user's home directory.
/// Returns the path unchanged if it doesn't start with `~`.
pub fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(path))
    } else if let Some(rest) = path.strip_prefix("~/") {
        dirs::home_dir()
            .map(|home| home.join(rest))
            .unwrap_or_else(|| PathBuf::from(path))
    } else {
        PathBuf::from(path)
    }
}

/// Walk a directory tree and collect file entries
pub fn walk_directory(root: &Path) -> Vec<WalkEntry> {
    let mut entries = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(false) // don't skip hidden files — photos can be in hidden dirs
        .git_ignore(false) // not a git repo
        .git_global(false)
        .git_exclude(false)
        .build();

    for result in walker {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if !metadata.is_file() {
            continue;
        }

        let path = normalize_path(entry.path());
        let size_bytes = metadata.len();

        // Extract mtime
        let (mtime_secs, mtime_nanos) = match metadata.modified() {
            Ok(mtime) => {
                let duration = mtime
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                (duration.as_secs() as i64, duration.subsec_nanos() as i32)
            }
            Err(_) => (0, 0),
        };

        entries.push(WalkEntry {
            path,
            size_bytes,
            mtime_secs,
            mtime_nanos,
            is_file: true,
        });
    }

    entries
}
