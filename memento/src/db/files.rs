use duckdb::Connection;

use crate::error::{DbError, Result};

use super::Db;

/// A row from the latest file state view, used for queries.
#[derive(Debug, Clone)]
pub struct FileRecord {
    pub id: i64,
    pub path: String,
    pub size_bytes: i64,
    pub mtime_secs: i64,
    pub mtime_nanos: i32,
}

/// File lifecycle operations: insert observations, query active files.
pub trait FileRepository {
    /// Insert a file observation (get-or-create file_master + insert file_stats row).
    /// Returns the file_id.
    #[allow(clippy::too_many_arguments)]
    fn upsert_file(
        &self,
        path: &str,
        root_dir: &str,
        filename: &str,
        extension: Option<&str>,
        size_bytes: i64,
        mtime_secs: i64,
        mtime_nanos: i32,
        file_type: &str,
    ) -> Result<i64>;

    /// Record that a file is now missing (insert a file_stats row with is_missing=true).
    fn mark_missing(&self, path: &str) -> Result<()>;

    fn file_exists(&self, path: &str) -> Result<bool>;
    fn get_active_files_for_root(&self, root_dir: &str) -> Result<Vec<FileRecord>>;
}

impl FileRepository for Db {
    fn upsert_file(
        &self,
        path: &str,
        root_dir: &str,
        filename: &str,
        extension: Option<&str>,
        size_bytes: i64,
        mtime_secs: i64,
        mtime_nanos: i32,
        file_type: &str,
    ) -> Result<i64> {
        let conn = self.conn()?;
        let file_id = get_or_create_file_id(&conn, path)?;
        insert_file_stat(
            &conn, file_id, root_dir, filename, extension, size_bytes, mtime_secs, mtime_nanos,
            file_type, false,
        )?;
        Ok(file_id)
    }

    fn mark_missing(&self, path: &str) -> Result<()> {
        let conn = self.conn()?;
        insert_missing_stat(&conn, path)
    }

    fn file_exists(&self, path: &str) -> Result<bool> {
        let conn = self.conn()?;
        conn.prepare("SELECT COUNT(*) > 0 FROM file_master WHERE path = ?")
            .and_then(|mut s| s.query_row([path], |row| row.get(0)))
            .map_err(|e| DbError::file_exists(path, e))
    }

    fn get_active_files_for_root(&self, root_dir: &str) -> Result<Vec<FileRecord>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, path, size_bytes, mtime_secs, mtime_nanos
                 FROM v_files
                 WHERE root_dir = ? AND is_missing = false",
            )
            .map_err(|e| DbError::get_active_files(root_dir, e))?;

        let rows = stmt
            .query_map([root_dir], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    size_bytes: row.get(2)?,
                    mtime_secs: row.get(3)?,
                    mtime_nanos: row.get(4)?,
                })
            })
            .map_err(|e| DbError::get_active_files(root_dir, e))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| DbError::get_active_files(root_dir, e))?);
        }
        Ok(results)
    }
}

// --- Internal helpers (operate on &Connection, avoid re-locking in batch loops) ---

/// Get or create a file_master entry for a path. Returns the file_id.
pub(crate) fn get_or_create_file_id(conn: &Connection, path: &str) -> Result<i64> {
    let existing_id: Option<i64> = conn
        .prepare("SELECT id FROM file_master WHERE path = ?")
        .and_then(|mut s| s.query_row([path], |row| row.get(0)))
        .ok();

    if let Some(id) = existing_id {
        return Ok(id);
    }

    let id: i64 = conn
        .prepare("SELECT nextval('file_master_id_seq')")
        .and_then(|mut s| s.query_row([], |row| row.get(0)))
        .map_err(|e| DbError::upsert_file(path, e))?;

    conn.execute(
        "INSERT INTO file_master (id, path) VALUES (?, ?)",
        duckdb::params![id, path],
    )
    .map_err(|e| DbError::upsert_file(path, e))?;

    Ok(id)
}

/// Insert a new file_stats observation. Returns the stat_id.
#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_file_stat(
    conn: &Connection,
    file_id: i64,
    root_dir: &str,
    filename: &str,
    extension: Option<&str>,
    size_bytes: i64,
    mtime_secs: i64,
    mtime_nanos: i32,
    file_type: &str,
    is_missing: bool,
) -> Result<i64> {
    let stat_id: i64 = conn
        .prepare("SELECT nextval('file_stats_id_seq')")
        .and_then(|mut s| s.query_row([], |row| row.get(0)))
        .map_err(|e| DbError::upsert_file(root_dir, e))?;

    conn.execute(
        "INSERT INTO file_stats (stat_id, file_id, root_dir, filename, extension, size_bytes, mtime_secs, mtime_nanos, file_type, is_missing)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        duckdb::params![stat_id, file_id, root_dir, filename, extension, size_bytes, mtime_secs, mtime_nanos, file_type, is_missing],
    )
    .map_err(|e| DbError::upsert_file(root_dir, e))?;

    Ok(stat_id)
}

/// Insert a missing observation for a file by path.
#[allow(clippy::type_complexity)]
pub(crate) fn insert_missing_stat(conn: &Connection, path: &str) -> Result<()> {
    // Get the file_id and latest state to carry forward fields
    let row: Option<(i64, String, String, Option<String>, i64, i64, i32, String)> = conn
        .prepare(
            "SELECT m.id, fs.root_dir, fs.filename, fs.extension,
                    fs.size_bytes, fs.mtime_secs, fs.mtime_nanos, fs.file_type
             FROM file_master m
             JOIN file_stats fs ON fs.file_id = m.id
             WHERE m.path = ?
             QUALIFY ROW_NUMBER() OVER (PARTITION BY m.id ORDER BY fs.observed_at DESC) = 1",
        )
        .and_then(|mut s| {
            s.query_row([path], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            })
        })
        .ok();

    if let Some((file_id, root_dir, filename, extension, size_bytes, mtime_secs, mtime_nanos, file_type)) = row {
        insert_file_stat(
            conn,
            file_id,
            &root_dir,
            &filename,
            extension.as_deref(),
            size_bytes,
            mtime_secs,
            mtime_nanos,
            &file_type,
            true,
        )?;
    }

    Ok(())
}
