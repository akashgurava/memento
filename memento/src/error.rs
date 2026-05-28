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
    error: Option<String>,
    identifier: Option<Identifier>,
}

impl ErrorContext {
    /// Creates a context with both an error description and an identifier.
    fn new(error: Option<impl ToString>, kind: &'static str, value: impl ToString) -> Self {
        Self {
            error: error.map(|e| e.to_string()),
            identifier: Some(Identifier::new(kind, value)),
        }
    }

    /// Creates a context with only an error description, no identifier.
    fn error_only(error: impl ToString) -> Self {
        Self {
            error: Some(error.to_string()),
            identifier: None,
        }
    }

    /// Creates an empty context (no error, no identifier).
    fn empty() -> Self {
        Self {
            error: None,
            identifier: None,
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

// ---------------------------------------------------------------------------
// Database errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum DbError {
    Init {
        path: String,
        error: String,
    },
    Migration {
        error: String,
    },
    LockFailed {
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

impl DbError {
    /// Failed to open or initialize the database.
    ///
    /// Error ID: `DB_INIT_FAILED`.
    /// Context: `path`, `error`.
    pub fn init(path: impl Into<String>, error: impl std::fmt::Display) -> MementoError {
        MementoError::Db(Self::Init {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Schema migration failed.
    ///
    /// Error ID: `DB_MIGRATION_FAILED`.
    /// Context: `error`.
    pub(crate) fn migration(error: impl std::fmt::Display) -> MementoError {
        MementoError::Db(Self::Migration {
            error: error.to_string(),
        })
    }

    /// Failed to acquire the database mutex lock.
    ///
    /// Error ID: `DB_LOCK_FAILED`.
    /// Context: `error`.
    pub(crate) fn lock_failed(error: impl std::fmt::Display) -> MementoError {
        MementoError::Db(Self::LockFailed {
            error: error.to_string(),
        })
    }

    /// Failed to insert or update a file record.
    ///
    /// Error ID: `DB_UPSERT_FILE_FAILED`.
    /// Context: `path`, `error`.
    pub(crate) fn upsert_file(
        path: impl Into<String>,
        error: impl std::fmt::Display,
    ) -> MementoError {
        MementoError::Db(Self::UpsertFile {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Failed to check if a file exists in the database.
    ///
    /// Error ID: `DB_FILE_EXISTS_FAILED`.
    /// Context: `path`, `error`.
    pub(crate) fn file_exists(
        path: impl Into<String>,
        error: impl std::fmt::Display,
    ) -> MementoError {
        MementoError::Db(Self::FileExists {
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
    ) -> MementoError {
        MementoError::Db(Self::GetActiveFiles {
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
    ) -> MementoError {
        MementoError::Db(Self::SetHash {
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
    ) -> MementoError {
        MementoError::Db(Self::SetPerceptualHash {
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
    ) -> MementoError {
        MementoError::Db(Self::GetFilesNeedingHash {
            hash_type: hash_type.into(),
            error: error.to_string(),
        })
    }

    /// Failed to insert metadata tags for a file.
    ///
    /// Error ID: `DB_INSERT_METADATA_FAILED`.
    /// Context: `file_id`, `error`.
    pub(crate) fn insert_metadata(file_id: i64, error: impl std::fmt::Display) -> MementoError {
        MementoError::Db(Self::InsertMetadata {
            file_id,
            error: error.to_string(),
        })
    }

    /// Failed to create a new scan run record.
    ///
    /// Error ID: `DB_CREATE_SCAN_RUN_FAILED`.
    /// Context: `error`.
    pub(crate) fn create_scan_run(error: impl std::fmt::Display) -> MementoError {
        MementoError::Db(Self::CreateScanRun {
            error: error.to_string(),
        })
    }

    /// Failed to update scan run progress counters.
    ///
    /// Error ID: `DB_UPDATE_SCAN_PROGRESS_FAILED`.
    /// Context: `scan_run_id`, `error`.
    pub(crate) fn update_scan_progress(
        scan_run_id: i64,
        error: impl std::fmt::Display,
    ) -> MementoError {
        MementoError::Db(Self::UpdateScanProgress {
            scan_run_id,
            error: error.to_string(),
        })
    }

    /// Failed to mark a scan run as completed.
    ///
    /// Error ID: `DB_COMPLETE_SCAN_RUN_FAILED`.
    /// Context: `scan_run_id`, `error`.
    pub(crate) fn complete_scan_run(
        scan_run_id: i64,
        error: impl std::fmt::Display,
    ) -> MementoError {
        MementoError::Db(Self::CompleteScanRun {
            scan_run_id,
            error: error.to_string(),
        })
    }

    /// A generic database query failed.
    ///
    /// Error ID: `DB_QUERY_FAILED`.
    /// Context: `operation`, `error`.
    pub fn query(operation: impl Into<String>, error: impl std::fmt::Display) -> MementoError {
        MementoError::Db(Self::Query {
            operation: operation.into(),
            error: error.to_string(),
        })
    }
}

impl ErrorInfo for DbError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Init { .. } => "DB_INIT_FAILED",
            Self::Migration { .. } => "DB_MIGRATION_FAILED",
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
            Self::Init { path, error } => ErrorContext::new(Some(error), "path", path),
            Self::Migration { error } => ErrorContext::error_only(error),
            Self::LockFailed { error } => ErrorContext::error_only(error),
            Self::UpsertFile { path, error } => ErrorContext::new(Some(error), "path", path),
            Self::FileExists { path, error } => ErrorContext::new(Some(error), "path", path),
            Self::GetActiveFiles { root_dir, error } => {
                ErrorContext::new(Some(error), "root_dir", root_dir)
            }
            Self::SetHash {
                file_id,
                hash_type,
                error,
            } => ErrorContext::new(Some(error), "file_id", format!("{file_id}/{hash_type}")),
            Self::SetPerceptualHash {
                file_id,
                hash_type,
                error,
            } => ErrorContext::new(Some(error), "file_id", format!("{file_id}/{hash_type}")),
            Self::GetFilesNeedingHash { hash_type, error } => {
                ErrorContext::new(Some(error), "hash_type", hash_type)
            }
            Self::InsertMetadata { file_id, error } => {
                ErrorContext::new(Some(error), "file_id", file_id.to_string())
            }
            Self::CreateScanRun { error } => ErrorContext::error_only(error),
            Self::UpdateScanProgress { scan_run_id, error } => {
                ErrorContext::new(Some(error), "scan_run_id", scan_run_id.to_string())
            }
            Self::CompleteScanRun { scan_run_id, error } => {
                ErrorContext::new(Some(error), "scan_run_id", scan_run_id.to_string())
            }
            Self::Query { operation, error } => {
                ErrorContext::new(Some(error), "operation", operation)
            }
        }
    }
}

impl_err_from_info!(DbError);

// ---------------------------------------------------------------------------
// Config errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ConfigError {
    Invalid { error: String },
}

impl ConfigError {
    /// Config content is malformed or contains invalid values.
    ///
    /// Error ID: `CONFIG_INVALID`.
    /// Context: `error`.
    pub(crate) fn invalid(error: impl std::fmt::Display) -> MementoError {
        MementoError::Config(Self::Invalid {
            error: error.to_string(),
        })
    }
}

impl ErrorInfo for ConfigError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Invalid { .. } => "CONFIG_INVALID",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::Invalid { error } => ErrorContext::error_only(error),
        }
    }
}

impl_err_from_info!(ConfigError);

// ---------------------------------------------------------------------------
// Scan errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ScanError {
    Cancelled,
    InvalidLevel { level: u8 },
    ThreadPoolBuild { error: String },
}

impl ScanError {
    /// Scan was cancelled by the user.
    ///
    /// Error ID: `SCAN_CANCELLED`.
    pub(crate) fn cancelled() -> MementoError {
        MementoError::Scan(Self::Cancelled)
    }

    /// Invalid scan level requested.
    ///
    /// Error ID: `SCAN_INVALID_LEVEL`.
    /// Context: `level`.
    pub fn invalid_level(level: u8) -> MementoError {
        MementoError::Scan(Self::InvalidLevel { level })
    }

    /// Failed to build the rayon thread pool for parallel processing.
    ///
    /// Error ID: `SCAN_THREAD_POOL_BUILD_FAILED`.
    /// Context: `error`.
    pub(crate) fn thread_pool_build(error: impl std::fmt::Display) -> MementoError {
        MementoError::Scan(Self::ThreadPoolBuild {
            error: error.to_string(),
        })
    }
}

impl ErrorInfo for ScanError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Cancelled => "SCAN_CANCELLED",
            Self::InvalidLevel { .. } => "SCAN_INVALID_LEVEL",
            Self::ThreadPoolBuild { .. } => "SCAN_THREAD_POOL_BUILD_FAILED",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::Cancelled => ErrorContext::empty(),
            Self::InvalidLevel { level } => {
                ErrorContext::new(None::<&str>, "level", level.to_string())
            }
            Self::ThreadPoolBuild { error } => ErrorContext::error_only(error),
        }
    }
}

impl_err_from_info!(ScanError);

// ---------------------------------------------------------------------------
// Hash errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum HashError {
    Decode { path: String, error: String },
    UnknownAlgorithm { algorithm: String },
}

impl HashError {
    /// Failed to decode image for content or perceptual hashing.
    ///
    /// Error ID: `HASH_DECODE_FAILED`.
    /// Context: `path`, `error`.
    pub(crate) fn decode(path: impl Into<String>, error: impl std::fmt::Display) -> MementoError {
        MementoError::Hash(Self::Decode {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Unknown hash algorithm specified.
    ///
    /// Error ID: `HASH_UNKNOWN_ALGORITHM`.
    /// Context: `algorithm`.
    pub fn unknown_algorithm(algorithm: impl Into<String>) -> MementoError {
        MementoError::Hash(Self::UnknownAlgorithm {
            algorithm: algorithm.into(),
        })
    }
}

impl ErrorInfo for HashError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Decode { .. } => "HASH_DECODE_FAILED",
            Self::UnknownAlgorithm { .. } => "HASH_UNKNOWN_ALGORITHM",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::Decode { path, error } => ErrorContext::new(Some(error), "path", path),
            Self::UnknownAlgorithm { algorithm } => {
                ErrorContext::new(None::<&str>, "algorithm", algorithm)
            }
        }
    }
}

impl_err_from_info!(HashError);

// ---------------------------------------------------------------------------
// Top-level error
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum MementoError {
    Db(DbError),
    Config(ConfigError),
    Scan(ScanError),
    Hash(HashError),
    Io(std::io::Error),
}

impl ErrorInfo for MementoError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Db(e) => e.error_id(),
            Self::Config(e) => e.error_id(),
            Self::Scan(e) => e.error_id(),
            Self::Hash(e) => e.error_id(),
            Self::Io(_) => "IO_ERROR",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::Db(e) => e.context(),
            Self::Config(e) => e.context(),
            Self::Scan(e) => e.context(),
            Self::Hash(e) => e.context(),
            Self::Io(e) => ErrorContext::error_only(e),
        }
    }
}

impl_err_from_info!(MementoError);

impl From<MementoError> for String {
    fn from(e: MementoError) -> Self {
        e.to_string()
    }
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<DbError> for MementoError {
    fn from(e: DbError) -> Self {
        Self::Db(e)
    }
}

impl From<ConfigError> for MementoError {
    fn from(e: ConfigError) -> Self {
        Self::Config(e)
    }
}

impl From<ScanError> for MementoError {
    fn from(e: ScanError) -> Self {
        Self::Scan(e)
    }
}

impl From<HashError> for MementoError {
    fn from(e: HashError) -> Self {
        Self::Hash(e)
    }
}

impl From<std::io::Error> for MementoError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_yml::Error> for MementoError {
    fn from(e: serde_yml::Error) -> Self {
        ConfigError::invalid(e)
    }
}

pub type Result<T> = std::result::Result<T, MementoError>;
