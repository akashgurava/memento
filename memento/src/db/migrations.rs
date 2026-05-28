use duckdb::Connection;

use crate::error::Result;

const MIGRATIONS: &[(&str, &str)] = &[("001_initial_schema", MIGRATION_001)];

const MIGRATION_001: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    id              BIGINT PRIMARY KEY,
    path            VARCHAR NOT NULL UNIQUE,
    root_dir        VARCHAR NOT NULL,
    filename        VARCHAR NOT NULL,
    extension       VARCHAR,
    size_bytes      BIGINT NOT NULL,
    mtime_secs      BIGINT NOT NULL,
    mtime_nanos     INTEGER NOT NULL DEFAULT 0,
    file_type       VARCHAR NOT NULL,

    -- Populated by Level 2 metadata scan
    metadata_scanned_at  TIMESTAMPTZ,
    width           INTEGER,
    height          INTEGER,
    duration_secs   DOUBLE,
    capture_ts      TIMESTAMPTZ,
    camera_make     VARCHAR,
    camera_model    VARCHAR,

    -- Hash columns (Level 3 — each nullable until that scan runs)
    hash_blake3         VARCHAR,
    hash_content_blake3 VARCHAR,
    hash_phash          BIGINT,
    hash_dhash          BIGINT,
    hash_whash          BIGINT,

    -- Bookkeeping
    first_seen_at    TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    last_verified_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp,
    is_missing       BOOLEAN NOT NULL DEFAULT false
);

CREATE SEQUENCE IF NOT EXISTS files_id_seq START 1;

CREATE TABLE IF NOT EXISTS file_metadata (
    file_id     BIGINT NOT NULL,
    namespace   VARCHAR NOT NULL,
    tag         VARCHAR NOT NULL,
    value_text  VARCHAR,
    value_int   BIGINT,
    value_real  DOUBLE,
    PRIMARY KEY (file_id, namespace, tag)
);

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

CREATE INDEX IF NOT EXISTS idx_files_root ON files(root_dir);
CREATE INDEX IF NOT EXISTS idx_files_type ON files(file_type);
CREATE INDEX IF NOT EXISTS idx_files_missing ON files(is_missing);
CREATE INDEX IF NOT EXISTS idx_files_blake3 ON files(hash_blake3);
CREATE INDEX IF NOT EXISTS idx_files_phash ON files(hash_phash);
CREATE INDEX IF NOT EXISTS idx_metadata_file ON file_metadata(file_id);
"#;

pub fn run_migrations(conn: &Connection) -> Result<()> {
    // Create migration tracking table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name VARCHAR PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
        );",
    )?;

    for (name, sql) in MIGRATIONS {
        let applied: bool = conn
            .prepare("SELECT COUNT(*) > 0 FROM schema_migrations WHERE name = ?")?
            .query_row([name], |row| row.get(0))?;

        if !applied {
            tracing::info!("Applying migration: {}", name);
            conn.execute_batch(sql)?;
            conn.execute("INSERT INTO schema_migrations (name) VALUES (?)", [name])?;
        }
    }

    Ok(())
}
