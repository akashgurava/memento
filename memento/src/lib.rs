pub mod config;
pub mod db;
pub mod error;
pub mod hashing;
pub mod metadata;
pub mod scanner;

// Re-export key dependencies for downstream crates
pub use duckdb;
pub use tokio_util;
