use duckdb::Connection;

use crate::error::{DbError, Result};

const MIGRATIONS: &[(&str, &str)] = &[("001_initial_schema", MIGRATION_001)];

const MIGRATION_001: &str = r#"
-- file_master: immutable identity table (insert-only, never updated/deleted)
CREATE TABLE IF NOT EXISTS file_master (
    id   BIGINT PRIMARY KEY,
    path VARCHAR NOT NULL UNIQUE
);

CREATE SEQUENCE IF NOT EXISTS file_master_id_seq START 1;

-- file_stats: one row per file per observation (append-only)
CREATE TABLE IF NOT EXISTS file_stats (
    stat_id     BIGINT PRIMARY KEY,
    file_id     BIGINT NOT NULL REFERENCES file_master(id),
    root_dir    VARCHAR NOT NULL,
    filename    VARCHAR NOT NULL,
    extension   VARCHAR,
    size_bytes  BIGINT NOT NULL,
    mtime_secs  BIGINT NOT NULL,
    mtime_nanos INTEGER NOT NULL DEFAULT 0,
    file_type   VARCHAR NOT NULL,
    is_missing  BOOLEAN NOT NULL DEFAULT false,
    observed_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

CREATE SEQUENCE IF NOT EXISTS file_stats_id_seq START 1;

-- file_metadata: append-only EAV store for all raw tags
CREATE TABLE IF NOT EXISTS file_metadata (
    file_id      BIGINT NOT NULL REFERENCES file_master(id),
    stat_id      BIGINT NOT NULL REFERENCES file_stats(stat_id),
    namespace    VARCHAR NOT NULL,
    tag          VARCHAR NOT NULL,
    value        VARCHAR,
    extracted_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- file_hashes: append-only hash store
CREATE TABLE IF NOT EXISTS file_hashes (
    file_id      BIGINT NOT NULL REFERENCES file_master(id),
    stat_id      BIGINT NOT NULL REFERENCES file_stats(stat_id),
    hash_name    VARCHAR NOT NULL,
    hash_value   VARCHAR,
    computed_at  TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
);

-- scan_runs: scan history and progress tracking
CREATE TABLE IF NOT EXISTS scan_runs (
    id              BIGINT PRIMARY KEY,
    scan_level      INTEGER NOT NULL,
    hash_type       VARCHAR,
    root_dir        VARCHAR,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    completed_at    TIMESTAMPTZ,
    status          VARCHAR NOT NULL DEFAULT 'running',
    files_processed BIGINT NOT NULL DEFAULT 0,
    files_total     BIGINT,
    error_message   VARCHAR
);

CREATE SEQUENCE IF NOT EXISTS scan_runs_id_seq START 1;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_file_stats_file_id ON file_stats(file_id);
CREATE INDEX IF NOT EXISTS idx_file_stats_root ON file_stats(root_dir);
CREATE INDEX IF NOT EXISTS idx_metadata_file ON file_metadata(file_id);
CREATE INDEX IF NOT EXISTS idx_metadata_lookup ON file_metadata(file_id, namespace, tag);
CREATE INDEX IF NOT EXISTS idx_hashes_file ON file_hashes(file_id);
CREATE INDEX IF NOT EXISTS idx_hashes_lookup ON file_hashes(file_id, hash_name);

-- View: latest file state per file_id
CREATE OR REPLACE VIEW v_files AS
SELECT m.id, m.path, fs.stat_id, fs.root_dir, fs.filename, fs.extension,
       fs.size_bytes, fs.mtime_secs, fs.mtime_nanos, fs.file_type,
       fs.is_missing, fs.observed_at
FROM file_master m
JOIN file_stats fs ON fs.file_id = m.id
QUALIFY ROW_NUMBER() OVER (PARTITION BY m.id ORDER BY fs.observed_at DESC) = 1;

-- View: latest metadata per file/tag
CREATE OR REPLACE VIEW v_file_metadata AS
SELECT file_id, stat_id, namespace, tag, value, extracted_at
FROM file_metadata
QUALIFY ROW_NUMBER() OVER (PARTITION BY file_id, namespace, tag ORDER BY extracted_at DESC) = 1;

-- View: latest hash per file/hash_name
CREATE OR REPLACE VIEW v_file_hashes AS
SELECT file_id, stat_id, hash_name, hash_value, computed_at
FROM file_hashes
QUALIFY ROW_NUMBER() OVER (PARTITION BY file_id, hash_name ORDER BY computed_at DESC) = 1;
"#;

/// Apply all pending migrations in order. Idempotent — tracks applied
/// migrations in the `schema_migrations` table.
pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name VARCHAR PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
        );",
    )
    .map_err(DbError::migration)?;

    for (name, sql) in MIGRATIONS {
        let applied: bool = conn
            .prepare("SELECT COUNT(*) > 0 FROM schema_migrations WHERE name = ?")
            .and_then(|mut s| s.query_row([name], |row| row.get(0)))
            .map_err(DbError::migration)?;

        if !applied {
            tracing::info!("APPLY_MIGRATION: START. migration: {}", name);
            conn.execute_batch(sql).map_err(DbError::migration)?;
            conn.execute("INSERT INTO schema_migrations (name) VALUES (?)", [name])
                .map_err(DbError::migration)?;
            tracing::info!("APPLY_MIGRATION: SUCCESS. migration: {}", name);
        }
    }

    Ok(())
}
