use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    pub scan_run_id: i64,
    pub level: u8,
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
