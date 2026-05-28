# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Memento is a Tauri v2 desktop app for cleaning up and consolidating a large photo library (~1TB). It serves as both a photo/media library manager and a deduplication tool. The primary workflow: scan → index → hash → find duplicates → consolidate.

**Key decisions:**
- Database: DuckDB (bundled, single file at `~/Library/Application Support/xyz.225274.memento/memento.duckdb`)
- Config: YAML file (same directory, `config.yaml`) — scan roots specified here, not via UI picker
- Hashing (images): BLAKE3 full-file, BLAKE3 content-only (pixels without metadata), pHash, dHash, wHash
- Hashing (videos): BLAKE3 full-file only (metadata + size sufficient for video dedup)
- Metadata: Store everything available (EAV table for raw tags — EXIF, XMP, IPTC, video)
- Performance: Comprehensiveness over speed; not time-constrained

## Development Commands

```bash
# Start Tauri dev mode (launches both Vite dev server and native window)
bun run tauri dev

# Frontend only (Vite dev server at http://localhost:1420)
bun run dev

# Type checking (frontend)
bun run check

# Build entire workspace (library + Tauri)
cargo build --workspace

# Build CLI binary only
cargo build -p memento --features cli

# Run CLI
./target/debug/memento --help

# Build the production app
bun run tauri build
```

**Build notes:**
- First `cargo build` is slow (~5-10 min) due to DuckDB `bundled` feature compiling from source
- `img_hash` 3.2.0 depends on `image` 0.23 — our `image` dep is pinned to 0.23 to match
- Video metadata extraction requires `ffprobe` in PATH (from FFmpeg)

## Architecture

**Cargo workspace** with two members:

1. **`memento/`** — Core library + CLI binary (feature-gated). All business logic lives here, Tauri-agnostic.
2. **`src-tauri/`** — Thin Tauri GUI shell. Imports from `memento::`, implements Tauri-specific bridges.

### Frontend (`src/`)
SvelteKit in SPA mode (SSR disabled via `+layout.ts`), using `adapter-static` with `index.html` fallback. Svelte 5 runes syntax (`$state`, etc.).

### Library (`memento/src/`)

```
lib.rs              — pub mod declarations, re-exports (duckdb, tokio_util)
main.rs             — CLI entry point (behind "cli" feature, requires clap + tracing-subscriber)
error.rs            — Structured errors: ErrorContext, ErrorInfo trait, impl_err_from_info! macro, domain enums
config/
  mod.rs            — load/save YAML config, platform path resolution
  schema.rs         — AppConfig serde structs with defaults
db/
  mod.rs            — init_db(), Connection setup
  migrations.rs     — Versioned SQL schema (schema_migrations tracking table)
  queries.rs        — Typed DB helpers (upsert_file, set_hash, insert_metadata_batch, etc.)
scanner/
  mod.rs            — Module declarations
  walk.rs           — Filesystem walking (ignore crate), extension classification
  progress.rs       — ProgressReporter trait + ScanProgress struct (no Tauri dep)
  level1.rs         — Stats scan (fast count + size by type)
  level2.rs         — Incremental metadata scan (mtime + size change detection)
  level3.rs         — Hash scan coordinator (parallel via rayon)
metadata/
  mod.rs            — extract_metadata() dispatch by file type
  image.rs          — EXIF extraction (kamadak-exif), all tags as EAV entries
  video.rs          — ffprobe JSON parsing
hashing/
  mod.rs            — HashAlgo enum, compute_hash() dispatch
  blake3_hash.rs    — Full-file (streaming 1MB chunks) + content-only (decode pixels, hash)
  perceptual.rs     — pHash, dHash, wHash via img_hash crate (64-bit hashes)
```

### Tauri Shell (`src-tauri/src/`)

```
lib.rs              — Tauri builder, state registration, command wiring
main.rs             — Binary entry point
state.rs            — AppState (wraps memento types: Arc<Mutex<Connection>>, AppConfig)
commands/
  mod.rs            — Re-exports all commands
  config_cmds.rs    — get_config, set_scan_roots
  scan_cmds.rs      — start_scan, cancel_scan + TauriProgressReporter (bridges trait → app.emit)
  library_cmds.rs   — get_library_stats
  dedup_cmds.rs     — find_exact_duplicates, find_near_duplicates, get_duplicate_summary
```

### Progress Reporting (Key Design)

Scanners accept `&dyn ProgressReporter` instead of `AppHandle`:
- **TauriProgressReporter** (in src-tauri) — emits `"scan:progress"` events
- **CliProgressReporter** (in memento/main.rs) — prints to terminal
- **NoopReporter** — for tests

### CLI (`memento`)

Built with `cargo build -p memento --features cli`. Commands:
```
memento scan stats|metadata|hash <algo>
memento config show|set-roots <paths...>
memento stats
memento dupes <hash_type>
```

### Scan Levels (progressive)

1. **Stats scan** — Fast filesystem walk, counts + sizes by type. Feeds landing page.
2. **Metadata scan** — Incremental (mtime + size detection). Extracts all EXIF/XMP/IPTC/video tags. Runs after Level 1.
3. **Hash scans** — Each algo independently triggered: `blake3`, `content_blake3`, `phash`, `dhash`, `whash`.

### IPC

- Frontend calls Rust commands via `invoke()` from `@tauri-apps/api/core`
- Long-running scans emit `"scan:progress"` events (frontend subscribes via `listen()`)
- Cancellation via `CancellationToken` (tokio-util)

### DuckDB Schema (3 core tables)

- `files` — One row per path. Contains type, size, mtime, promoted metadata fields, nullable hash columns.
- `file_metadata` — EAV store for all raw tags: `(file_id, namespace, tag, value_text, value_int, value_real)`.
- `scan_runs` — Scan history and progress tracking.

Perceptual hashes stored as BIGINT (64-bit) for fast Hamming distance via XOR + popcount.

## Error Pattern

Servir-style structured errors (no thiserror):
- `ErrorContext` struct with optional message + optional `Identifier { kind, value }`
- `ErrorInfo` trait: `error_id() -> &'static str`, `context() -> ErrorContext`
- `impl_err_from_info!` macro generates Display and Error
- Domain enums (`DbError`, `ConfigError`, `ScanError`, `HashError`, `MetadataError`) wrap into top-level `MementoError`
- Constructor methods on enums return `MementoError` directly (e.g., `DbError::query("msg")`)

## Key Constraints

- Vite dev server must run on port 1420 (Tauri expects this fixed port)
- No SSR — all SvelteKit routes must have `export const ssr = false` in their layout
- `src-tauri/` crate types: `staticlib`, `cdylib`, `rlib` for cross-platform Tauri compatibility
- `image` crate pinned to 0.23 (must match `img_hash` 3.x dependency)
- DuckDB is single-writer — `Arc<Mutex<Connection>>` serializes writes
- `memento` re-exports `duckdb` and `tokio_util` so `src-tauri` doesn't duplicate heavy deps
- CLI feature gate: `cargo build -p memento --features cli` (clap + tracing-subscriber are optional deps)
