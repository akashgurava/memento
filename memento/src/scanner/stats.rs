//! Stats scan — filesystem indexing.
//!
//! Walks all configured roots, upserts every discovered file into the store,
//! marks missing files, and returns aggregated counts and sizes by type.

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::error::Result;
use crate::scanner::progress::{ProgressReporter, ScanProgress};
use crate::scanner::store::{StatEntry, StatsScanStore};
use crate::scanner::walk::{classify_extension, expand_tilde, walk_directory};

use serde::Serialize;

/// Aggregated file counts and sizes returned by a stats scan.
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

/// Run stats scan across all configured roots.
///
/// Walks each root directory, upserts all files into the store, marks files
/// no longer on disk as missing, and returns aggregated stats.
pub fn run_stats_scan(
    config: &AppConfig,
    store: &dyn StatsScanStore,
    scan_run_id: i64,
    reporter: &dyn ProgressReporter,
    cancel_token: &CancellationToken,
) -> Result<LibraryStats> {
    tracing::info!("STATS_SCAN: START. roots: {:?}", config.scan.roots);
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

        let root_path = expand_tilde(root);
        if !root_path.exists() {
            tracing::warn!("STATS_SCAN_ROOT: SKIPPED. root: {}, reason: path does not exist", root);
            continue;
        }
        let root_str = root_path.to_string_lossy().to_string();

        // Walk filesystem
        let entries = walk_directory(&root_path);

        // Build StatEntry batch and accumulate stats
        let mut stat_entries: Vec<StatEntry> = Vec::with_capacity(entries.len());
        let mut seen_paths: HashSet<String> = HashSet::with_capacity(entries.len());

        for entry in &entries {
            if cancel_token.is_cancelled() {
                return Err(crate::error::ScanError::cancelled());
            }

            let ext = Path::new(&entry.path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let file_type = classify_extension(ext, config);

            stats.total_files += 1;
            stats.total_size_bytes += entry.size_bytes as i64;
            match file_type {
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

            let filename = Path::new(&entry.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let extension = Path::new(&entry.path)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());

            seen_paths.insert(entry.path.clone());
            stat_entries.push(StatEntry {
                path: entry.path.clone(),
                filename,
                extension,
                size_bytes: entry.size_bytes as i64,
                mtime_secs: entry.mtime_secs,
                mtime_nanos: entry.mtime_nanos,
                file_type: file_type.to_string(),
            });

            if stats.total_files % 1000 == 0 {
                reporter.report(&ScanProgress {
                    scan_run_id,
                    stage: "stats".into(),
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

        // Persist to store
        store.upsert_file_batch(&root_str, &stat_entries)?;

        // Mark missing files
        let known_paths = store.get_known_paths_for_root(&root_str)?;
        let missing: Vec<&str> = known_paths
            .iter()
            .filter(|p| !seen_paths.contains(p.as_str()))
            .map(|p| p.as_str())
            .collect();
        if !missing.is_empty() {
            store.mark_missing_batch(&missing)?;
        }
    }

    reporter.report(&ScanProgress {
        scan_run_id,
        stage: "stats".into(),
        hash_type: None,
        status: "completed".into(),
        files_processed: stats.total_files,
        files_total: Some(stats.total_files),
        current_path: None,
        elapsed_secs: start.elapsed().as_secs_f64(),
        error: None,
    });

    tracing::info!(
        "STATS_SCAN: SUCCESS. total_files: {}, images: {}, videos: {}, elapsed_secs: {:.2}",
        stats.total_files, stats.image_count, stats.video_count, start.elapsed().as_secs_f64()
    );

    Ok(stats)
}
