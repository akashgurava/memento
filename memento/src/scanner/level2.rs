use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use duckdb::Connection;
use rayon::prelude::*;
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::db::queries;
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
    db: &Arc<Mutex<Connection>>,
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
            let conn = db.lock().map_err(ScanError::lock_failed)?;

            let mut stmt = conn.prepare(
                "SELECT id, path, size_bytes, mtime_secs, mtime_nanos FROM files WHERE root_dir = ? AND is_missing = false"
            )?;
            let db_rows: Vec<(i64, String, i64, i64, i32)> = stmt
                .query_map([root], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut changes: Vec<FileChange> = Vec::new();
            let mut seen_paths: HashMap<&str, bool> = HashMap::new();

            for (_id, path, size, mtime_s, mtime_n) in &db_rows {
                if let Some(entry) = walk_map.get(path.as_str()) {
                    seen_paths.insert(path.as_str(), true);
                    if entry.size_bytes as i64 != *size
                        || entry.mtime_secs != *mtime_s
                        || entry.mtime_nanos != *mtime_n
                    {
                        changes.push(FileChange::Modified((*entry).clone()));
                    }
                } else {
                    queries::mark_missing(&conn, path)?;
                }
            }

            for entry in &walk_entries {
                if !seen_paths.contains_key(entry.path.as_str()) {
                    let exists: bool = conn
                        .prepare("SELECT COUNT(*) > 0 FROM files WHERE path = ?")?
                        .query_row([&entry.path], |row| row.get(0))
                        .unwrap_or(false);

                    if !exists {
                        changes.push(FileChange::New(entry.clone()));
                    }
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
            let conn = db.lock().map_err(ScanError::lock_failed)?;

            for (change, filename, extension, file_type, metadata_entries) in &extracted {
                let entry = change.entry();

                let file_id = queries::upsert_file(
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
                    queries::invalidate_hashes(&conn, file_id)?;
                }

                if !metadata_entries.is_empty() {
                    queries::insert_metadata_batch(&conn, file_id, metadata_entries)?;
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
