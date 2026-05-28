use memento::db;
use memento::db::queries;

fn setup_db() -> duckdb::Connection {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.duckdb");
    let conn = db::init_db(&db_path).unwrap();
    // Leak the tempdir so it persists for the duration of the test
    std::mem::forget(dir);
    conn
}

// --- init_db / migrations ---

#[test]
fn init_db_creates_tables() {
    let conn = setup_db();

    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM information_schema.tables WHERE table_name IN ('files', 'file_metadata', 'scan_runs', 'schema_migrations')")
        .unwrap()
        .query_row([], |r| r.get(0))
        .unwrap();

    assert_eq!(count, 4);
}

#[test]
fn init_db_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.duckdb");

    let _conn1 = db::init_db(&db_path).unwrap();
    drop(_conn1);
    let conn2 = db::init_db(&db_path).unwrap();

    // Should still have exactly one migration record
    let count: i64 = conn2
        .prepare("SELECT COUNT(*) FROM schema_migrations")
        .unwrap()
        .query_row([], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

// --- upsert_file ---

#[test]
fn upsert_file_insert_new() {
    let conn = setup_db();
    let id = queries::upsert_file(
        &conn,
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
fn upsert_file_returns_same_id_on_update() {
    let conn = setup_db();

    let id1 = queries::upsert_file(
        &conn,
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

    let id2 = queries::upsert_file(
        &conn,
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

    // Verify size was updated
    let size: i64 = conn
        .prepare("SELECT size_bytes FROM files WHERE id = ?")
        .unwrap()
        .query_row([id1], |r| r.get(0))
        .unwrap();
    assert_eq!(size, 2048);
}

#[test]
fn upsert_file_multiple_get_unique_ids() {
    let conn = setup_db();

    let id1 = queries::upsert_file(
        &conn,
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

    let id2 = queries::upsert_file(
        &conn,
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
    let conn = setup_db();

    let id = queries::upsert_file(
        &conn,
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

    queries::mark_missing(&conn, "/photos/gone.jpg").unwrap();

    let is_missing: bool = conn
        .prepare("SELECT is_missing FROM files WHERE id = ?")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert!(is_missing);
}

#[test]
fn upsert_after_mark_missing_clears_flag() {
    let conn = setup_db();

    queries::upsert_file(
        &conn,
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

    queries::mark_missing(&conn, "/photos/back.jpg").unwrap();

    let id = queries::upsert_file(
        &conn,
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

    let is_missing: bool = conn
        .prepare("SELECT is_missing FROM files WHERE id = ?")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert!(!is_missing);
}

// --- set_hash / set_perceptual_hash ---

#[test]
fn set_hash_blake3() {
    let conn = setup_db();
    let id = queries::upsert_file(
        &conn,
        "/p/x.jpg",
        "/p",
        "x.jpg",
        Some("jpg"),
        100,
        0,
        0,
        "image",
    )
    .unwrap();

    queries::set_hash(
        &conn,
        id,
        "blake3",
        "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
    )
    .unwrap();

    let hash: String = conn
        .prepare("SELECT hash_blake3 FROM files WHERE id = ?")
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
    let conn = setup_db();
    let id = queries::upsert_file(
        &conn,
        "/p/y.jpg",
        "/p",
        "y.jpg",
        Some("jpg"),
        100,
        0,
        0,
        "image",
    )
    .unwrap();

    queries::set_hash(&conn, id, "content_blake3", "deadbeef").unwrap();

    let hash: String = conn
        .prepare("SELECT hash_content_blake3 FROM files WHERE id = ?")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(hash, "deadbeef");
}

#[test]
fn set_hash_invalid_type_errors() {
    let conn = setup_db();
    let id = queries::upsert_file(
        &conn,
        "/p/z.jpg",
        "/p",
        "z.jpg",
        Some("jpg"),
        100,
        0,
        0,
        "image",
    )
    .unwrap();

    let err = queries::set_hash(&conn, id, "phash", "value").unwrap_err();
    assert!(err.to_string().contains("HASH_UNKNOWN_ALGORITHM"));
}

#[test]
fn set_perceptual_hash_phash() {
    let conn = setup_db();
    let id = queries::upsert_file(
        &conn,
        "/p/a.jpg",
        "/p",
        "a.jpg",
        Some("jpg"),
        100,
        0,
        0,
        "image",
    )
    .unwrap();

    queries::set_perceptual_hash(&conn, id, "phash", 12345678).unwrap();

    let hash: i64 = conn
        .prepare("SELECT hash_phash FROM files WHERE id = ?")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(hash, 12345678);
}

#[test]
fn set_perceptual_hash_invalid_type_errors() {
    let conn = setup_db();
    let id = queries::upsert_file(
        &conn,
        "/p/b.jpg",
        "/p",
        "b.jpg",
        Some("jpg"),
        100,
        0,
        0,
        "image",
    )
    .unwrap();

    let err = queries::set_perceptual_hash(&conn, id, "blake3", 999).unwrap_err();
    assert!(err.to_string().contains("HASH_UNKNOWN_ALGORITHM"));
}

// --- invalidate_hashes ---

#[test]
fn invalidate_hashes_clears_all() {
    let conn = setup_db();
    let id = queries::upsert_file(
        &conn,
        "/p/inv.jpg",
        "/p",
        "inv.jpg",
        Some("jpg"),
        100,
        0,
        0,
        "image",
    )
    .unwrap();

    queries::set_hash(&conn, id, "blake3", "abc123").unwrap();
    queries::set_hash(&conn, id, "content_blake3", "def456").unwrap();
    queries::set_perceptual_hash(&conn, id, "phash", 111).unwrap();
    queries::set_perceptual_hash(&conn, id, "dhash", 222).unwrap();
    queries::set_perceptual_hash(&conn, id, "whash", 333).unwrap();

    queries::invalidate_hashes(&conn, id).unwrap();

    let nulls: i64 = conn
        .prepare(
            "SELECT CASE WHEN hash_blake3 IS NULL AND hash_content_blake3 IS NULL
                    AND hash_phash IS NULL AND hash_dhash IS NULL AND hash_whash IS NULL
                    THEN 1 ELSE 0 END FROM files WHERE id = ?",
        )
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(nulls, 1);
}

// --- insert_metadata_batch ---

#[test]
fn insert_metadata_batch_stores_entries() {
    let conn = setup_db();
    let id = queries::upsert_file(
        &conn,
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
        (
            "exif".into(),
            "Make".into(),
            Some("Canon".into()),
            None,
            None,
        ),
        ("exif".into(), "ISO".into(), None, Some(400), None),
        ("exif".into(), "FocalLength".into(), None, None, Some(50.0)),
    ];

    queries::insert_metadata_batch(&conn, id, &entries).unwrap();

    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM file_metadata WHERE file_id = ?")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3);
}

#[test]
fn insert_metadata_batch_replaces_on_second_call() {
    let conn = setup_db();
    let id = queries::upsert_file(
        &conn,
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
        (
            "exif".into(),
            "Make".into(),
            Some("Nikon".into()),
            None,
            None,
        ),
        (
            "exif".into(),
            "Model".into(),
            Some("D850".into()),
            None,
            None,
        ),
    ];
    queries::insert_metadata_batch(&conn, id, &entries1).unwrap();

    let entries2 = vec![(
        "exif".into(),
        "Make".into(),
        Some("Canon".into()),
        None,
        None,
    )];
    queries::insert_metadata_batch(&conn, id, &entries2).unwrap();

    let count: i64 = conn
        .prepare("SELECT COUNT(*) FROM file_metadata WHERE file_id = ?")
        .unwrap()
        .query_row([id], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "second batch should replace first");
}

// --- get_files_needing_hash ---

#[test]
fn get_files_needing_hash_returns_unhashed_files() {
    let conn = setup_db();

    let id1 = queries::upsert_file(
        &conn,
        "/p/a.jpg",
        "/p",
        "a.jpg",
        Some("jpg"),
        100,
        0,
        0,
        "image",
    )
    .unwrap();
    let id2 = queries::upsert_file(
        &conn,
        "/p/b.jpg",
        "/p",
        "b.jpg",
        Some("jpg"),
        200,
        0,
        0,
        "image",
    )
    .unwrap();

    // Hash one of them
    queries::set_hash(&conn, id1, "blake3", "somehash").unwrap();

    let needing = queries::get_files_needing_hash(&conn, "blake3", None).unwrap();
    assert_eq!(needing.len(), 1);
    assert_eq!(needing[0].0, id2);
}

#[test]
fn get_files_needing_hash_with_type_filter() {
    let conn = setup_db();

    queries::upsert_file(
        &conn,
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
    queries::upsert_file(
        &conn,
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

    let images_only = queries::get_files_needing_hash(&conn, "blake3", Some("image")).unwrap();
    assert_eq!(images_only.len(), 1);
    assert!(images_only[0].1.contains("img.jpg"));
}

#[test]
fn get_files_needing_hash_excludes_missing() {
    let conn = setup_db();

    queries::upsert_file(
        &conn,
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
    queries::mark_missing(&conn, "/p/missing.jpg").unwrap();

    let needing = queries::get_files_needing_hash(&conn, "blake3", None).unwrap();
    assert!(needing.is_empty());
}

#[test]
fn get_files_needing_hash_invalid_type_errors() {
    let conn = setup_db();
    let err = queries::get_files_needing_hash(&conn, "sha256", None).unwrap_err();
    assert!(err.to_string().contains("HASH_UNKNOWN_ALGORITHM"));
}

#[test]
fn get_files_needing_hash_ordered_by_size_asc() {
    let conn = setup_db();

    queries::upsert_file(
        &conn,
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
    queries::upsert_file(
        &conn,
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
    queries::upsert_file(
        &conn,
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

    let needing = queries::get_files_needing_hash(&conn, "blake3", None).unwrap();
    assert_eq!(needing.len(), 3);
    assert!(needing[0].1.contains("small"));
    assert!(needing[1].1.contains("med"));
    assert!(needing[2].1.contains("big"));
}

// --- scan_runs ---

#[test]
fn create_and_complete_scan_run() {
    let conn = setup_db();

    let run_id = queries::create_scan_run(&conn, 1, None, Some("/photos")).unwrap();
    assert!(run_id > 0);

    queries::update_scan_progress(&conn, run_id, 50, Some(100)).unwrap();

    let progress: i64 = conn
        .prepare("SELECT files_processed FROM scan_runs WHERE id = ?")
        .unwrap()
        .query_row([run_id], |r| r.get(0))
        .unwrap();
    assert_eq!(progress, 50);

    queries::complete_scan_run(&conn, run_id, "completed", None).unwrap();

    let status: String = conn
        .prepare("SELECT status FROM scan_runs WHERE id = ?")
        .unwrap()
        .query_row([run_id], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "completed");
}

#[test]
fn scan_run_ids_are_monotonic() {
    let conn = setup_db();

    let id1 = queries::create_scan_run(&conn, 1, None, None).unwrap();
    let id2 = queries::create_scan_run(&conn, 2, Some("blake3"), None).unwrap();
    let id3 = queries::create_scan_run(&conn, 3, Some("phash"), Some("/root")).unwrap();

    assert!(id2 > id1);
    assert!(id3 > id2);
}
