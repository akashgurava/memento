# memento

Photo library manager and deduplication engine. Scans, indexes, hashes, and identifies duplicate media files across large collections.

## Prerequisites

- **ffprobe** (from [FFmpeg](https://ffmpeg.org/download.html)) — required for video metadata extraction. Must be in `PATH`.
  - macOS: `brew install ffmpeg`
  - Windows: download from ffmpeg.org, add `bin/` to PATH
  - Linux: `apt install ffmpeg` / `pacman -S ffmpeg`

- **DuckDB** (Windows only)
  - Release zips include `duckdb.dll` alongside the exe — no setup needed
  - On macOS/Linux the `bundled` feature (default) statically links DuckDB — single binary, no runtime deps
  - Building from source on Windows: set `DUCKDB_LIB_DIR` and `DUCKDB_INCLUDE_DIR` to the extracted [DuckDB release](https://github.com/duckdb/duckdb/releases) directory

Without ffprobe, video files will still be scanned and hashed, but video-specific metadata (duration, codec, resolution) will not be extracted.

## CLI Usage

### Configuration

Copy `config.sample.toml` to `config.toml` in your working directory and edit the `roots` field. See `config.sample.toml` for all available options.

```bash
# Set roots (macOS/Linux)
memento config set-roots ~/Photos /Volumes/Backup/Photos

# Set roots (Windows — use forward slashes, quote paths with spaces)
memento config set-roots "C:/Users/You/Pictures" "D:/My Photos/Backup"

# View current config
memento config show
```

Options:
- `--config path/to/config.toml` — use a specific config file (default: `./config.toml`)
- `--db path/to/memento.duckdb` — override database path (default resolution: `--db` flag > `db_path` in config > `./memento.duckdb`)

### Scanning

```bash
memento scan stats              # Level 1 — count files and sizes by type
memento scan metadata           # Level 2 — extract EXIF/video metadata (incremental)
memento scan hash <algo>        # Level 3 — compute hashes (blake3|content_blake3|phash|dhash|whash)
```

### Querying

```bash
memento stats                   # Show library statistics from DB
memento dupes <hash_type>       # List duplicate groups (blake3|content_blake3)
```

### Full pipeline example

```bash
cp config.sample.toml config.toml
memento config set-roots ~/Photos /Volumes/Backup/Photos    # macOS/Linux
# memento config set-roots C:/Users/You/Pictures            # Windows
memento scan stats
memento scan metadata
memento scan hash blake3
memento scan hash content_blake3
memento scan hash phash
memento dupes blake3
```

### Environment

- `RUST_LOG` — Control log verbosity (default: `memento=info`)

## Hashing strategies

| Algorithm | Output | Use case |
|---|---|---|
| `blake3` | 256-bit hex | Exact byte-for-byte duplicates |
| `content_blake3` | 256-bit hex | Same pixels, different metadata (re-exported, stripped EXIF) |
| `phash` | 64-bit int | Perceptual similarity (resized, recompressed) |
| `dhash` | 64-bit int | Gradient-based perceptual (rotation-sensitive) |
| `whash` | 64-bit int | Wavelet-based perceptual (most tolerant) |

Perceptual hashes are compared via Hamming distance (XOR + popcount). Distance 0 = identical, <10 = likely same image.

## Library usage

| Module | Description |
|---|---|
| `scanner` | Multi-level progressive scanning — stats, metadata, hashing |
| `hashing` | BLAKE3 (full-file + content-only), pHash, dHash, wHash |
| `metadata` | EXIF/XMP/IPTC extraction (images), ffprobe parsing (videos) |
| `db` | DuckDB schema, migrations, typed query helpers |
| `config` | TOML-based configuration with platform-aware paths |
| `error` | Structured errors — domain enums with ErrorContext/ErrorInfo pattern |

```rust
use std::path::Path;
use memento::config;
use memento::db::Db;
use memento::scanner::{level1, level2, level3};
use memento::scanner::progress::NoopReporter;
use memento::tokio_util::sync::CancellationToken;

let config = config::load_from(Path::new("config.toml")).unwrap();
let db_path = config::db_path_relative_to(Path::new("config.toml"));
let db = Db::open(&db_path).unwrap();
let cancel = CancellationToken::new();

// Level 1: stats
let stats = level1::run_stats_scan(&config, 0, &NoopReporter, &cancel).unwrap();

// Level 2: metadata (incremental)
level2::run_metadata_scan(&config, &db, 0, &NoopReporter, &cancel).unwrap();

// Level 3: hash (per algorithm)
level3::run_hash_scan(&config, &db, 0, "blake3", &NoopReporter, &cancel).unwrap();
```

Implement `ProgressReporter` to receive progress updates in your own context (GUI, HTTP, etc).

### Scan levels

| Level | Purpose | Parallelism |
|---|---|---|
| 1 — Stats | Fast count + size by file type | Single-threaded (I/O bound) |
| 2 — Metadata | Incremental EXIF/video metadata extraction | Rayon thread pool |
| 3 — Hash | Per-algorithm hash computation | Rayon thread pool |

## Development

Requires **Rust 1.75+**. First build is slow (~5 min) due to DuckDB compiling from source (`bundled` feature, enabled by default).

```bash
# Build CLI (uses bundled DuckDB — static, no runtime deps)
cargo build -p memento --features cli

# Build without bundled DuckDB (requires libduckdb installed or DUCKDB_LIB_DIR set)
cargo build -p memento --no-default-features --features cli

# Build entire workspace (lib + Tauri GUI)
cargo build --workspace

# Run tests
cargo test -p memento
```

### Windows note

CI builds on Windows use pre-built DuckDB libraries (`--no-default-features`) to avoid MSVC OOM during compilation. The resulting binary requires `duckdb.dll` in the same directory or PATH. Local development can use the `bundled` default for a self-contained binary.

Test images are in `test-images/`.
