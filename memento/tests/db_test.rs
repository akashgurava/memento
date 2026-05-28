use memento::db::{Db, FileRepository, HashRepository, MetadataRepository, ScanRepository};

fn setup_db() -> Db {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.duckdb");
    let db = Db::open(&db_path).unwrap();
    // Leak the tempdir so it persists for the duration of the test
    std::mem::forget(dir);
    db
}

// --- init / migrations ---

#[test]
fn init_db_creates_tables() {
    let db = setup_db();
    let conn = db.conn().unwrap();

    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM information_schema.tables WHERE table_name IN ('file_master', 'file_stats', 'file_metadata', 'file_hashes', 'scan_runs', 'schema_migrations')")
        .unwrap()
        .query_row([], |r| r.get(0))
        .unwrap();

    assert_eq!(count, 6);
}

#[test]
fn init_db_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.duckdb");

    let _db1 = Db::open(&db_path).unwrap();
    drop(_db1);
    let db2 = Db::open(&db_path).unwrap();

    let conn = db2.conn().unwrap();
    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM schema_migrations")
        .unwrap()
        .query_row([], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

// --- upsert_file (insert observations) ---

#[test]
fn upsert_file_insert_new() {
    let db = setup_db();
    let id = db
        .upsert_file(
            "/photos/img1.jpg",
            "/photos",
            "img1.jpg",
            Some("jpg"),
            1024,
            1700000000,
            0,
            "image",
        )
        .unwrap();
    assert!(id > 0);
}

#[test]
fn upsert_file_returns_same_id_on_second_observation() {
    let db = setup_db();

    let id1 = db
        .upsert_file(
            "/photos/img1.jpg",
            "/photos",
            "img1.jpg",
            Some("jpg"),
            1024,
            1700000000,
            0,
            "image",
        )
        .unwrap();

    let id2 = db
        .upsert_file(
            "/photos/img1.jpg",
            "/photos",
            "img1.jpg",
            Some("jpg"),
            2048,
            1700000001,
            0,
            "image",
        )
        .unwrap();

    assert_eq!(id1, id2);

    // Verify latest state shows new size via v_files
    let conn = db.conn().unwrap();
    let size: i64 = conn
        .prepare("SELECT size_bytes FROM v_files WHERE id = ?")
        .unwrap()
        .query_row([id1], |r| r.get(0))
        .unwrap();
    assert_eq!(size, 2048);

    // Both observations recorded in file_stats
    let stat_count: i64 = conn
        .prepare("SELECT COUNT(*) FROM file_stats WHERE file_id = ?")
        .unwrap()
        .query_row([id1], |r| r.get(0))
        .unwrap();
    assert_eq!(stat_count, 2);
}

#[test]
fn upsert_file_multiple_get_unique_ids() {
    let db = setup_db();

    let id1 = db
        .upsert_file(
            "/photos/a.jpg",
            "/photos",
            "a.jpg",
            Some("jpg"),
            100,
            1700000000,
            0,
            "image",
        )
        .unwrap();

    let id2 = db
        .upsert_file(
            "/photos/b.jpg",
            "/photos",
            "b.jpg",
            Some("jpg"),
            200,
            1700000000,
            0,
            "image",
        )
        .unwrap();

    assert_ne!(id1, id2);
}

// --- mark_missing ---

#[test]
fn mark_missing_sets_flag() {
    let db = setup_db();

    let id = db
        .upsert_file(
            "/photos/gone.jpg",
            "/photos",
            "gone.jpg",
            Some("jpg"),
            512,
            1700000000,
            0,
            "image",
        )
        .unwrap();

    db.mark_missing("/photos/gone.jpg").unwrap();

    let conn = db.conn().unwrap();
    let is_missing: bool = conn
        .prepare("SELECT is_missing FROM v_files WHERE id = ?")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert!(is_missing);
}

#[test]
fn upsert_after_mark_missing_clears_flag() {
    let db = setup_db();

    db.upsert_file(
        "/photos/back.jpg",
        "/photos",
        "back.jpg",
        Some("jpg"),
        512,
        1700000000,
        0,
        "image",
    )
    .unwrap();

    db.mark_missing("/photos/back.jpg").unwrap();

    let id = db
        .upsert_file(
            "/photos/back.jpg",
            "/photos",
            "back.jpg",
            Some("jpg"),
            512,
            1700000000,
            0,
            "image",
        )
        .unwrap();

    let conn = db.conn().unwrap();
    let is_missing: bool = conn
        .prepare("SELECT is_missing FROM v_files WHERE id = ?")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert!(!is_missing);
}

// --- set_hash ---

#[test]
fn set_hash_blake3() {
    let db = setup_db();
    let id = db
        .upsert_file("/p/x.jpg", "/p", "x.jpg", Some("jpg"), 100, 0, 0, "image")
        .unwrap();

    db.set_hash(
        id,
        "blake3",
        "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
    )
    .unwrap();

    let conn = db.conn().unwrap();
    let hash: String = conn
        .prepare("SELECT hash_value FROM v_file_hashes WHERE file_id = ? AND hash_name = 'blake3'")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(
        hash,
        "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
    );
}

#[test]
fn set_hash_content_blake3() {
    let db = setup_db();
    let id = db
        .upsert_file("/p/y.jpg", "/p", "y.jpg", Some("jpg"), 100, 0, 0, "image")
        .unwrap();

    db.set_hash(id, "content_blake3", "deadbeef").unwrap();

    let conn = db.conn().unwrap();
    let hash: String = conn
        .prepare("SELECT hash_value FROM v_file_hashes WHERE file_id = ? AND hash_name = 'content_blake3'")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(hash, "deadbeef");
}

#[test]
fn set_hash_invalid_type_errors() {
    let db = setup_db();
    let id = db
        .upsert_file("/p/z.jpg", "/p", "z.jpg", Some("jpg"), 100, 0, 0, "image")
        .unwrap();

    let err = db.set_hash(id, "sha256", "value").unwrap_err();
    assert!(err.to_string().contains("HASH_UNKNOWN_ALGORITHM"));
}

#[test]
fn set_perceptual_hash_phash() {
    let db = setup_db();
    let id = db
        .upsert_file("/p/a.jpg", "/p", "a.jpg", Some("jpg"), 100, 0, 0, "image")
        .unwrap();

    db.set_perceptual_hash(id, "phash", 12345678).unwrap();

    let conn = db.conn().unwrap();
    let hash: String = conn
        .prepare("SELECT hash_value FROM v_file_hashes WHERE file_id = ? AND hash_name = 'phash'")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(hash, "12345678");
}

#[test]
fn set_perceptual_hash_invalid_type_errors() {
    let db = setup_db();
    let id = db
        .upsert_file("/p/b.jpg", "/p", "b.jpg", Some("jpg"), 100, 0, 0, "image")
        .unwrap();

    let err = db.set_perceptual_hash(id, "sha256", 999).unwrap_err();
    assert!(err.to_string().contains("HASH_UNKNOWN_ALGORITHM"));
}

// --- insert_metadata_batch ---

#[test]
fn insert_metadata_batch_stores_entries() {
    let db = setup_db();
    let id = db
        .upsert_file(
            "/p/meta.jpg",
            "/p",
            "meta.jpg",
            Some("jpg"),
            100,
            0,
            0,
            "image",
        )
        .unwrap();

    let entries = vec![
        ("exif".into(), "Make".into(), Some("Canon".into())),
        ("exif".into(), "ISO".into(), Some("400".into())),
        ("exif".into(), "FocalLength".into(), Some("50.0".into())),
    ];

    db.insert_metadata_batch(id, &entries).unwrap();

    let conn = db.conn().unwrap();
    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM file_metadata WHERE file_id = ?")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3);
}

#[test]
fn insert_metadata_batch_appends_on_second_call() {
    let db = setup_db();
    let id = db
        .upsert_file(
            "/p/meta2.jpg",
            "/p",
            "meta2.jpg",
            Some("jpg"),
            100,
            0,
            0,
            "image",
        )
        .unwrap();

    let entries1 = vec![
        ("exif".into(), "Make".into(), Some("Nikon".into())),
        ("exif".into(), "Model".into(), Some("D850".into())),
    ];
    db.insert_metadata_batch(id, &entries1).unwrap();

    let entries2 = vec![("exif".into(), "Make".into(), Some("Canon".into()))];
    db.insert_metadata_batch(id, &entries2).unwrap();

    let conn = db.conn().unwrap();
    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM file_metadata WHERE file_id = ?")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3, "second batch appends (append-only)");

    // v_file_metadata shows latest value for Make
    let latest_make: String = conn
        .prepare("SELECT value FROM v_file_metadata WHERE file_id = ? AND tag = 'Make'")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(latest_make, "Canon");
}

// --- get_files_needing_hash ---

#[test]
fn get_files_needing_hash_returns_unhashed_files() {
    let db = setup_db();

    let id1 = db
        .upsert_file("/p/a.jpg", "/p", "a.jpg", Some("jpg"), 100, 0, 0, "image")
        .unwrap();
    let id2 = db
        .upsert_file("/p/b.jpg", "/p", "b.jpg", Some("jpg"), 200, 0, 0, "image")
        .unwrap();

    db.set_hash(id1, "blake3", "somehash").unwrap();

    let needing = db.get_files_needing_hash("blake3", None).unwrap();
    assert_eq!(needing.len(), 1);
    assert_eq!(needing[0].0, id2);
}

#[test]
fn get_files_needing_hash_with_type_filter() {
    let db = setup_db();

    db.upsert_file(
        "/p/img.jpg",
        "/p",
        "img.jpg",
        Some("jpg"),
        100,
        0,
        0,
        "image",
    )
    .unwrap();
    db.upsert_file(
        "/p/vid.mp4",
        "/p",
        "vid.mp4",
        Some("mp4"),
        200,
        0,
        0,
        "video",
    )
    .unwrap();

    let images_only = db.get_files_needing_hash("blake3", Some("image")).unwrap();
    assert_eq!(images_only.len(), 1);
    assert!(images_only[0].1.contains("img.jpg"));
}

#[test]
fn get_files_needing_hash_excludes_missing() {
    let db = setup_db();

    db.upsert_file(
        "/p/missing.jpg",
        "/p",
        "missing.jpg",
        Some("jpg"),
        100,
        0,
        0,
        "image",
    )
    .unwrap();
    db.mark_missing("/p/missing.jpg").unwrap();

    let needing = db.get_files_needing_hash("blake3", None).unwrap();
    assert!(needing.is_empty());
}

#[test]
fn get_files_needing_hash_invalid_type_errors() {
    let db = setup_db();
    let err = db.get_files_needing_hash("sha256", None).unwrap_err();
    assert!(err.to_string().contains("HASH_UNKNOWN_ALGORITHM"));
}

#[test]
fn get_files_needing_hash_ordered_by_size_asc() {
    let db = setup_db();

    db.upsert_file(
        "/p/big.jpg",
        "/p",
        "big.jpg",
        Some("jpg"),
        9999,
        0,
        0,
        "image",
    )
    .unwrap();
    db.upsert_file(
        "/p/small.jpg",
        "/p",
        "small.jpg",
        Some("jpg"),
        10,
        0,
        0,
        "image",
    )
    .unwrap();
    db.upsert_file(
        "/p/med.jpg",
        "/p",
        "med.jpg",
        Some("jpg"),
        500,
        0,
        0,
        "image",
    )
    .unwrap();

    let needing = db.get_files_needing_hash("blake3", None).unwrap();
    assert_eq!(needing.len(), 3);
    assert!(needing[0].1.contains("small"));
    assert!(needing[1].1.contains("med"));
    assert!(needing[2].1.contains("big"));
}

// --- scan_runs ---

#[test]
fn create_and_complete_scan_run() {
    let db = setup_db();

    let run_id = db.create_scan_run(1, None, Some("/photos")).unwrap();
    assert!(run_id > 0);

    db.update_scan_progress(run_id, 50, Some(100)).unwrap();

    {
        let conn = db.conn().unwrap();
        let progress: i64 = conn
            .prepare("SELECT files_processed FROM scan_runs WHERE id = ?")
            .unwrap()
            .query_row([run_id], |r| r.get(0))
            .unwrap();
        assert_eq!(progress, 50);
    }

    db.complete_scan_run(run_id, "completed", None).unwrap();

    let conn = db.conn().unwrap();
    let status: String = conn
        .prepare("SELECT status FROM scan_runs WHERE id = ?")
        .unwrap()
        .query_row([run_id], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "completed");
}

#[test]
fn scan_run_ids_are_monotonic() {
    let db = setup_db();

    let id1 = db.create_scan_run(1, None, None).unwrap();
    let id2 = db.create_scan_run(2, Some("blake3"), None).unwrap();
    let id3 = db.create_scan_run(3, Some("phash"), Some("/root")).unwrap();

    assert!(id2 > id1);
    assert!(id3 > id2);
}

// --- file_exists ---

#[test]
fn file_exists_returns_true_for_existing() {
    let db = setup_db();
    db.upsert_file("/p/x.jpg", "/p", "x.jpg", Some("jpg"), 100, 0, 0, "image")
        .unwrap();

    assert!(db.file_exists("/p/x.jpg").unwrap());
    assert!(!db.file_exists("/p/nonexistent.jpg").unwrap());
}

// --- get_active_files_for_root ---

#[test]
fn get_active_files_for_root_excludes_missing() {
    let db = setup_db();
    db.upsert_file(
        "/photos/a.jpg",
        "/photos",
        "a.jpg",
        Some("jpg"),
        100,
        1000,
        0,
        "image",
    )
    .unwrap();
    db.upsert_file(
        "/photos/b.jpg",
        "/photos",
        "b.jpg",
        Some("jpg"),
        200,
        2000,
        0,
        "image",
    )
    .unwrap();
    db.mark_missing("/photos/b.jpg").unwrap();

    let records = db.get_active_files_for_root("/photos").unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].path, "/photos/a.jpg");
}
