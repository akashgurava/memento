# Memento

Photo library manager and deduplication engine. Scans, indexes, hashes, and identifies duplicate media files across large collections (~1TB+).

## Structure

| Crate | Description |
|---|---|
| [`memento/`](memento/) | Core library + CLI binary (feature-gated) |
| `src-tauri/` | Tauri v2 desktop GUI shell |
| `src/` | SvelteKit frontend (SPA) |

## Quick Start (CLI)

```bash
cargo build -p memento --features cli
./target/debug/memento config set-roots ~/Photos /Volumes/Backup
./target/debug/memento scan stats
./target/debug/memento scan metadata
./target/debug/memento scan hash blake3
./target/debug/memento dupes blake3
```

See [`memento/README.md`](memento/README.md) for full CLI and library documentation.

## Development

```bash
# CLI only
cargo build -p memento --features cli

# Full workspace (library + Tauri)
cargo build --workspace

# Tests
cargo test -p memento

# Tauri dev mode (frontend + native window)
bun run tauri dev
```

## Prerequisites

- Rust 1.75+
- [FFmpeg](https://ffmpeg.org/download.html) (`ffprobe` in PATH) — for video metadata
- [Bun](https://bun.sh/) — for frontend dev (Tauri GUI only)
