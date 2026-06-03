use std::{any::Any, path::PathBuf};

/// An identifier for an error.
///
/// Provides structured context about which resource an error relates to.
/// For example: `Identifier { kind: "path", value: "/Photos/IMG_001.jpg" }`
#[derive(Debug)]
struct Identifier {
    kind: &'static str,
    value: String,
}

impl Identifier {
    fn new(kind: &'static str, value: impl ToString) -> Self {
        Self {
            kind,
            value: value.to_string(),
        }
    }

    fn kind(&self) -> &'static str {
        self.kind
    }

    fn value(&self) -> &str {
        &self.value
    }
}

/// Structured error context for logging and frontend display.
///
/// Contains an optional error description and an optional identifier.
/// Display format: `ERROR_ID. kind: value (error message)`
#[derive(Debug)]
struct ErrorContext {
    identifier: Option<Identifier>,
    error: Option<String>,
}

impl ErrorContext {
    /// Creates a context with both an error description and an identifier.
    fn new(kind: &'static str, value: impl ToString, error: Option<impl ToString>) -> Self {
        Self {
            identifier: Some(Identifier::new(kind, value)),
            error: error.map(|e| e.to_string()),
        }
    }

    /// Creates a context with only an error description, no identifier.
    fn error_only(error: impl ToString) -> Self {
        Self {
            identifier: None,
            error: Some(error.to_string()),
        }
    }

    /// Creates an empty context (no error, no identifier).
    fn empty() -> Self {
        Self {
            identifier: None,
            error: None,
        }
    }
}

/// Internal trait for converting error types into structured error info.
///
/// Each error type provides:
/// - A machine-readable error ID (e.g. `"DB_MIGRATION_FAILED"`)
/// - Structured context for logging/display
trait ErrorInfo {
    /// Returns a machine-readable error identifier (e.g. `"SCAN_CANCELLED"`).
    fn error_id(&self) -> &'static str;
    fn context(&self) -> ErrorContext;
}

macro_rules! impl_err_from_info {
    ($error:ty) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let ctx = self.context();
                f.write_str(self.error_id())?;
                if let Some(id) = ctx.identifier.as_ref() {
                    write!(f, ". {}: {}", id.kind(), id.value())?;
                }
                if let Some(error) = ctx.error.as_ref() {
                    write!(f, " ({error})")?;
                }
                Ok(())
            }
        }

        impl std::error::Error for $error {}
    };
}

// ----------------------------- Scan errors -----------------------------

#[derive(Debug)]
pub enum ScanError {
    Fs { path: PathBuf, error: String },
    Walk { path: PathBuf, error: String },
}

impl ErrorInfo for ScanError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Fs { .. } => "SCAN_FS_ERROR",
            Self::Walk { .. } => "SCAN_WALK_ERROR",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::Fs { path, error } => {
                ErrorContext::new("path", path.to_string_lossy(), Some(error))
            }
            Self::Walk { path, error } => {
                ErrorContext::new("path", path.to_string_lossy(), Some(error))
            }
        }
    }
}

impl_err_from_info!(ScanError);

// ----------------------------- Database errors -----------------------------

#[derive(Debug)]
pub enum DbError {
    Init {
        path: String,
        error: String,
    },
    LockFailed {
        error: String,
    },

    Migration {
        error: String,
    },
    Appender {
        table: String,
        error: String,
    },

    UpsertFile {
        path: String,
        error: String,
    },
    FileExists {
        path: String,
        error: String,
    },
    GetActiveFiles {
        root_dir: String,
        error: String,
    },
    SetHash {
        file_id: i64,
        hash_type: String,
        error: String,
    },
    SetPerceptualHash {
        file_id: i64,
        hash_type: String,
        error: String,
    },
    GetFilesNeedingHash {
        hash_type: String,
        error: String,
    },
    InsertMetadata {
        file_id: i64,
        error: String,
    },
    CreateScanRun {
        error: String,
    },
    UpdateScanProgress {
        scan_run_id: i64,
        error: String,
    },
    CompleteScanRun {
        scan_run_id: i64,
        error: String,
    },
    Query {
        operation: String,
        error: String,
    },
}

impl ErrorInfo for DbError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Init { .. } => "DB_INIT_FAILED",
            Self::Migration { .. } => "DB_MIGRATION_FAILED",
            Self::Appender { .. } => "DB_APPENDER_FAILED",
            Self::LockFailed { .. } => "DB_LOCK_FAILED",
            Self::UpsertFile { .. } => "DB_UPSERT_FILE_FAILED",
            Self::FileExists { .. } => "DB_FILE_EXISTS_FAILED",
            Self::GetActiveFiles { .. } => "DB_GET_ACTIVE_FILES_FAILED",
            Self::SetHash { .. } => "DB_SET_HASH_FAILED",
            Self::SetPerceptualHash { .. } => "DB_SET_PERCEPTUAL_HASH_FAILED",
            Self::GetFilesNeedingHash { .. } => "DB_GET_FILES_NEEDING_HASH_FAILED",
            Self::InsertMetadata { .. } => "DB_INSERT_METADATA_FAILED",
            Self::CreateScanRun { .. } => "DB_CREATE_SCAN_RUN_FAILED",
            Self::UpdateScanProgress { .. } => "DB_UPDATE_SCAN_PROGRESS_FAILED",
            Self::CompleteScanRun { .. } => "DB_COMPLETE_SCAN_RUN_FAILED",
            Self::Query { .. } => "DB_QUERY_FAILED",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::Init { path, error } => ErrorContext::new("path", path, Some(error)),
            Self::Migration { error } => ErrorContext::error_only(error),
            Self::Appender { table, error } => ErrorContext::new("table", table, Some(error)),
            Self::LockFailed { error } => ErrorContext::error_only(error),
            Self::UpsertFile { path, error } => ErrorContext::new("path", path, Some(error)),
            Self::FileExists { path, error } => ErrorContext::new("path", path, Some(error)),
            Self::GetActiveFiles { root_dir, error } => {
                ErrorContext::new("root_dir", root_dir, Some(error))
            }
            Self::SetHash {
                file_id,
                hash_type,
                error,
            } => ErrorContext::new("file_id", format!("{file_id}/{hash_type}"), Some(error)),
            Self::SetPerceptualHash {
                file_id,
                hash_type,
                error,
            } => ErrorContext::new("file_id", format!("{file_id}/{hash_type}"), Some(error)),
            Self::GetFilesNeedingHash { hash_type, error } => {
                ErrorContext::new("hash_type", hash_type, Some(error))
            }
            Self::InsertMetadata { file_id, error } => {
                ErrorContext::new("file_id", file_id.to_string(), Some(error))
            }
            Self::CreateScanRun { error } => ErrorContext::error_only(error),
            Self::UpdateScanProgress { scan_run_id, error } => {
                ErrorContext::new("scan_run_id", scan_run_id.to_string(), Some(error))
            }
            Self::CompleteScanRun { scan_run_id, error } => {
                ErrorContext::new("scan_run_id", scan_run_id.to_string(), Some(error))
            }
            Self::Query { operation, error } => {
                ErrorContext::new("operation", operation, Some(error))
            }
        }
    }
}

impl_err_from_info!(DbError);

impl From<DbError> for MementoError {
    fn from(e: DbError) -> Self {
        Self::Db(e)
    }
}

// ----------------------------- Top-level error -----------------------------

#[derive(Debug)]
pub enum MementoError {
    ConfigError { path: String, error: String },

    InvalidPath { path: PathBuf },

    Cancelled,

    TemplateError(String),

    ScanError(ScanError),

    Db(DbError),

    ThreadPanic { thread_id: String, error: String },
}

impl MementoError {
    pub(crate) fn config_error(path: String, error: String) -> Self {
        Self::ConfigError { path, error }
    }

    pub(crate) fn invalid_path(path: PathBuf) -> Self {
        Self::InvalidPath { path }
    }

    pub(crate) fn cancelled() -> Self {
        Self::Cancelled
    }

    pub(crate) fn scan_error(error: ScanError) -> Self {
        Self::ScanError(error)
    }

    pub(crate) fn db(error: DbError) -> Self {
        Self::Db(error)
    }

    pub fn thread_panic(thread_id: String, e: Box<dyn Any + Send>) -> Self {
        let error = if let Some(s) = e.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload invariant broken".to_string()
        };
        Self::ThreadPanic { thread_id, error }
    }
}

// ----------------------------- Scan error helpers -----------------------------
impl MementoError {
    pub(crate) fn fs(path: PathBuf, error: impl ToString) -> Self {
        Self::scan_error(ScanError::Fs {
            path,
            error: error.to_string(),
        })
    }

    pub(crate) fn walk(path: PathBuf, error: impl ToString) -> Self {
        Self::scan_error(ScanError::Walk {
            path,
            error: error.to_string(),
        })
    }
}

// ----------------------------- DB error helpers -----------------------------

impl MementoError {
    /// Failed to open or initialize the database.
    ///
    /// Error ID: `DB_INIT_FAILED`.
    /// Context: `path`, `error`.
    pub(crate) fn init(path: impl Into<String>, error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::Init {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Failed to acquire the database mutex lock.
    ///
    /// Error ID: `DB_LOCK_FAILED`.
    /// Context: `error`.
    pub(crate) fn lock_failed(error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::LockFailed {
            error: error.to_string(),
        })
    }

    /// Schema migration failed.
    ///
    /// Error ID: `DB_MIGRATION_FAILED`.
    /// Context: `error`.
    pub(crate) fn migration(error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::Migration {
            error: error.to_string(),
        })
    }

    /// Failed to create appender.
    ///
    /// Error ID: `DB_APPENDER_FAILED`.
    /// Context: `table`, `error`.
    pub(crate) fn appender_failed(table: impl Into<String>, error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::Appender {
            table: table.into(),
            error: error.to_string(),
        })
    }

    /// Failed to insert or update a file record.
    ///
    /// Error ID: `DB_UPSERT_FILE_FAILED`.
    /// Context: `path`, `error`.
    pub(crate) fn upsert_file(path: impl Into<String>, error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::UpsertFile {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Failed to check if a file exists in the database.
    ///
    /// Error ID: `DB_FILE_EXISTS_FAILED`.
    /// Context: `path`, `error`.
    pub(crate) fn file_exists(path: impl Into<String>, error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::FileExists {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Failed to query active files for a root directory.
    ///
    /// Error ID: `DB_GET_ACTIVE_FILES_FAILED`.
    /// Context: `root_dir`, `error`.
    pub(crate) fn get_active_files(
        root_dir: impl Into<String>,
        error: impl std::fmt::Display,
    ) -> Self {
        MementoError::db(DbError::GetActiveFiles {
            root_dir: root_dir.into(),
            error: error.to_string(),
        })
    }

    /// Failed to store a cryptographic hash (blake3, content_blake3).
    ///
    /// Error ID: `DB_SET_HASH_FAILED`.
    /// Context: `file_id`, `hash_type`, `error`.
    pub(crate) fn set_hash(
        file_id: i64,
        hash_type: impl Into<String>,
        error: impl std::fmt::Display,
    ) -> Self {
        MementoError::db(DbError::SetHash {
            file_id,
            hash_type: hash_type.into(),
            error: error.to_string(),
        })
    }

    /// Failed to store a perceptual hash (phash, dhash, whash).
    ///
    /// Error ID: `DB_SET_PERCEPTUAL_HASH_FAILED`.
    /// Context: `file_id`, `hash_type`, `error`.
    pub(crate) fn set_perceptual_hash(
        file_id: i64,
        hash_type: impl Into<String>,
        error: impl std::fmt::Display,
    ) -> Self {
        MementoError::db(DbError::SetPerceptualHash {
            file_id,
            hash_type: hash_type.into(),
            error: error.to_string(),
        })
    }

    /// Failed to query files that need a specific hash computed.
    ///
    /// Error ID: `DB_GET_FILES_NEEDING_HASH_FAILED`.
    /// Context: `hash_type`, `error`.
    pub(crate) fn get_files_needing_hash(
        hash_type: impl Into<String>,
        error: impl std::fmt::Display,
    ) -> Self {
        MementoError::db(DbError::GetFilesNeedingHash {
            hash_type: hash_type.into(),
            error: error.to_string(),
        })
    }

    /// Failed to insert metadata tags for a file.
    ///
    /// Error ID: `DB_INSERT_METADATA_FAILED`.
    /// Context: `file_id`, `error`.
    pub(crate) fn insert_metadata(file_id: i64, error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::InsertMetadata {
            file_id,
            error: error.to_string(),
        })
    }

    /// Failed to create a new scan run record.
    ///
    /// Error ID: `DB_CREATE_SCAN_RUN_FAILED`.
    /// Context: `error`.
    pub(crate) fn create_scan_run(error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::CreateScanRun {
            error: error.to_string(),
        })
    }

    /// Failed to update scan run progress counters.
    ///
    /// Error ID: `DB_UPDATE_SCAN_PROGRESS_FAILED`.
    /// Context: `scan_run_id`, `error`.
    pub(crate) fn update_scan_progress(scan_run_id: i64, error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::UpdateScanProgress {
            scan_run_id,
            error: error.to_string(),
        })
    }

    /// Failed to mark a scan run as completed.
    ///
    /// Error ID: `DB_COMPLETE_SCAN_RUN_FAILED`.
    /// Context: `scan_run_id`, `error`.
    pub(crate) fn complete_scan_run(scan_run_id: i64, error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::CompleteScanRun {
            scan_run_id,
            error: error.to_string(),
        })
    }

    /// A generic database query failed.
    ///
    /// Error ID: `DB_QUERY_FAILED`.
    /// Context: `operation`, `error`.
    pub fn query(operation: impl Into<String>, error: impl std::fmt::Display) -> Self {
        MementoError::db(DbError::Query {
            operation: operation.into(),
            error: error.to_string(),
        })
    }
}

impl ErrorInfo for MementoError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::ConfigError { .. } => "CONFIG_ERROR",
            Self::InvalidPath { .. } => "INVALID_PATH",
            Self::Cancelled => "CANCELLED",
            Self::TemplateError(_) => "TEMPLATE_ERROR",
            Self::ScanError(e) => e.error_id(),
            Self::Db(e) => e.error_id(),
            Self::ThreadPanic { .. } => "UNKNOWN",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::ConfigError { path, error } => {
                ErrorContext::new("CONFIG_PATH", path, Some(error))
            }
            Self::InvalidPath { path } => ErrorContext::new(
                "PATH",
                path.to_string_lossy().to_string(),
                Some("Path is not a valid UTF-8 string"),
            ),
            Self::Cancelled => ErrorContext::empty(),
            Self::TemplateError(e) => ErrorContext::error_only(e),
            Self::ScanError(e) => e.context(),
            Self::Db(e) => e.context(),
            Self::ThreadPanic {
                thread_id: id,
                error,
            } => ErrorContext::new("UNKNOWN", id, Some(error)),
        }
    }
}

impl_err_from_info!(MementoError);

impl From<MementoError> for String {
    fn from(e: MementoError) -> Self {
        e.to_string()
    }
}
