//! Persistence traits for scan operations.
//!
//! Defines the storage contracts that scanners need without depending on
//! the concrete `db` module. Implementations live in `db/scan_store_impl.rs`.

use crate::error::Result;
use crate::hashing::HashResult;
use crate::metadata::MetadataEntry;

/// A file entry discovered during the stats scan, ready to be persisted.
#[derive(Debug, Clone)]
pub struct StatEntry {
    pub path: String,
    pub filename: String,
    pub extension: Option<String>,
    pub size_bytes: i64,
    pub mtime_secs: i64,
    pub mtime_nanos: i32,
    pub file_type: String,
}

/// Persistence operations needed by the stats scan.
pub trait StatsScanStore: Send + Sync {
    /// Get all known (non-missing) file paths for a root directory.
    fn get_known_paths_for_root(&self, root: &str) -> Result<Vec<String>>;

    /// Upsert a batch of file entries. If a file's mtime/size changed,
    /// the implementation should invalidate its hashes.
    fn upsert_file_batch(&self, root: &str, entries: &[StatEntry]) -> Result<()>;

    /// Mark files as missing (no longer on disk).
    fn mark_missing_batch(&self, paths: &[&str]) -> Result<()>;
}

/// Persistence operations needed by the metadata scan.
pub trait MetadataScanStore: Send + Sync {
    /// Get files that need metadata extraction.
    /// Returns `(file_id, path, file_type)` for files without metadata.
    fn get_files_needing_metadata(&self) -> Result<Vec<(i64, String, String)>>;

    /// Persist a batch of extracted metadata entries.
    fn persist_metadata_batch(&self, entries: &[(i64, Vec<MetadataEntry>)]) -> Result<()>;
}

/// Persistence operations needed by the hash scan.
pub trait HashScanStore: Send + Sync {
    /// Get files that don't yet have the specified hash computed.
    /// Returns `(file_id, path)` pairs ordered by size ascending.
    fn get_files_needing_hash(
        &self,
        hash_type: &str,
        filter: Option<&str>,
    ) -> Result<Vec<(i64, String)>>;

    /// Persist a batch of hash results. Logs warnings for per-file errors;
    /// returns `Err` only on fatal store failures.
    fn persist_hash_batch(
        &self,
        hash_type: &str,
        results: &[(i64, std::result::Result<HashResult, String>)],
    ) -> Result<()>;
}
