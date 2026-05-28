use crate::error::{DbError, Result};
use crate::hashing::HashResult;
use crate::metadata::MetadataEntry;
use crate::scanner::store::{HashScanStore, MetadataScanStore, StatEntry, StatsScanStore};

use super::files::{get_or_create_file_id, insert_file_stat, insert_missing_stat};
use super::Db;

impl StatsScanStore for Db {
    fn get_known_paths_for_root(&self, root: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT path FROM v_files
                 WHERE root_dir = ? AND is_missing = false",
            )
            .map_err(|e| DbError::get_active_files(root, e))?;

        let rows = stmt
            .query_map([root], |row| row.get::<_, String>(0))
            .map_err(|e| DbError::get_active_files(root, e))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| DbError::get_active_files(root, e))?);
        }
        Ok(results)
    }

    fn upsert_file_batch(&self, root: &str, entries: &[StatEntry]) -> Result<()> {
        let conn = self.conn()?;

        for entry in entries {
            let file_id = get_or_create_file_id(&conn, &entry.path)?;
            insert_file_stat(
                &conn,
                file_id,
                root,
                &entry.filename,
                entry.extension.as_deref(),
                entry.size_bytes,
                entry.mtime_secs,
                entry.mtime_nanos,
                &entry.file_type,
                false,
            )?;
        }

        Ok(())
    }

    fn mark_missing_batch(&self, paths: &[&str]) -> Result<()> {
        let conn = self.conn()?;
        for path in paths {
            insert_missing_stat(&conn, path)?;
        }
        Ok(())
    }
}

impl MetadataScanStore for Db {
    fn get_files_needing_metadata(&self) -> Result<Vec<(i64, String, String)>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT v.id, v.path, v.file_type
                 FROM v_files v
                 WHERE v.is_missing = false
                   AND v.file_type IN ('image', 'video')
                   AND NOT EXISTS (
                       SELECT 1 FROM file_metadata md WHERE md.file_id = v.id
                   )",
            )
            .map_err(|e| DbError::query("get_files_needing_metadata", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| DbError::query("get_files_needing_metadata", e))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| DbError::query("get_files_needing_metadata", e))?);
        }
        Ok(results)
    }

    fn persist_metadata_batch(&self, entries: &[(i64, Vec<MetadataEntry>)]) -> Result<()> {
        let conn = self.conn()?;
        for (file_id, metadata) in entries {
            if !metadata.is_empty() {
                super::metadata_repo::insert_metadata_batch_impl(&conn, *file_id, metadata)?;
            }
        }
        Ok(())
    }
}

impl HashScanStore for Db {
    fn get_files_needing_hash(
        &self,
        hash_type: &str,
        filter: Option<&str>,
    ) -> Result<Vec<(i64, String)>> {
        let conn = self.conn()?;
        super::hashes::get_files_needing_hash_impl(&conn, hash_type, filter)
    }

    fn persist_hash_batch(
        &self,
        hash_type: &str,
        results: &[(i64, std::result::Result<HashResult, String>)],
    ) -> Result<()> {
        let conn = self.conn()?;

        for (file_id, result) in results {
            match result {
                Ok(HashResult::Hex(hex)) => {
                    super::hashes::set_hash_impl(&conn, *file_id, hash_type, hex)?;
                }
                Ok(HashResult::Perceptual(value)) => {
                    super::hashes::set_perceptual_hash_impl(&conn, *file_id, hash_type, *value)?;
                }
                Err(e) => {
                    tracing::warn!("PERSIST_HASH: SKIPPED. file_id: {}, hash_type: {}, reason: {}", file_id, hash_type, e);
                }
            }
        }

        Ok(())
    }
}
