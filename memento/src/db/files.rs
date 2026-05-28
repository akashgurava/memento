use duckdb::Connection;

use crate::error::Result;

use super::Db;

/// A file record as stored in the database.
#[derive(Debug, Clone)]
pub struct FileRecord {
    pub id: i64,
    pub path: String,
    pub size_bytes: i64,
    pub mtime_secs: i64,
    pub mtime_nanos: i32,
}

/// Repository for file CRUD operations.
pub trait FileRepository {
    /// Insert or update a file record. Returns the file ID.
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

    /// Mark a file as missing (no longer on disk).
    fn mark_missing(&self, path: &str) -> Result<()>;

    /// Check if a file exists in the database.
    fn file_exists(&self, path: &str) -> Result<bool>;

    /// Get all active (non-missing) files for a given root directory.
    fn get_active_files_for_root(&self, root_dir: &str) -> Result<Vec<FileRecord>>;

    /// Mark a file's metadata as scanned at current timestamp.
    fn mark_metadata_scanned(&self, file_id: i64) -> Result<()>;
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
        upsert_file_impl(
            &conn,
            path,
            root_dir,
            filename,
            extension,
            size_bytes,
            mtime_secs,
            mtime_nanos,
            file_type,
        )
    }

    fn mark_missing(&self, path: &str) -> Result<()> {
        let conn = self.conn()?;
        mark_missing_impl(&conn, path)
    }

    fn file_exists(&self, path: &str) -> Result<bool> {
        let conn = self.conn()?;
        let exists: bool = conn
            .prepare("SELECT COUNT(*) > 0 FROM files WHERE path = ?")?
            .query_row([path], |row| row.get(0))?;
        Ok(exists)
    }

    fn get_active_files_for_root(&self, root_dir: &str) -> Result<Vec<FileRecord>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, size_bytes, mtime_secs, mtime_nanos
             FROM files WHERE root_dir = ? AND is_missing = false",
        )?;
        let rows = stmt.query_map([root_dir], |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                size_bytes: row.get(2)?,
                mtime_secs: row.get(3)?,
                mtime_nanos: row.get(4)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    fn mark_metadata_scanned(&self, file_id: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE files SET metadata_scanned_at = current_timestamp WHERE id = ?",
            [file_id],
        )?;
        Ok(())
    }
}

// --- Internal helpers (operate on &Connection, avoid re-locking) ---

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsert_file_impl(
    conn: &Connection,
    path: &str,
    root_dir: &str,
    filename: &str,
    extension: Option<&str>,
    size_bytes: i64,
    mtime_secs: i64,
    mtime_nanos: i32,
    file_type: &str,
) -> Result<i64> {
    let existing_id: Option<i64> = conn
        .prepare("SELECT id FROM files WHERE path = ?")?
        .query_row([path], |row| row.get(0))
        .ok();

    if let Some(id) = existing_id {
        conn.execute(
            "UPDATE files SET root_dir = ?, filename = ?, extension = ?, size_bytes = ?,
             mtime_secs = ?, mtime_nanos = ?, file_type = ?, is_missing = false,
             last_verified_at = current_timestamp
             WHERE id = ?",
            duckdb::params![
                root_dir,
                filename,
                extension,
                size_bytes,
                mtime_secs,
                mtime_nanos,
                file_type,
                id
            ],
        )?;
        Ok(id)
    } else {
        let id: i64 = conn
            .prepare("SELECT nextval('files_id_seq')")?
            .query_row([], |row| row.get(0))?;

        conn.execute(
            "INSERT INTO files (id, path, root_dir, filename, extension, size_bytes, mtime_secs, mtime_nanos, file_type)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            duckdb::params![id, path, root_dir, filename, extension, size_bytes, mtime_secs, mtime_nanos, file_type],
        )?;
        Ok(id)
    }
}

pub(crate) fn mark_missing_impl(conn: &Connection, path: &str) -> Result<()> {
    conn.execute(
        "UPDATE files SET is_missing = true, last_verified_at = current_timestamp WHERE path = ?",
        [path],
    )?;
    Ok(())
}
