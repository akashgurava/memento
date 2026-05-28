use duckdb::Connection;

use crate::error::Result;
use crate::metadata::MetadataEntry;

use super::Db;

/// Repository for file metadata (EXIF, XMP, etc.) storage.
pub trait MetadataRepository {
    /// Insert metadata tags for a file (replaces existing).
    fn insert_metadata_batch(&self, file_id: i64, entries: &[MetadataEntry]) -> Result<()>;
}

impl MetadataRepository for Db {
    fn insert_metadata_batch(&self, file_id: i64, entries: &[MetadataEntry]) -> Result<()> {
        let conn = self.conn()?;
        insert_metadata_batch_impl(&conn, file_id, entries)
    }
}

// --- Internal helper ---

pub(crate) fn insert_metadata_batch_impl(
    conn: &Connection,
    file_id: i64,
    entries: &[MetadataEntry],
) -> Result<()> {
    conn.execute("DELETE FROM file_metadata WHERE file_id = ?", [file_id])?;

    let mut stmt = conn.prepare(
        "INSERT INTO file_metadata (file_id, namespace, tag, value_text, value_int, value_real)
         VALUES (?, ?, ?, ?, ?, ?)",
    )?;

    for (namespace, tag, value_text, value_int, value_real) in entries {
        stmt.execute(duckdb::params![
            file_id,
            namespace,
            tag,
            value_text.as_deref(),
            value_int,
            value_real,
        ])?;
    }

    Ok(())
}
