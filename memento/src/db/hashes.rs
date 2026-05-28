use duckdb::Connection;

use crate::error::{HashError, Result};

use super::Db;

/// Repository for hash storage and retrieval.
pub trait HashRepository {
    /// Clear all hash columns for a file (after modification detected).
    fn invalidate_hashes(&self, file_id: i64) -> Result<()>;

    /// Update a cryptographic hash column (blake3, content_blake3).
    fn set_hash(&self, file_id: i64, hash_type: &str, value: &str) -> Result<()>;

    /// Update a perceptual hash column (phash, dhash, whash).
    fn set_perceptual_hash(&self, file_id: i64, hash_type: &str, value: i64) -> Result<()>;

    /// Get files missing a specific hash (where that column is NULL).
    fn get_files_needing_hash(
        &self,
        hash_type: &str,
        file_type_filter: Option<&str>,
    ) -> Result<Vec<(i64, String)>>;
}

impl HashRepository for Db {
    fn invalidate_hashes(&self, file_id: i64) -> Result<()> {
        let conn = self.conn()?;
        invalidate_hashes_impl(&conn, file_id)
    }

    fn set_hash(&self, file_id: i64, hash_type: &str, value: &str) -> Result<()> {
        let conn = self.conn()?;
        set_hash_impl(&conn, file_id, hash_type, value)
    }

    fn set_perceptual_hash(&self, file_id: i64, hash_type: &str, value: i64) -> Result<()> {
        let conn = self.conn()?;
        set_perceptual_hash_impl(&conn, file_id, hash_type, value)
    }

    fn get_files_needing_hash(
        &self,
        hash_type: &str,
        file_type_filter: Option<&str>,
    ) -> Result<Vec<(i64, String)>> {
        let conn = self.conn()?;
        get_files_needing_hash_impl(&conn, hash_type, file_type_filter)
    }
}

// --- Internal helpers ---

pub(crate) fn invalidate_hashes_impl(conn: &Connection, file_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE files SET
         hash_blake3 = NULL, hash_content_blake3 = NULL,
         hash_phash = NULL, hash_dhash = NULL, hash_whash = NULL,
         metadata_scanned_at = NULL
         WHERE id = ?",
        [file_id],
    )?;
    Ok(())
}

pub(crate) fn set_hash_impl(
    conn: &Connection,
    file_id: i64,
    hash_type: &str,
    value: &str,
) -> Result<()> {
    let column = match hash_type {
        "blake3" => "hash_blake3",
        "content_blake3" => "hash_content_blake3",
        _ => return Err(HashError::unknown_algorithm(hash_type)),
    };
    let sql = format!("UPDATE files SET {} = ? WHERE id = ?", column);
    conn.execute(&sql, duckdb::params![value, file_id])?;
    Ok(())
}

pub(crate) fn set_perceptual_hash_impl(
    conn: &Connection,
    file_id: i64,
    hash_type: &str,
    value: i64,
) -> Result<()> {
    let column = match hash_type {
        "phash" => "hash_phash",
        "dhash" => "hash_dhash",
        "whash" => "hash_whash",
        _ => return Err(HashError::unknown_algorithm(hash_type)),
    };
    let sql = format!("UPDATE files SET {} = ? WHERE id = ?", column);
    conn.execute(&sql, duckdb::params![value, file_id])?;
    Ok(())
}

pub(crate) fn get_files_needing_hash_impl(
    conn: &Connection,
    hash_type: &str,
    file_type_filter: Option<&str>,
) -> Result<Vec<(i64, String)>> {
    let column = match hash_type {
        "blake3" => "hash_blake3",
        "content_blake3" => "hash_content_blake3",
        "phash" => "hash_phash",
        "dhash" => "hash_dhash",
        "whash" => "hash_whash",
        _ => return Err(HashError::unknown_algorithm(hash_type)),
    };

    let sql = if let Some(ft) = file_type_filter {
        format!(
            "SELECT id, path FROM files WHERE {} IS NULL AND is_missing = false AND file_type = '{}' ORDER BY size_bytes ASC",
            column, ft
        )
    } else {
        format!(
            "SELECT id, path FROM files WHERE {} IS NULL AND is_missing = false ORDER BY size_bytes ASC",
            column
        )
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}
