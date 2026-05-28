use crate::error::{DbError, Result};

use super::Db;

/// Repository for scan run lifecycle management.
pub trait ScanRepository {
    /// Create a new scan run record. Returns the run ID.
    fn create_scan_run(
        &self,
        scan_level: i32,
        hash_type: Option<&str>,
        root_dir: Option<&str>,
    ) -> Result<i64>;

    /// Update scan run progress counters.
    fn update_scan_progress(
        &self,
        scan_run_id: i64,
        files_processed: i64,
        files_total: Option<i64>,
    ) -> Result<()>;

    /// Mark scan run as completed (or failed).
    fn complete_scan_run(
        &self,
        scan_run_id: i64,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<()>;
}

impl ScanRepository for Db {
    fn create_scan_run(
        &self,
        scan_level: i32,
        hash_type: Option<&str>,
        root_dir: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn()?;

        let id: i64 = conn
            .prepare("SELECT nextval('scan_runs_id_seq')")
            .and_then(|mut s| s.query_row([], |row| row.get(0)))
            .map_err(DbError::create_scan_run)?;

        conn.execute(
            "INSERT INTO scan_runs (id, scan_level, hash_type, root_dir)
             VALUES (?, ?, ?, ?)",
            duckdb::params![id, scan_level, hash_type, root_dir],
        )
        .map_err(DbError::create_scan_run)?;

        Ok(id)
    }

    fn update_scan_progress(
        &self,
        scan_run_id: i64,
        files_processed: i64,
        files_total: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE scan_runs SET files_processed = ?, files_total = ? WHERE id = ?",
            duckdb::params![files_processed, files_total, scan_run_id],
        )
        .map_err(|e| DbError::update_scan_progress(scan_run_id, e))?;
        Ok(())
    }

    fn complete_scan_run(
        &self,
        scan_run_id: i64,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE scan_runs SET status = ?, completed_at = current_timestamp, error_message = ? WHERE id = ?",
            duckdb::params![status, error_message, scan_run_id],
        )
        .map_err(|e| DbError::complete_scan_run(scan_run_id, e))?;
        Ok(())
    }
}
