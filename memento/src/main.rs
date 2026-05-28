use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use tokio_util::sync::CancellationToken;

use memento::config::{self, AppConfig};
use memento::db::Db;
use memento::error::{DbError, HashError, Result};
use memento::scanner::progress::{ProgressReporter, ScanProgress};
use memento::scanner::{hash_scan, metadata_scan, stats};

struct CliProgressReporter;

impl CliProgressReporter {
    fn term_width() -> usize {
        std::env::var("COLUMNS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(80)
    }

    fn progress_bar(processed: i64, total: i64, bar_width: usize) -> String {
        let ratio = if total > 0 {
            processed as f64 / total as f64
        } else {
            0.0
        };
        let filled = (ratio * bar_width as f64) as usize;
        let empty = bar_width.saturating_sub(filled);
        format!("{}{}", "█".repeat(filled), "░".repeat(empty))
    }
}

impl ProgressReporter for CliProgressReporter {
    fn report(&self, progress: &ScanProgress) {
        match progress.status.as_str() {
            "completed" => {
                let width = Self::term_width();
                print!("\r{}\r", " ".repeat(width));
                println!(
                    "[{}] Completed. {} files in {:.1}s",
                    progress.stage, progress.files_processed, progress.elapsed_secs
                );
            }
            _ => {
                let width = Self::term_width();

                let line = if let Some(total) = progress.files_total {
                    let pct = if total > 0 {
                        (progress.files_processed as f64 / total as f64 * 100.0) as u32
                    } else {
                        0
                    };
                    let bar = Self::progress_bar(progress.files_processed, total, 20);
                    format!(
                        "[{}] {} {}/{} ({}%)",
                        progress.stage, bar, progress.files_processed, total, pct
                    )
                } else {
                    format!(
                        "[{}] {} files ({:.1}s)",
                        progress.stage, progress.files_processed, progress.elapsed_secs
                    )
                };

                let display_width = line.chars().count();
                let pad = " ".repeat(width.saturating_sub(display_width));
                print!("\r{}{}", line, pad);
                let _ = std::io::stdout().flush();
            }
        }
    }
}

#[derive(Parser)]
#[command(name = "memento", about = "Photo library manager CLI")]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "config.yaml")]
    config: PathBuf,

    /// Path to database file (overrides config)
    #[arg(short, long)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run scans
    Scan {
        #[command(subcommand)]
        action: ScanAction,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Show library statistics from DB
    Stats,
    /// Show duplicate file groups
    Dupes {
        /// Hash type: blake3, content_blake3
        hash_type: String,
    },
}

#[derive(Subcommand)]
enum ScanAction {
    /// Run stats scan (fast file counting)
    Stats,
    /// Run metadata scan (incremental)
    Metadata,
    /// Run hash scan for a specific algorithm
    Hash {
        /// Algorithm: blake3, content_blake3, phash, dhash, whash
        algo: String,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set scan root directories
    SetRoots {
        /// Directory paths to scan
        paths: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "memento=info".into()),
        )
        .init();

    run().map_err(|e| {
        tracing::error!("{}", e);
        Box::new(e) as Box<dyn std::error::Error>
    })
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cli.config.clone();
    let app_config = config::load_from(&config_path)?;

    let db_path = cli.db.unwrap_or_else(|| {
        app_config
            .db_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("memento.duckdb"))
    });

    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();
    ctrlc::set_handler(move || {
        cancel_token_clone.cancel();
    })
    .ok();

    match cli.command {
        Commands::Config { action } => handle_config(action, app_config, &config_path),
        Commands::Stats => handle_stats(&db_path),
        Commands::Scan { action } => handle_scan(action, app_config, &db_path, &cancel_token),
        Commands::Dupes { hash_type } => handle_dupes(&db_path, &hash_type),
    }
}

fn handle_config(action: ConfigAction, config: AppConfig, config_path: &Path) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let yaml_str = serde_yml::to_string(&config)?;
            println!("{}", yaml_str);
        }
        ConfigAction::SetRoots { paths } => {
            let mut config = config;
            config.scan.roots = paths.iter().map(|p| p.replace('\\', "/")).collect();
            config::save_to(&config, config_path)?;
            println!("Scan roots updated: {:?}", config.scan.roots);
        }
    }
    Ok(())
}

fn handle_stats(db_path: &Path) -> Result<()> {
    if !db_path.exists() {
        return Err(DbError::init(
            db_path.display().to_string(),
            "database not found (run a scan first)",
        ));
    }

    let db = Db::open(db_path)?;
    let conn = db.conn()?;

    let total: i64 = conn
        .prepare("SELECT COUNT(*) FROM v_files WHERE is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .map_err(|e| DbError::query("show_stats_total", e))?;

    let total_size: i64 = conn
        .prepare("SELECT COALESCE(SUM(size_bytes), 0) FROM v_files WHERE is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .map_err(|e| DbError::query("show_stats_size", e))?;

    let images: i64 = conn
        .prepare("SELECT COUNT(*) FROM v_files WHERE file_type = 'image' AND is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .map_err(|e| DbError::query("show_stats_images", e))?;

    let videos: i64 = conn
        .prepare("SELECT COUNT(*) FROM v_files WHERE file_type = 'video' AND is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .map_err(|e| DbError::query("show_stats_videos", e))?;


    println!("Library Statistics:");
    println!("  Total files: {}", total);
    println!(
        "  Total size:  {:.2} GB",
        total_size as f64 / 1_073_741_824.0
    );
    println!("  Images:      {}", images);
    println!("  Videos:      {}", videos);
    println!("  Other:       {}", total - images - videos);

    Ok(())
}

fn handle_scan(
    action: ScanAction,
    config: AppConfig,
    db_path: &Path,
    cancel_token: &CancellationToken,
) -> Result<()> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let db = Db::open(db_path)?;
    let reporter = CliProgressReporter;

    match action {
        ScanAction::Stats => {
            let library_stats =
                stats::run_stats_scan(&config, &db, 0, &reporter, cancel_token)?;
            println!("\nResults:");
            println!("  Total files: {}", library_stats.total_files);
            println!(
                "  Total size:  {:.2} GB",
                library_stats.total_size_bytes as f64 / 1_073_741_824.0
            );
            println!(
                "  Images:      {} ({:.2} GB)",
                library_stats.image_count,
                library_stats.image_size_bytes as f64 / 1_073_741_824.0
            );
            println!(
                "  Videos:      {} ({:.2} GB)",
                library_stats.video_count,
                library_stats.video_size_bytes as f64 / 1_073_741_824.0
            );
            println!(
                "  Other:       {} ({:.2} GB)",
                library_stats.other_count,
                library_stats.other_size_bytes as f64 / 1_073_741_824.0
            );
        }
        ScanAction::Metadata => {
            metadata_scan::run_metadata_scan(&config, &db, 0, &reporter, cancel_token)?;
        }
        ScanAction::Hash { algo } => {
            hash_scan::run_hash_scan(&config, &db, 0, &algo, &reporter, cancel_token)?;
        }
    }

    Ok(())
}

fn handle_dupes(db_path: &Path, hash_type: &str) -> Result<()> {
    match hash_type {
        "blake3" | "content_blake3" => {}
        _ => return Err(HashError::unknown_algorithm(hash_type)),
    };

    let db = Db::open(db_path)?;
    let conn = db.conn()?;

    let sql = format!(
        "SELECT h.hash_value, COUNT(*) as cnt, SUM(v.size_bytes) as total_size
         FROM v_file_hashes h
         JOIN v_files v ON v.id = h.file_id
         WHERE h.hash_value IS NOT NULL AND h.hash_name = '{ht}' AND v.is_missing = false
         GROUP BY h.hash_value HAVING COUNT(*) > 1
         ORDER BY total_size DESC
         LIMIT 50",
        ht = hash_type
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| DbError::query("find_duplicates_prepare", e))?;

    let groups: Vec<(String, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| DbError::query("find_duplicates_fetch", e))?
        .filter_map(|r| r.ok())
        .collect();

    if groups.is_empty() {
        println!("No duplicates found (hash type: {}).", hash_type);
        return Ok(());
    }

    println!(
        "Found {} duplicate groups (showing top 50):\n",
        groups.len()
    );
    for (hash, count, size) in &groups {
        println!(
            "  {} — {} files, {:.2} MB wasted",
            &hash[..16],
            count,
            (*size as f64 - (*size as f64 / *count as f64)) / 1_048_576.0
        );
    }

    Ok(())
}
