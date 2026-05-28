use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use rayon::prelude::*;
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::db::hashes::invalidate_hashes_impl;
use crate::db::metadata_repo::insert_metadata_batch_impl;
use crate::db::{files::upsert_file_impl, Db, FileRepository};
use crate::error::{Result, ScanError};
use crate::metadata;
use crate::scanner::progress::{ProgressReporter, ScanProgress};
use crate::scanner::walk::{classify_extension, walk_directory, WalkEntry};

#[derive(Debug, Clone)]
enum FileChange {
    New(WalkEntry),
    Modified(WalkEntry),
}

impl FileChange {
    fn entry(&self) -> &WalkEntry {
        match self {
            FileChange::New(e) | FileChange::Modified(e) => e,
        }
    }

    fn is_modified(&self) -> bool {
        matches!(self, FileChange::Modified(_))
    }
}

/// Run Level 2 incremental metadata scan
pub fn run_metadata_scan(
    config: &AppConfig,
    db: &Db,
    scan_run_id: i64,
    reporter: &dyn ProgressReporter,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let start = Instant::now();
    let batch_size = config.scan.level2.batch_size;

    let parallelism = if config.scan.level2.parallelism == 0 {
        rayon::current_num_threads()
    } else {
        config.scan.level2.parallelism
    };

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(parallelism)
        .build()
        .map_err(ScanError::lock_failed)?;

    for root in &config.scan.roots {
        if cancel_token.is_cancelled() {
            return Err(ScanError::cancelled());
        }

        let root_path = Path::new(root);
        if !root_path.exists() {
            tracing::warn!("Scan root does not exist: {}", root);
            continue;
        }

        // Phase A: Walk filesystem
        let walk_entries = walk_directory(root_path);
        let walk_map: HashMap<&str, &WalkEntry> =
            walk_entries.iter().map(|e| (e.path.as_str(), e)).collect();

        // Phase B: Compare against DB
        let changes = {
            let db_records = db.get_active_files_for_root(root)?;

            let mut changes: Vec<FileChange> = Vec::new();
            let mut seen_paths: HashMap<&str, bool> = HashMap::new();

            for record in &db_records {
                if let Some(entry) = walk_map.get(record.path.as_str()) {
                    seen_paths.insert(record.path.as_str(), true);
                    if entry.size_bytes as i64 != record.size_bytes
                        || entry.mtime_secs != record.mtime_secs
                        || entry.mtime_nanos != record.mtime_nanos
                    {
                        changes.push(FileChange::Modified((*entry).clone()));
                    }
                } else {
                    db.mark_missing(&record.path)?;
                }
            }

            for entry in &walk_entries {
                if !seen_paths.contains_key(entry.path.as_str()) && !db.file_exists(&entry.path)? {
                    changes.push(FileChange::New(entry.clone()));
                }
            }

            changes
        };

        // Phase C: Process changes in batches — extract metadata in parallel, write to DB serially
        let total_changes = changes.len() as i64;
        let mut processed: i64 = 0;

        for chunk in changes.chunks(batch_size) {
            if cancel_token.is_cancelled() {
                return Err(ScanError::cancelled());
            }

            // Parallel: classify and extract metadata for each file in the batch
            let extracted: Vec<_> = pool.install(|| {
                chunk
                    .par_iter()
                    .map(|change| {
                        let entry = change.entry();
                        let filename = Path::new(&entry.path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();
                        let extension = Path::new(&entry.path)
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_lowercase());
                        let file_type = extension
                            .as_deref()
                            .map(|ext| classify_extension(ext, config))
                            .unwrap_or("other");

                        let metadata_entries = metadata::extract_metadata(&entry.path, file_type);
                        (change, filename, extension, file_type, metadata_entries)
                    })
                    .collect()
            });

            // Serial: write results to DB (single-writer constraint)
            let conn = db.conn()?;

            for (change, filename, extension, file_type, metadata_entries) in &extracted {
                let entry = change.entry();

                let file_id = upsert_file_impl(
                    &conn,
                    &entry.path,
                    root,
                    filename,
                    extension.as_deref(),
                    entry.size_bytes as i64,
                    entry.mtime_secs,
                    entry.mtime_nanos,
                    file_type,
                )?;

                if change.is_modified() {
                    invalidate_hashes_impl(&conn, file_id)?;
                }

                if !metadata_entries.is_empty() {
                    insert_metadata_batch_impl(&conn, file_id, metadata_entries)?;
                }

                conn.execute(
                    "UPDATE files SET metadata_scanned_at = current_timestamp WHERE id = ?",
                    [file_id],
                )?;

                processed += 1;
            }

            reporter.report(&ScanProgress {
                scan_run_id,
                level: 2,
                hash_type: None,
                status: "running".into(),
                files_processed: processed,
                files_total: Some(total_changes),
                current_path: None,
                elapsed_secs: start.elapsed().as_secs_f64(),
                error: None,
            });
        }
    }

    reporter.report(&ScanProgress {
        scan_run_id,
        level: 2,
        hash_type: None,
        status: "completed".into(),
        files_processed: 0,
        files_total: None,
        current_path: None,
        elapsed_secs: start.elapsed().as_secs_f64(),
        error: None,
    });

    Ok(())
}
