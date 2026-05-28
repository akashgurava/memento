use std::fs;
use std::path::PathBuf;

use memento::config::AppConfig;
use memento::scanner::level1::run_stats_scan;
use memento::scanner::progress::NoopReporter;
use memento::scanner::walk::{classify_extension, walk_directory};
use memento::tokio_util::sync::CancellationToken;

fn test_images_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("test-images");
    path
}

// --- classify_extension ---

#[test]
fn classify_image_extensions() {
    let config = AppConfig::default();
    assert_eq!(classify_extension("jpg", &config), "image");
    assert_eq!(classify_extension("jpeg", &config), "image");
    assert_eq!(classify_extension("png", &config), "image");
    assert_eq!(classify_extension("tiff", &config), "image");
    assert_eq!(classify_extension("bmp", &config), "image");
    assert_eq!(classify_extension("gif", &config), "image");
    assert_eq!(classify_extension("webp", &config), "image");
}

#[test]
fn classify_video_extensions() {
    let config = AppConfig::default();
    assert_eq!(classify_extension("mp4", &config), "video");
    assert_eq!(classify_extension("mov", &config), "video");
    assert_eq!(classify_extension("avi", &config), "video");
    assert_eq!(classify_extension("mkv", &config), "video");
}

#[test]
fn classify_case_insensitive() {
    let config = AppConfig::default();
    assert_eq!(classify_extension("JPG", &config), "image");
    assert_eq!(classify_extension("Png", &config), "image");
    assert_eq!(classify_extension("MP4", &config), "video");
    assert_eq!(classify_extension("MKV", &config), "video");
}

#[test]
fn classify_unknown_extension() {
    let config = AppConfig::default();
    assert_eq!(classify_extension("txt", &config), "other");
    assert_eq!(classify_extension("pdf", &config), "other");
    assert_eq!(classify_extension("rs", &config), "other");
    assert_eq!(classify_extension("", &config), "other");
}

// --- walk_directory ---

#[test]
fn walk_finds_all_files_in_test_images() {
    let entries = walk_directory(&test_images_dir());
    // We know there are 27 files in test-images (26 images + unknown.txt)
    assert!(
        entries.len() >= 26,
        "expected at least 26 files, got {}",
        entries.len()
    );
}

#[test]
fn walk_entries_are_all_files() {
    let entries = walk_directory(&test_images_dir());
    for entry in &entries {
        assert!(entry.is_file);
    }
}

#[test]
fn walk_entries_have_positive_size() {
    let entries = walk_directory(&test_images_dir());
    for entry in &entries {
        assert!(entry.size_bytes > 0, "file {} has zero size", entry.path);
    }
}

#[test]
fn walk_entries_have_mtime() {
    let entries = walk_directory(&test_images_dir());
    for entry in &entries {
        assert!(entry.mtime_secs > 0, "file {} has zero mtime", entry.path);
    }
}

#[test]
fn walk_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let entries = walk_directory(dir.path());
    assert!(entries.is_empty());
}

#[test]
fn walk_nonexistent_dir_returns_empty() {
    let entries = walk_directory(std::path::Path::new("/nonexistent/path/xyz"));
    assert!(entries.is_empty());
}

#[test]
fn walk_includes_hidden_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".hidden_photo.jpg"), b"fake image").unwrap();
    fs::write(dir.path().join("normal.jpg"), b"fake image").unwrap();

    let entries = walk_directory(dir.path());
    assert_eq!(entries.len(), 2);
}

#[test]
fn walk_recurses_subdirectories() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("subdir");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join("top.jpg"), b"data").unwrap();
    fs::write(sub.join("nested.jpg"), b"data").unwrap();

    let entries = walk_directory(dir.path());
    assert_eq!(entries.len(), 2);
}

// --- run_stats_scan ---

#[test]
fn stats_scan_with_test_images() {
    let mut config = AppConfig::default();
    config.scan.roots = vec![test_images_dir().to_string_lossy().to_string()];

    let cancel = CancellationToken::new();
    let stats = run_stats_scan(&config, 0, &NoopReporter, &cancel).unwrap();

    // 26 image files + 1 txt = 27 total
    assert!(stats.total_files >= 26);
    assert!(stats.image_count >= 25); // most are images
    assert_eq!(stats.other_count, 1); // unknown.txt
    assert_eq!(stats.video_count, 0);
}

#[test]
fn stats_scan_size_invariant() {
    let mut config = AppConfig::default();
    config.scan.roots = vec![test_images_dir().to_string_lossy().to_string()];

    let cancel = CancellationToken::new();
    let stats = run_stats_scan(&config, 0, &NoopReporter, &cancel).unwrap();

    assert_eq!(
        stats.total_files,
        stats.image_count + stats.video_count + stats.other_count,
        "file counts must add up"
    );
    assert_eq!(
        stats.total_size_bytes,
        stats.image_size_bytes + stats.video_size_bytes + stats.other_size_bytes,
        "size bytes must add up"
    );
}

#[test]
fn stats_scan_empty_roots() {
    let config = AppConfig::default(); // no roots configured
    let cancel = CancellationToken::new();
    let stats = run_stats_scan(&config, 0, &NoopReporter, &cancel).unwrap();

    assert_eq!(stats.total_files, 0);
    assert_eq!(stats.total_size_bytes, 0);
}

#[test]
fn stats_scan_nonexistent_root_skipped() {
    let mut config = AppConfig::default();
    config.scan.roots = vec!["/nonexistent/path/12345".into()];

    let cancel = CancellationToken::new();
    let stats = run_stats_scan(&config, 0, &NoopReporter, &cancel).unwrap();

    assert_eq!(stats.total_files, 0);
}

#[test]
fn stats_scan_cancellation() {
    let mut config = AppConfig::default();
    config.scan.roots = vec![test_images_dir().to_string_lossy().to_string()];

    let cancel = CancellationToken::new();
    cancel.cancel(); // pre-cancel

    let err = run_stats_scan(&config, 0, &NoopReporter, &cancel).unwrap_err();
    assert!(err.to_string().contains("SCAN_CANCELLED"));
}

#[test]
fn stats_scan_multiple_roots() {
    let dir1 = tempfile::tempdir().unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    fs::write(dir1.path().join("a.jpg"), b"img1").unwrap();
    fs::write(dir2.path().join("b.png"), b"img2").unwrap();

    let mut config = AppConfig::default();
    config.scan.roots = vec![
        dir1.path().to_string_lossy().to_string(),
        dir2.path().to_string_lossy().to_string(),
    ];

    let cancel = CancellationToken::new();
    let stats = run_stats_scan(&config, 0, &NoopReporter, &cancel).unwrap();

    assert_eq!(stats.total_files, 2);
    assert_eq!(stats.image_count, 2);
}
