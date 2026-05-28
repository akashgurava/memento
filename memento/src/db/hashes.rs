use duckdb::Connection;

use crate::error::{DbError, HashError, Result};

use super::Db;

/// Hash storage and querying for all supported algorithms.
pub trait HashRepository {
    fn set_hash(&self, file_id: i64, hash_type: &str, value: &str) -> Result<()>;
    fn set_perceptual_hash(&self, file_id: i64, hash_type: &str, value: i64) -> Result<()>;
    fn get_files_needing_hash(
        &self,
        hash_type: &str,
        file_type_filter: Option<&str>,
    ) -> Result<Vec<(i64, String)>>;
}

impl HashRepository for Db {
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

/// Get the latest stat_id for a file_id.
fn get_latest_stat_id(conn: &Connection, file_id: i64) -> Result<i64> {
    conn.prepare(
        "SELECT stat_id FROM file_stats WHERE file_id = ?
         QUALIFY ROW_NUMBER() OVER (PARTITION BY file_id ORDER BY observed_at DESC) = 1",
    )
    .and_then(|mut s| s.query_row([file_id], |row| row.get(0)))
    .map_err(|e| DbError::set_hash(file_id, "stat_id_lookup", e))
}

/// Validate hash_name and return it (for insert).
fn validate_hash_name(hash_type: &str) -> std::result::Result<&str, crate::error::MementoError> {
    match hash_type {
        "blake3" | "content_blake3" | "phash" | "dhash" | "whash" => Ok(hash_type),
        _ => Err(HashError::unknown_algorithm(hash_type)),
    }
}

/// Store a cryptographic hash (blake3 or content_blake3) for a file.
pub(crate) fn set_hash_impl(
    conn: &Connection,
    file_id: i64,
    hash_type: &str,
    value: &str,
) -> Result<()> {
    validate_hash_name(hash_type)?;
    let stat_id = get_latest_stat_id(conn, file_id)?;

    conn.execute(
        "INSERT INTO file_hashes (file_id, stat_id, hash_name, hash_value) VALUES (?, ?, ?, ?)",
        duckdb::params![file_id, stat_id, hash_type, value],
    )
    .map_err(|e| DbError::set_hash(file_id, hash_type, e))?;

    Ok(())
}

/// Store a perceptual hash (phash, dhash, whash) as text representation of 64-bit integer.
pub(crate) fn set_perceptual_hash_impl(
    conn: &Connection,
    file_id: i64,
    hash_type: &str,
    value: i64,
) -> Result<()> {
    validate_hash_name(hash_type)?;
    let stat_id = get_latest_stat_id(conn, file_id)?;

    conn.execute(
        "INSERT INTO file_hashes (file_id, stat_id, hash_name, hash_value) VALUES (?, ?, ?, ?)",
        duckdb::params![file_id, stat_id, hash_type, value.to_string()],
    )
    .map_err(|e| DbError::set_perceptual_hash(file_id, hash_type, e))?;

    Ok(())
}

/// Query files that don't yet have a specific hash computed.
/// Returns `(file_id, path)` pairs ordered by size ascending (small files first).
pub(crate) fn get_files_needing_hash_impl(
    conn: &Connection,
    hash_type: &str,
    file_type_filter: Option<&str>,
) -> Result<Vec<(i64, String)>> {
    validate_hash_name(hash_type)?;

    let sql = if let Some(ft) = file_type_filter {
        format!(
            "SELECT v.id, v.path FROM v_files v
             WHERE v.is_missing = false AND v.file_type = '{ft}'
               AND NOT EXISTS (
                   SELECT 1 FROM file_hashes h
                   WHERE h.file_id = v.id AND h.hash_name = '{ht}'
               )
             ORDER BY v.size_bytes ASC",
            ft = ft,
            ht = hash_type
        )
    } else {
        format!(
            "SELECT v.id, v.path FROM v_files v
             WHERE v.is_missing = false
               AND NOT EXISTS (
                   SELECT 1 FROM file_hashes h
                   WHERE h.file_id = v.id AND h.hash_name = '{ht}'
               )
             ORDER BY v.size_bytes ASC",
            ht = hash_type
        )
    };

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| DbError::get_files_needing_hash(hash_type, e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| DbError::get_files_needing_hash(hash_type, e))?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| DbError::get_files_needing_hash(hash_type, e))?);
    }
    Ok(results)
}
