use std::path::Path;
use std::time::Instant;

use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::error::Result;
use crate::scanner::progress::{ProgressReporter, ScanProgress};
use crate::scanner::walk::{classify_extension, walk_directory};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LibraryStats {
    pub total_files: i64,
    pub total_size_bytes: i64,
    pub image_count: i64,
    pub image_size_bytes: i64,
    pub video_count: i64,
    pub video_size_bytes: i64,
    pub other_count: i64,
    pub other_size_bytes: i64,
}

/// Run Level 1 stats scan across all configured roots
pub fn run_stats_scan(
    config: &AppConfig,
    scan_run_id: i64,
    reporter: &dyn ProgressReporter,
    cancel_token: &CancellationToken,
) -> Result<LibraryStats> {
    let start = Instant::now();
    let mut stats = LibraryStats {
        total_files: 0,
        total_size_bytes: 0,
        image_count: 0,
        image_size_bytes: 0,
        video_count: 0,
        video_size_bytes: 0,
        other_count: 0,
        other_size_bytes: 0,
    };

    for root in &config.scan.roots {
        if cancel_token.is_cancelled() {
            return Err(crate::error::ScanError::cancelled());
        }

        let root_path = Path::new(root);
        if !root_path.exists() {
            tracing::warn!("Scan root does not exist: {}", root);
            continue;
        }

        let entries = walk_directory(root_path);

        for entry in &entries {
            if cancel_token.is_cancelled() {
                return Err(crate::error::ScanError::cancelled());
            }

            stats.total_files += 1;
            stats.total_size_bytes += entry.size_bytes as i64;

            let ext = Path::new(&entry.path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            match classify_extension(ext, config) {
                "image" => {
                    stats.image_count += 1;
                    stats.image_size_bytes += entry.size_bytes as i64;
                }
                "video" => {
                    stats.video_count += 1;
                    stats.video_size_bytes += entry.size_bytes as i64;
                }
                _ => {
                    stats.other_count += 1;
                    stats.other_size_bytes += entry.size_bytes as i64;
                }
            }

            if stats.total_files % 1000 == 0 {
                reporter.report(&ScanProgress {
                    scan_run_id,
                    level: 1,
                    hash_type: None,
                    status: "running".into(),
                    files_processed: stats.total_files,
                    files_total: None,
                    current_path: Some(entry.path.clone()),
                    elapsed_secs: start.elapsed().as_secs_f64(),
                    error: None,
                });
            }
        }
    }

    reporter.report(&ScanProgress {
        scan_run_id,
        level: 1,
        hash_type: None,
        status: "completed".into(),
        files_processed: stats.total_files,
        files_total: Some(stats.total_files),
        current_path: None,
        elapsed_secs: start.elapsed().as_secs_f64(),
        error: None,
    });

    Ok(stats)
}
