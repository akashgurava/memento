/// An identifier for an error.
///
/// Provides structured context about which resource an error relates to.
/// For example: `Identifier { kind: "path", value: "/Photos/IMG_001.jpg" }`
#[derive(Debug)]
pub(crate) struct Identifier {
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
pub(crate) struct ErrorContext {
    error: Option<String>,
    identifier: Option<Identifier>,
}

impl ErrorContext {
    /// Creates a context with both an error description and an identifier.
    pub(crate) fn new(
        error: Option<impl ToString>,
        kind: &'static str,
        value: impl ToString,
    ) -> Self {
        Self {
            error: error.map(|e| e.to_string()),
            identifier: Some(Identifier::new(kind, value)),
        }
    }

    /// Creates a context with only an error description, no identifier.
    pub(crate) fn error_only(error: impl ToString) -> Self {
        Self {
            error: Some(error.to_string()),
            identifier: None,
        }
    }

    /// Creates an empty context (no error, no identifier).
    pub(crate) fn empty() -> Self {
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
pub(crate) trait ErrorInfo {
    /// Returns a machine-readable error identifier (e.g. `"SCAN_CANCELLED"`).
    fn error_id(&self) -> &'static str;

    /// Returns the structured context for this error.
    fn context(&self) -> ErrorContext;
}

/// Generates `Display` and `Error` implementations from [`ErrorInfo`].
///
/// The `Display` output format is: `ERROR_ID` or `ERROR_ID. kind: value (error)`
/// for structured log output.
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

/// Database-level failure.
#[derive(Debug)]
pub enum DbError {
    Init {
        path: String,
        error: String,
    },
    Migration {
        error: String,
    },
    Query {
        operation: &'static str,
        error: String,
    },
    Write {
        operation: &'static str,
        error: String,
    },
}

impl DbError {
    /// Failed to open/initialize the database.
    pub fn init(path: impl Into<String>, error: impl std::fmt::Display) -> MementoError {
        MementoError::Db(Self::Init {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Schema migration failed.
    pub fn migration(error: impl std::fmt::Display) -> MementoError {
        MementoError::Db(Self::Migration {
            error: error.to_string(),
        })
    }

    /// A read query failed.
    pub fn query(operation: &'static str, error: impl std::fmt::Display) -> MementoError {
        MementoError::Db(Self::Query {
            operation,
            error: error.to_string(),
        })
    }

    /// A write operation failed.
    pub fn write(operation: &'static str, error: impl std::fmt::Display) -> MementoError {
        MementoError::Db(Self::Write {
            operation,
            error: error.to_string(),
        })
    }
}

impl ErrorInfo for DbError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Init { .. } => "DB_INIT_FAILED",
            Self::Migration { .. } => "DB_MIGRATION_FAILED",
            Self::Query { .. } => "DB_QUERY_FAILED",
            Self::Write { .. } => "DB_WRITE_FAILED",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::Init { path, error } => ErrorContext::new(Some(error), "path", path),
            Self::Migration { error } => ErrorContext::error_only(error),
            Self::Query { operation, error } => {
                ErrorContext::new(Some(error), "operation", operation)
            }
            Self::Write { operation, error } => {
                ErrorContext::new(Some(error), "operation", operation)
            }
        }
    }
}

impl_err_from_info!(DbError);

// ---------------------------------------------------------------------------
// Config errors
// ---------------------------------------------------------------------------

/// Configuration failure.
#[derive(Debug)]
pub enum ConfigError {
    Load { path: String, error: String },
    Save { path: String, error: String },
    Invalid { error: String },
}

impl ConfigError {
    /// Failed to load config from disk.
    pub fn load(path: impl Into<String>, error: impl std::fmt::Display) -> MementoError {
        MementoError::Config(Self::Load {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Failed to save config to disk.
    pub fn save(path: impl Into<String>, error: impl std::fmt::Display) -> MementoError {
        MementoError::Config(Self::Save {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Config content is invalid.
    pub fn invalid(error: impl std::fmt::Display) -> MementoError {
        MementoError::Config(Self::Invalid {
            error: error.to_string(),
        })
    }
}

impl ErrorInfo for ConfigError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Load { .. } => "CONFIG_LOAD_FAILED",
            Self::Save { .. } => "CONFIG_SAVE_FAILED",
            Self::Invalid { .. } => "CONFIG_INVALID",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::Load { path, error } => ErrorContext::new(Some(error), "path", path),
            Self::Save { path, error } => ErrorContext::new(Some(error), "path", path),
            Self::Invalid { error } => ErrorContext::error_only(error),
        }
    }
}

impl_err_from_info!(ConfigError);

// ---------------------------------------------------------------------------
// Scan errors
// ---------------------------------------------------------------------------

/// Scan operation failure.
#[derive(Debug)]
pub enum ScanError {
    Cancelled,
    InvalidLevel { level: u8 },
    InvalidHashType { hash_type: String },
    RootNotFound { path: String },
    LockFailed { error: String },
}

impl ScanError {
    /// Scan was cancelled by user.
    pub fn cancelled() -> MementoError {
        MementoError::Scan(Self::Cancelled)
    }

    /// Invalid scan level requested.
    pub fn invalid_level(level: u8) -> MementoError {
        MementoError::Scan(Self::InvalidLevel { level })
    }

    /// Unknown hash type specified.
    pub fn invalid_hash_type(hash_type: impl Into<String>) -> MementoError {
        MementoError::Scan(Self::InvalidHashType {
            hash_type: hash_type.into(),
        })
    }

    /// Configured scan root directory does not exist.
    pub fn root_not_found(path: impl Into<String>) -> MementoError {
        MementoError::Scan(Self::RootNotFound { path: path.into() })
    }

    /// Failed to acquire mutex lock.
    pub fn lock_failed(error: impl std::fmt::Display) -> MementoError {
        MementoError::Scan(Self::LockFailed {
            error: error.to_string(),
        })
    }
}

impl ErrorInfo for ScanError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Cancelled => "SCAN_CANCELLED",
            Self::InvalidLevel { .. } => "SCAN_INVALID_LEVEL",
            Self::InvalidHashType { .. } => "SCAN_INVALID_HASH_TYPE",
            Self::RootNotFound { .. } => "SCAN_ROOT_NOT_FOUND",
            Self::LockFailed { .. } => "SCAN_LOCK_FAILED",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::Cancelled => ErrorContext::empty(),
            Self::InvalidLevel { level } => {
                ErrorContext::new(None::<&str>, "level", level.to_string())
            }
            Self::InvalidHashType { hash_type } => {
                ErrorContext::new(None::<&str>, "hash_type", hash_type)
            }
            Self::RootNotFound { path } => ErrorContext::new(None::<&str>, "path", path),
            Self::LockFailed { error } => ErrorContext::error_only(error),
        }
    }
}

impl_err_from_info!(ScanError);

// ---------------------------------------------------------------------------
// Hash errors
// ---------------------------------------------------------------------------

/// Hashing operation failure.
#[derive(Debug)]
pub enum HashError {
    FileOpen { path: String, error: String },
    Decode { path: String, error: String },
    UnknownAlgorithm { algorithm: String },
}

impl HashError {
    /// Failed to open file for hashing.
    pub fn file_open(path: impl Into<String>, error: impl std::fmt::Display) -> MementoError {
        MementoError::Hash(Self::FileOpen {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Failed to decode image for content/perceptual hashing.
    pub fn decode(path: impl Into<String>, error: impl std::fmt::Display) -> MementoError {
        MementoError::Hash(Self::Decode {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// Unknown hash algorithm specified.
    pub fn unknown_algorithm(algorithm: impl Into<String>) -> MementoError {
        MementoError::Hash(Self::UnknownAlgorithm {
            algorithm: algorithm.into(),
        })
    }
}

impl ErrorInfo for HashError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::FileOpen { .. } => "HASH_FILE_OPEN_FAILED",
            Self::Decode { .. } => "HASH_DECODE_FAILED",
            Self::UnknownAlgorithm { .. } => "HASH_UNKNOWN_ALGORITHM",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::FileOpen { path, error } => ErrorContext::new(Some(error), "path", path),
            Self::Decode { path, error } => ErrorContext::new(Some(error), "path", path),
            Self::UnknownAlgorithm { algorithm } => {
                ErrorContext::new(None::<&str>, "algorithm", algorithm)
            }
        }
    }
}

impl_err_from_info!(HashError);

// ---------------------------------------------------------------------------
// Metadata errors
// ---------------------------------------------------------------------------

/// Metadata extraction failure.
#[derive(Debug)]
pub enum MetadataError {
    ExifParse { path: String, error: String },
    FfprobeFailed { path: String, error: String },
}

impl MetadataError {
    /// Failed to parse EXIF data from image.
    pub fn exif_parse(path: impl Into<String>, error: impl std::fmt::Display) -> MementoError {
        MementoError::Metadata(Self::ExifParse {
            path: path.into(),
            error: error.to_string(),
        })
    }

    /// ffprobe execution failed for video.
    pub fn ffprobe_failed(path: impl Into<String>, error: impl std::fmt::Display) -> MementoError {
        MementoError::Metadata(Self::FfprobeFailed {
            path: path.into(),
            error: error.to_string(),
        })
    }
}

impl ErrorInfo for MetadataError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::ExifParse { .. } => "METADATA_EXIF_PARSE_FAILED",
            Self::FfprobeFailed { .. } => "METADATA_FFPROBE_FAILED",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::ExifParse { path, error } => ErrorContext::new(Some(error), "path", path),
            Self::FfprobeFailed { path, error } => ErrorContext::new(Some(error), "path", path),
        }
    }
}

impl_err_from_info!(MetadataError);

// ---------------------------------------------------------------------------
// Top-level error
// ---------------------------------------------------------------------------

/// Top-level error type for all memento operations.
///
/// Tauri commands serialize this via `Display` across the IPC boundary.
/// The format is: `ERROR_ID. kind: value (underlying error)`
#[derive(Debug)]
pub enum MementoError {
    Db(DbError),
    Config(ConfigError),
    Scan(ScanError),
    Hash(HashError),
    Metadata(MetadataError),
    Io(std::io::Error),
}

impl ErrorInfo for MementoError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::Db(e) => e.error_id(),
            Self::Config(e) => e.error_id(),
            Self::Scan(e) => e.error_id(),
            Self::Hash(e) => e.error_id(),
            Self::Metadata(e) => e.error_id(),
            Self::Io(_) => "IO_ERROR",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::Db(e) => e.context(),
            Self::Config(e) => e.context(),
            Self::Scan(e) => e.context(),
            Self::Hash(e) => e.context(),
            Self::Metadata(e) => e.context(),
            Self::Io(e) => ErrorContext::error_only(e),
        }
    }
}

impl_err_from_info!(MementoError);

// Tauri commands require errors that implement Into<String>
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

impl From<MetadataError> for MementoError {
    fn from(e: MetadataError) -> Self {
        Self::Metadata(e)
    }
}

impl From<std::io::Error> for MementoError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<duckdb::Error> for MementoError {
    fn from(e: duckdb::Error) -> Self {
        DbError::query("unknown", e)
    }
}

impl From<toml::de::Error> for MementoError {
    fn from(e: toml::de::Error) -> Self {
        ConfigError::invalid(e)
    }
}

impl From<toml::ser::Error> for MementoError {
    fn from(e: toml::ser::Error) -> Self {
        ConfigError::invalid(e)
    }
}

pub type Result<T> = std::result::Result<T, MementoError>;
