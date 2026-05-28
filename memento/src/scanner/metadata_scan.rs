//! Metadata scan — extract and store file metadata.
//!
//! For files that don't yet have metadata extracted, reads EXIF/XMP/IPTC/video
//! tags and persists them to the EAV `file_metadata` table.
//!
//! Metadata extraction runs in parallel (rayon); persistence is handled by the
//! [`MetadataScanStore`] implementation (which manages its own batching/locking).

use std::time::Instant;

use rayon::prelude::*;
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::error::{Result, ScanError};
use crate::metadata;
use crate::scanner::progress::{ProgressReporter, ScanProgress};
use crate::scanner::store::MetadataScanStore;

/// Run metadata scan for files that need metadata extraction.
///
/// Queries the store for files without metadata, extracts tags in parallel,
/// and persists results in batches.
pub fn run_metadata_scan(
    config: &AppConfig,
    store: &dyn MetadataScanStore,
    scan_run_id: i64,
    reporter: &dyn ProgressReporter,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let start = Instant::now();
    let batch_size = config.scan.metadata.batch_size;

    let parallelism = if config.scan.metadata.parallelism == 0 {
        rayon::current_num_threads()
    } else {
        config.scan.metadata.parallelism
    };

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(parallelism)
        .build()
        .map_err(ScanError::thread_pool_build)?;

    let files = store.get_files_needing_metadata()?;
    let total = files.len() as i64;
    tracing::info!("METADATA_SCAN: START. files: {}, parallelism: {}", total, parallelism);
    let mut processed: i64 = 0;
    let report_interval = 50.min(batch_size);

    for chunk in files.chunks(batch_size) {
        if cancel_token.is_cancelled() {
            return Err(ScanError::cancelled());
        }

        // Process in smaller sub-chunks for progress reporting
        for sub_chunk in chunk.chunks(report_interval) {
            if cancel_token.is_cancelled() {
                return Err(ScanError::cancelled());
            }

            let extracted: Vec<(i64, Vec<_>)> = pool.install(|| {
                sub_chunk
                    .par_iter()
                    .map(|(file_id, path, file_type)| {
                        let entries = metadata::extract_metadata(path, file_type);
                        (*file_id, entries)
                    })
                    .collect()
            });

            store.persist_metadata_batch(&extracted)?;

            processed += sub_chunk.len() as i64;

            reporter.report(&ScanProgress {
                scan_run_id,
                stage: "metadata".into(),
                hash_type: None,
                status: "running".into(),
                files_processed: processed,
                files_total: Some(total),
                current_path: sub_chunk.last().map(|(_, path, _)| path.clone()),
                elapsed_secs: start.elapsed().as_secs_f64(),
                error: None,
            });
        }
    }

    reporter.report(&ScanProgress {
        scan_run_id,
        stage: "metadata".into(),
        hash_type: None,
        status: "completed".into(),
        files_processed: processed,
        files_total: Some(total),
        current_path: None,
        elapsed_secs: start.elapsed().as_secs_f64(),
        error: None,
    });

    tracing::info!(
        "METADATA_SCAN: SUCCESS. files_processed: {}, elapsed_secs: {:.2}",
        processed, start.elapsed().as_secs_f64()
    );

    Ok(())
}
