//! Progress reporting abstraction for scan operations.
//!
//! Defines the [`ProgressReporter`] trait that decouples scan logic from the
//! presentation layer. Implementations exist for Tauri events, CLI terminal
//! output, and no-op (tests).

use serde::Serialize;

/// Real-time progress snapshot emitted during any scan stage.
///
/// Serializable so Tauri can forward it directly as a JSON event payload.
#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    pub scan_run_id: i64,
    /// Scan stage: "stats", "metadata", or "hash"
    pub stage: String,
    pub hash_type: Option<String>,
    pub status: String,
    pub files_processed: i64,
    pub files_total: Option<i64>,
    pub current_path: Option<String>,
    pub elapsed_secs: f64,
    pub error: Option<String>,
}

/// Trait for reporting scan progress. Implementations bridge to Tauri events, terminal output, etc.
pub trait ProgressReporter: Send + Sync {
    fn report(&self, progress: &ScanProgress);
}

/// No-op reporter for when progress isn't needed (e.g. tests)
pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn report(&self, _progress: &ScanProgress) {}
}
