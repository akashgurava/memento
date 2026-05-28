use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use clap::{Parser, Subcommand};
use tokio_util::sync::CancellationToken;

use memento::config::{self, AppConfig};
use memento::db;
use memento::scanner::progress::{ProgressReporter, ScanProgress};
use memento::scanner::{level1, level2, level3};

struct CliProgressReporter;

impl ProgressReporter for CliProgressReporter {
    fn report(&self, progress: &ScanProgress) {
        match progress.status.as_str() {
            "completed" => {
                println!(
                    "[level {}] Completed. {} files processed in {:.1}s",
                    progress.level, progress.files_processed, progress.elapsed_secs
                );
            }
            _ => {
                let total = progress
                    .files_total
                    .map(|t| format!("/{}", t))
                    .unwrap_or_default();
                print!(
                    "\r[level {}] {}{} files processed ({:.1}s)    ",
                    progress.level, progress.files_processed, total, progress.elapsed_secs
                );
            }
        }
    }
}

#[derive(Parser)]
#[command(name = "memento", about = "Photo library manager CLI")]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "config.toml")]
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
    /// Run Level 1 stats scan (fast file counting)
    Stats,
    /// Run Level 2 metadata scan (incremental)
    Metadata,
    /// Run Level 3 hash scan for a specific algorithm
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
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "memento=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let config_path = cli.config.clone();

    let app_config = config::load_from(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config: {}", e);
        std::process::exit(1);
    });

    // Priority: --db flag > config.db_path > ./memento.duckdb
    let db_path = cli.db.unwrap_or_else(|| {
        app_config
            .db_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("memento.duckdb"))
    });

    match cli.command {
        Commands::Config { action } => handle_config(action, app_config, &config_path),
        Commands::Stats => handle_stats(app_config, &db_path),
        Commands::Scan { action } => handle_scan(action, app_config, &db_path),
        Commands::Dupes { hash_type } => handle_dupes(app_config, &db_path, &hash_type),
    }
}

fn handle_config(action: ConfigAction, config: AppConfig, config_path: &Path) {
    match action {
        ConfigAction::Show => {
            let toml_str = toml::to_string_pretty(&config).expect("Failed to serialize config");
            println!("{}", toml_str);
        }
        ConfigAction::SetRoots { paths } => {
            let mut config = config;
            config.scan.roots = paths.clone();
            config::save_to(&config, config_path).unwrap_or_else(|e| {
                eprintln!("Failed to save config: {}", e);
                std::process::exit(1);
            });
            println!("Scan roots updated: {:?}", paths);
        }
    }
}

fn handle_stats(config: AppConfig, db_path: &Path) {
    if !db_path.exists() {
        println!(
            "No database found at {}. Run a scan first.",
            db_path.display()
        );
        return;
    }

    let conn = db::init_db(db_path).unwrap_or_else(|e| {
        eprintln!("Failed to open database: {}", e);
        std::process::exit(1);
    });

    let total: i64 = conn
        .prepare("SELECT COUNT(*) FROM files WHERE is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    let total_size: i64 = conn
        .prepare("SELECT COALESCE(SUM(size_bytes), 0) FROM files WHERE is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    let images: i64 = conn
        .prepare("SELECT COUNT(*) FROM files WHERE file_type = 'image' AND is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    let videos: i64 = conn
        .prepare("SELECT COUNT(*) FROM files WHERE file_type = 'video' AND is_missing = false")
        .and_then(|mut s| s.query_row([], |r| r.get(0)))
        .unwrap_or(0);

    println!("Library Statistics:");
    println!("  Total files: {}", total);
    println!(
        "  Total size:  {:.2} GB",
        total_size as f64 / 1_073_741_824.0
    );
    println!("  Images:      {}", images);
    println!("  Videos:      {}", videos);
    println!("  Other:       {}", total - images - videos);

    let _ = config;
}

fn handle_scan(action: ScanAction, config: AppConfig, db_path: &Path) {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = db::init_db(db_path).unwrap_or_else(|e| {
        eprintln!("Failed to initialize database: {}", e);
        std::process::exit(1);
    });

    let db = Arc::new(Mutex::new(conn));
    let reporter = CliProgressReporter;
    let cancel_token = CancellationToken::new();

    match action {
        ScanAction::Stats => {
            println!("Running Level 1 stats scan...");
            match level1::run_stats_scan(&config, 0, &reporter, &cancel_token) {
                Ok(stats) => {
                    println!("\nResults:");
                    println!("  Total files: {}", stats.total_files);
                    println!(
                        "  Total size:  {:.2} GB",
                        stats.total_size_bytes as f64 / 1_073_741_824.0
                    );
                    println!(
                        "  Images:      {} ({:.2} GB)",
                        stats.image_count,
                        stats.image_size_bytes as f64 / 1_073_741_824.0
                    );
                    println!(
                        "  Videos:      {} ({:.2} GB)",
                        stats.video_count,
                        stats.video_size_bytes as f64 / 1_073_741_824.0
                    );
                    println!(
                        "  Other:       {} ({:.2} GB)",
                        stats.other_count,
                        stats.other_size_bytes as f64 / 1_073_741_824.0
                    );
                }
                Err(e) => eprintln!("Scan failed: {}", e),
            }
        }
        ScanAction::Metadata => {
            println!("Running Level 2 metadata scan...");
            match level2::run_metadata_scan(&config, &db, 0, &reporter, &cancel_token) {
                Ok(()) => println!("\nMetadata scan complete."),
                Err(e) => eprintln!("Scan failed: {}", e),
            }
        }
        ScanAction::Hash { algo } => {
            println!("Running Level 3 hash scan (algorithm: {})...", algo);
            match level3::run_hash_scan(&config, &db, 0, &algo, &reporter, &cancel_token) {
                Ok(()) => println!("\nHash scan complete."),
                Err(e) => eprintln!("Scan failed: {}", e),
            }
        }
    }
}

fn handle_dupes(config: AppConfig, db_path: &Path, hash_type: &str) {
    let column = match hash_type {
        "blake3" => "hash_blake3",
        "content_blake3" => "hash_content_blake3",
        _ => {
            eprintln!(
                "Invalid hash type: {}. Use blake3 or content_blake3.",
                hash_type
            );
            std::process::exit(1);
        }
    };

    let conn = db::init_db(db_path).unwrap_or_else(|e| {
        eprintln!("Failed to open database: {}", e);
        std::process::exit(1);
    });

    let sql = format!(
        "SELECT {col}, COUNT(*) as cnt, SUM(size_bytes) as total_size
         FROM files WHERE {col} IS NOT NULL AND is_missing = false
         GROUP BY {col} HAVING COUNT(*) > 1
         ORDER BY total_size DESC
         LIMIT 50",
        col = column
    );

    let mut stmt = conn.prepare(&sql).unwrap_or_else(|e| {
        eprintln!("Query failed: {}", e);
        std::process::exit(1);
    });

    let groups: Vec<(String, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .unwrap_or_else(|e| {
            eprintln!("Query failed: {}", e);
            std::process::exit(1);
        })
        .filter_map(|r| r.ok())
        .collect();

    if groups.is_empty() {
        println!("No duplicates found (hash type: {}).", hash_type);
        return;
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

    let _ = config;
}
