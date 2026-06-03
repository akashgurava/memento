mod migrations;

use std::path::Path;

use duckdb::{params, Connection};

use crate::{bus::Consumer, error::MementoError, scan::ScanMessage};

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, MementoError> {
        let conn = Connection::open(&path).map_err(|e| {
            MementoError::init(path.as_ref().to_string_lossy().into_owned(), e.to_string())
        })?;

        let db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    /// Apply all pending migrations in order.
    fn run_migrations(&self) -> Result<(), MementoError> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    name VARCHAR PRIMARY KEY,
                    applied_at TIMESTAMPTZ NOT NULL DEFAULT current_timestamp
                );",
            )
            .map_err(|e| MementoError::query("CREATE_SCHEMA_MIGRATIONS", e.to_string()))?;

        for (name, sql) in migrations::MIGRATIONS {
            let applied: bool = self
                .conn
                .prepare("SELECT COUNT(*) > 0 FROM schema_migrations WHERE name = ?")
                .and_then(|mut s| s.query_row([name], |row| row.get(0)))
                .map_err(|e| MementoError::query("CHECK_MIGRATION_APPLIED", e.to_string()))?;

            if !applied {
                tracing::info!("APPLY_MIGRATION: START. migration: {}", name);
                self.conn
                    .execute_batch(sql)
                    .map_err(|e| MementoError::migration(e.to_string()))?;
                self.conn
                    .execute("INSERT INTO schema_migrations (name) VALUES (?)", [name])
                    .map_err(|e| MementoError::migration(e.to_string()))?;
                tracing::info!("APPLY_MIGRATION: SUCCESS. migration: {}", name);
            }
        }

        Ok(())
    }
}

pub struct DbScanConsumer {
    conn: Connection,
    count: u64,
}

impl DbScanConsumer {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, MementoError> {
        let db = Db::new(&path)?;

        db.conn
            .execute_batch("CREATE TEMP TABLE IF NOT EXISTS file_master_temp (path VARCHAR);")
            .map_err(|e| MementoError::query("CREATE_TEMP_TABLE_FILE_MASTER", e.to_string()))?;

        Ok(Self {
            conn: db.conn,
            count: 0,
        })
    }

    pub fn count(&self) -> u64 {
        self.count
    }
}

impl Consumer for DbScanConsumer {
    type Message = ScanMessage;

    fn consume(&mut self, message: &Self::Message) -> Result<(), MementoError> {
        if let ScanMessage::File { file } = message {
            let path_arc = file.path();
            let path_str = path_arc.to_string_lossy();
            self.conn
                .execute(
                    "INSERT INTO file_master_temp (path) VALUES (?)",
                    params![path_str.as_ref()],
                )
                .map_err(|e| MementoError::upsert_file(path_str.into_owned(), e.to_string()))?;
            self.count += 1;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), MementoError> {
        self.conn
            .execute_batch(
                "INSERT INTO file_master (path) SELECT path FROM file_master_temp
                 ON CONFLICT (path) DO NOTHING;
                 DROP TABLE IF EXISTS file_master_temp;",
            )
            .map_err(|e| MementoError::query("MERGE_FROM_TEMP_FILE_MASTER", e.to_string()))?;
        tracing::info!("DB_SCAN_COMPLETE. files_inserted: {}", self.count);
        Ok(())
    }
}
