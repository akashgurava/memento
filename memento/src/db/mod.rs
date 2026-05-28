pub(crate) mod files;
pub(crate) mod hashes;
pub(crate) mod metadata_repo;
pub mod migrations;
pub(crate) mod scans;

pub use files::{FileRecord, FileRepository};
pub use hashes::HashRepository;
pub use metadata_repo::MetadataRepository;
pub use scans::ScanRepository;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use duckdb::Connection;

use crate::error::{DbError, Result};

/// Thread-safe database handle wrapping a DuckDB connection.
///
/// All repository traits are implemented on this struct, with each method
/// acquiring the internal lock. For batch operations or raw queries,
/// use [`Db::conn()`] to access the underlying connection directly.
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Open (or create) the database at `path` and run migrations.
    pub fn open(path: &Path) -> Result<Self> {
        let conn =
            Connection::open(path).map_err(|e| DbError::init(path.display().to_string(), e))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Acquire the underlying connection lock.
    ///
    /// Use this for ad-hoc queries or batch operations where acquiring
    /// the lock per-call would be undesirable.
    pub fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(DbError::lock_failed)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn()?;
        migrations::run_migrations(&conn)
    }
}

#[cfg(test)]
impl Db {
    /// Create an in-memory database for tests.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| DbError::init(":memory:", e))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }
}
