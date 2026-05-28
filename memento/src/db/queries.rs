use duckdb::Connection;

use crate::error::Result;
use crate::metadata::MetadataEntry;

/// Insert or update a file record. Returns the file ID.
#[allow(clippy::too_many_arguments)]
pub fn upsert_file(
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
    // Check if file already exists
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

/// Mark a file as missing (no longer on disk)
pub fn mark_missing(conn: &Connection, path: &str) -> Result<()> {
    conn.execute(
        "UPDATE files SET is_missing = true, last_verified_at = current_timestamp WHERE path = ?",
        [path],
    )?;
    Ok(())
}

/// Clear all hash columns for a file (after modification detected)
pub fn invalidate_hashes(conn: &Connection, file_id: i64) -> Result<()> {
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

/// Update a specific hash column for a file
pub fn set_hash(conn: &Connection, file_id: i64, hash_type: &str, value: &str) -> Result<()> {
    let column = match hash_type {
        "blake3" => "hash_blake3",
        "content_blake3" => "hash_content_blake3",
        _ => return Err(crate::error::HashError::unknown_algorithm(hash_type)),
    };
    let sql = format!("UPDATE files SET {} = ? WHERE id = ?", column);
    conn.execute(&sql, duckdb::params![value, file_id])?;
    Ok(())
}

/// Update a perceptual hash column (stored as i64)
pub fn set_perceptual_hash(
    conn: &Connection,
    file_id: i64,
    hash_type: &str,
    value: i64,
) -> Result<()> {
    let column = match hash_type {
        "phash" => "hash_phash",
        "dhash" => "hash_dhash",
        "whash" => "hash_whash",
        _ => return Err(crate::error::HashError::unknown_algorithm(hash_type)),
    };
    let sql = format!("UPDATE files SET {} = ? WHERE id = ?", column);
    conn.execute(&sql, duckdb::params![value, file_id])?;
    Ok(())
}

/// Insert metadata tags for a file (batch)
pub fn insert_metadata_batch(
    conn: &Connection,
    file_id: i64,
    entries: &[MetadataEntry],
) -> Result<()> {
    // Delete existing metadata for this file first
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

/// Create a new scan run record
pub fn create_scan_run(
    conn: &Connection,
    scan_level: i32,
    hash_type: Option<&str>,
    root_dir: Option<&str>,
) -> Result<i64> {
    let id: i64 = conn
        .prepare("SELECT nextval('scan_runs_id_seq')")?
        .query_row([], |row| row.get(0))?;

    conn.execute(
        "INSERT INTO scan_runs (id, scan_level, hash_type, root_dir)
         VALUES (?, ?, ?, ?)",
        duckdb::params![id, scan_level, hash_type, root_dir],
    )?;

    Ok(id)
}

/// Update scan run progress
pub fn update_scan_progress(
    conn: &Connection,
    scan_run_id: i64,
    files_processed: i64,
    files_total: Option<i64>,
) -> Result<()> {
    conn.execute(
        "UPDATE scan_runs SET files_processed = ?, files_total = ? WHERE id = ?",
        duckdb::params![files_processed, files_total, scan_run_id],
    )?;
    Ok(())
}

/// Mark scan run as completed
pub fn complete_scan_run(
    conn: &Connection,
    scan_run_id: i64,
    status: &str,
    error_message: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE scan_runs SET status = ?, completed_at = current_timestamp, error_message = ? WHERE id = ?",
        duckdb::params![status, error_message, scan_run_id],
    )?;
    Ok(())
}

/// Get files needing a specific hash (where that hash column is NULL)
pub fn get_files_needing_hash(
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
        _ => return Err(crate::error::HashError::unknown_algorithm(hash_type)),
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
