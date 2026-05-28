use std::time::Instant;

use rayon::prelude::*;
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::db::hashes::{set_hash_impl, set_perceptual_hash_impl};
use crate::db::{Db, HashRepository};
use crate::error::{Result, ScanError};
use crate::hashing::{self, HashAlgo};
use crate::scanner::progress::{ProgressReporter, ScanProgress};

/// Run Level 3 hash scan for a specific algorithm
pub fn run_hash_scan(
    config: &AppConfig,
    db: &Db,
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

    let files_to_hash = db.get_files_needing_hash(hash_type, file_type_filter)?;

    let total = files_to_hash.len() as i64;
    let processed = std::sync::atomic::AtomicI64::new(0);
    let batch_size = config.scan.level3.batch_size;

    let parallelism = if config.scan.level3.parallelism == 0 {
        rayon::current_num_threads()
    } else {
        config.scan.level3.parallelism
    };

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(parallelism)
        .build()
        .map_err(ScanError::lock_failed)?;

    for batch in files_to_hash.chunks(batch_size) {
        if cancel_token.is_cancelled() {
            return Err(ScanError::cancelled());
        }

        let results: Vec<(i64, std::result::Result<hashing::HashResult, String>)> =
            pool.install(|| {
                batch
                    .par_iter()
                    .map(|(file_id, path)| {
                        let result = hashing::compute_hash(&algo, path).map_err(|e| e.to_string());
                        (*file_id, result)
                    })
                    .collect()
            });

        let conn = db.conn()?;
        for (file_id, result) in &results {
            match result {
                Ok(hash_result) => match hash_result {
                    hashing::HashResult::Hex(hex) => {
                        set_hash_impl(&conn, *file_id, hash_type, hex)?;
                    }
                    hashing::HashResult::Perceptual(value) => {
                        set_perceptual_hash_impl(&conn, *file_id, hash_type, *value)?;
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to hash file {}: {}", file_id, e);
                }
            }
        }

        let count = processed.fetch_add(batch.len() as i64, std::sync::atomic::Ordering::Relaxed)
            + batch.len() as i64;

        reporter.report(&ScanProgress {
            scan_run_id,
            level: 3,
            hash_type: Some(hash_type.to_string()),
            status: "running".into(),
            files_processed: count,
            files_total: Some(total),
            current_path: None,
            elapsed_secs: start.elapsed().as_secs_f64(),
            error: None,
        });
    }

    reporter.report(&ScanProgress {
        scan_run_id,
        level: 3,
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
