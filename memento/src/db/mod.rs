pub mod migrations;
pub mod queries;

use std::path::Path;

use duckdb::Connection;

use crate::error::Result;

pub fn init_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    migrations::run_migrations(&conn)?;
    Ok(conn)
}
