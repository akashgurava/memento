//! Hash scan — compute file fingerprints.
//!
//! Computes one hash algorithm per invocation across all files that don't yet
//! have that hash. Supported algorithms:
//! - `blake3` — full-file BLAKE3 (all file types)
//! - `content_blake3` — pixel-only BLAKE3 (images only)
//! - `phash`, `dhash`, `whash` — perceptual hashes (images only, 64-bit)
//!
//! Hashing is parallelized via rayon; results are persisted by the
//! [`HashScanStore`] implementation (which manages its own batching/locking).
//! Files are processed smallest-first to maximize throughput.

use std::time::Instant;

use rayon::prelude::*;
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::error::{Result, ScanError};
use crate::hashing::{self, HashAlgo};
use crate::scanner::progress::{ProgressReporter, ScanProgress};
use crate::scanner::store::HashScanStore;

/// Run hash scan for a specific algorithm.
///
/// Queries the store for files missing the specified hash, computes hashes in
/// parallel batches, and persists results. Perceptual hashes are stored as
/// 64-bit integers (for fast Hamming distance via XOR + popcount); cryptographic
/// hashes are stored as hex strings.
pub fn run_hash_scan(
    config: &AppConfig,
    store: &dyn HashScanStore,
    scan_run_id: i64,
    hash_type: &str,
    reporter: &dyn ProgressReporter,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let start = Instant::now();
    let algo = HashAlgo::parse(hash_type)?;

    let file_type_filter = match algo {
        HashAlgo::PHash | HashAlgo::DHash | HashAlgo::WHash | HashAlgo::ContentBlake3 => {
            Some("image")
        }
        HashAlgo::Blake3Full => None,
    };

    let files_to_hash = store.get_files_needing_hash(hash_type, file_type_filter)?;

    let total = files_to_hash.len() as i64;
    tracing::info!("HASH_SCAN: START. hash_type: {}, files: {}", hash_type, total);
    let processed = std::sync::atomic::AtomicI64::new(0);
    let batch_size = config.scan.hash.batch_size;

    let parallelism = if config.scan.hash.parallelism == 0 {
        rayon::current_num_threads()
    } else {
        config.scan.hash.parallelism
    };

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(parallelism)
        .build()
        .map_err(ScanError::thread_pool_build)?;

    let report_interval = 50.min(batch_size);

    for batch in files_to_hash.chunks(batch_size) {
        if cancel_token.is_cancelled() {
            return Err(ScanError::cancelled());
        }

        for sub_batch in batch.chunks(report_interval) {
            if cancel_token.is_cancelled() {
                return Err(ScanError::cancelled());
            }

            let results: Vec<(i64, std::result::Result<hashing::HashResult, String>)> =
                pool.install(|| {
                    sub_batch
                        .par_iter()
                        .map(|(file_id, path)| {
                            let result =
                                hashing::compute_hash(&algo, path).map_err(|e| e.to_string());
                            (*file_id, result)
                        })
                        .collect()
                });

            store.persist_hash_batch(hash_type, &results)?;

            let count = processed
                .fetch_add(sub_batch.len() as i64, std::sync::atomic::Ordering::Relaxed)
                + sub_batch.len() as i64;

            reporter.report(&ScanProgress {
                scan_run_id,
                stage: "hash".into(),
                hash_type: Some(hash_type.to_string()),
                status: "running".into(),
                files_processed: count,
                files_total: Some(total),
                current_path: sub_batch.last().map(|(_, p)| p.clone()),
                elapsed_secs: start.elapsed().as_secs_f64(),
                error: None,
            });
        }
    }

    tracing::info!(
        "HASH_SCAN: SUCCESS. hash_type: {}, files_processed: {}, elapsed_secs: {:.2}",
        hash_type, total, start.elapsed().as_secs_f64()
    );

    reporter.report(&ScanProgress {
        scan_run_id,
        stage: "hash".into(),
        hash_type: Some(hash_type.to_string()),
        status: "completed".into(),
        files_processed: total,
        files_total: Some(total),
        current_path: None,
        elapsed_secs: start.elapsed().as_secs_f64(),
        error: None,
    });

    Ok(())
}
