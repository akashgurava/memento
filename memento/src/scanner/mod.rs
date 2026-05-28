//! Progressive scanner pipeline.
//!
//! Scanning is split into three independent stages, each building on the previous:
//!
//! - **Stats** ([`stats`]) — Fast filesystem walk. Counts files and sizes by type.
//!   No DB writes. Feeds the landing page.
//! - **Metadata** ([`metadata_scan`]) — Incremental metadata scan. Detects new/modified
//!   files via mtime + size comparison, extracts EXIF/video metadata, persists to store.
//! - **Hash** ([`hash_scan`]) — Computes one hash algorithm per invocation
//!   (blake3, content_blake3, phash, dhash, whash). Parallel via rayon.
//!
//! All scan functions accept a [`progress::ProgressReporter`] trait object and a
//! [`CancellationToken`](tokio_util::sync::CancellationToken) for cooperative cancellation.

pub mod hash_scan;
pub mod metadata_scan;
pub mod progress;
pub mod stats;
pub mod store;
pub mod walk;
