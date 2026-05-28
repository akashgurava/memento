use duckdb::Connection;

use crate::error::{DbError, Result};
use crate::metadata::MetadataEntry;

use super::Db;

/// EAV metadata tag storage (EXIF, XMP, IPTC, video tags).
pub trait MetadataRepository {
    /// Append metadata entries for a file.
    fn insert_metadata_batch(&self, file_id: i64, entries: &[MetadataEntry]) -> Result<()>;
}

impl MetadataRepository for Db {
    fn insert_metadata_batch(&self, file_id: i64, entries: &[MetadataEntry]) -> Result<()> {
        let conn = self.conn()?;
        insert_metadata_batch_impl(&conn, file_id, entries)
    }
}

/// Append metadata entries for a file (append-only, no DELETE).
/// Looks up the latest stat_id for the file and associates entries with it.
pub(crate) fn insert_metadata_batch_impl(
    conn: &Connection,
    file_id: i64,
    entries: &[MetadataEntry],
) -> Result<()> {
    let stat_id: i64 = conn
        .prepare(
            "SELECT stat_id FROM file_stats WHERE file_id = ?
             QUALIFY ROW_NUMBER() OVER (PARTITION BY file_id ORDER BY observed_at DESC) = 1",
        )
        .and_then(|mut s| s.query_row([file_id], |row| row.get(0)))
        .map_err(|e| DbError::insert_metadata(file_id, e))?;

    let mut stmt = conn
        .prepare(
            "INSERT INTO file_metadata (file_id, stat_id, namespace, tag, value)
             VALUES (?, ?, ?, ?, ?)",
        )
        .map_err(|e| DbError::insert_metadata(file_id, e))?;

    for (namespace, tag, value) in entries {
        stmt.execute(duckdb::params![file_id, stat_id, namespace, tag, value.as_deref()])
            .map_err(|e| DbError::insert_metadata(file_id, e))?;
    }

    Ok(())
}
