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

// ----------------------------- Top-level error -----------------------------

#[derive(Debug)]
pub enum MementoError {
    ConfigError { path: String, error: String },
}

impl ErrorInfo for MementoError {
    fn error_id(&self) -> &'static str {
        match self {
            Self::ConfigError { .. } => "CONFIG_ERROR",
        }
    }

    fn context(&self) -> ErrorContext {
        match self {
            Self::ConfigError { path, error } => {
                ErrorContext::new(Some(error), "CONFIG_PATH", path)
            }
        }
    }
}

impl_err_from_info!(MementoError);

impl From<MementoError> for String {
    fn from(e: MementoError) -> Self {
        e.to_string()
    }
}
