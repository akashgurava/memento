use std::path::PathBuf;

use memento::hashing::{compute_hash, HashAlgo, HashResult};

fn test_image(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("test-images");
    path.push(name);
    path.to_string_lossy().to_string()
}

// --- HashAlgo ---

#[test]
fn hash_algo_parse_valid() {
    let cases = vec![
        ("blake3", "blake3"),
        ("content_blake3", "content_blake3"),
        ("phash", "phash"),
        ("dhash", "dhash"),
        ("whash", "whash"),
    ];
    for (input, expected) in cases {
        let algo = HashAlgo::parse(input).unwrap();
        assert_eq!(algo.as_str(), expected);
    }
}

#[test]
fn hash_algo_parse_invalid() {
    let err = HashAlgo::parse("md5").unwrap_err();
    assert!(err.to_string().contains("HASH_UNKNOWN_ALGORITHM"));
}

#[test]
fn hash_algo_roundtrip() {
    let algos = ["blake3", "content_blake3", "phash", "dhash", "whash"];
    for name in algos {
        let algo = HashAlgo::parse(name).unwrap();
        assert_eq!(algo.as_str(), name);
    }
}

// --- blake3 full-file ---

#[test]
fn blake3_full_returns_hex() {
    let result = compute_hash(&HashAlgo::Blake3Full, &test_image("baboon.png")).unwrap();
    match result {
        HashResult::Hex(hex) => {
            assert_eq!(hex.len(), 64, "blake3 hex should be 64 chars");
            assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        }
        _ => panic!("expected HashResult::Hex"),
    }
}

#[test]
fn blake3_full_deterministic() {
    let path = test_image("baboon.png");
    let h1 = compute_hash(&HashAlgo::Blake3Full, &path).unwrap();
    let h2 = compute_hash(&HashAlgo::Blake3Full, &path).unwrap();
    match (h1, h2) {
        (HashResult::Hex(a), HashResult::Hex(b)) => assert_eq!(a, b),
        _ => panic!("expected Hex results"),
    }
}

#[test]
fn blake3_full_different_files_differ() {
    let h1 = compute_hash(&HashAlgo::Blake3Full, &test_image("baboon.png")).unwrap();
    let h2 = compute_hash(&HashAlgo::Blake3Full, &test_image("boat.png")).unwrap();
    match (h1, h2) {
        (HashResult::Hex(a), HashResult::Hex(b)) => assert_ne!(a, b),
        _ => panic!("expected Hex results"),
    }
}

#[test]
fn blake3_full_nonexistent_file_errors() {
    let err = compute_hash(&HashAlgo::Blake3Full, "/nonexistent/file.jpg").unwrap_err();
    assert!(err.to_string().contains("IO_ERROR"));
}

// --- blake3 content-only ---

#[test]
fn content_blake3_returns_hex() {
    let result = compute_hash(&HashAlgo::ContentBlake3, &test_image("baboon.png")).unwrap();
    match result {
        HashResult::Hex(hex) => {
            assert_eq!(hex.len(), 64);
            assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        }
        _ => panic!("expected HashResult::Hex"),
    }
}

#[test]
fn content_blake3_deterministic() {
    let path = test_image("lena.bmp");
    let h1 = compute_hash(&HashAlgo::ContentBlake3, &path).unwrap();
    let h2 = compute_hash(&HashAlgo::ContentBlake3, &path).unwrap();
    match (h1, h2) {
        (HashResult::Hex(a), HashResult::Hex(b)) => assert_eq!(a, b),
        _ => panic!("expected Hex results"),
    }
}

#[test]
fn content_blake3_differs_from_full_blake3() {
    let path = test_image("baboon.png");
    let full = compute_hash(&HashAlgo::Blake3Full, &path).unwrap();
    let content = compute_hash(&HashAlgo::ContentBlake3, &path).unwrap();
    match (full, content) {
        (HashResult::Hex(a), HashResult::Hex(b)) => {
            assert_ne!(a, b, "content-only hash should differ from full-file hash");
        }
        _ => panic!("expected Hex results"),
    }
}

#[test]
fn content_blake3_non_image_file_errors() {
    let err = compute_hash(&HashAlgo::ContentBlake3, &test_image("unknown.txt")).unwrap_err();
    assert!(err.to_string().contains("HASH_DECODE_FAILED"));
}

// --- Perceptual hashes ---

#[test]
fn phash_returns_perceptual() {
    let result = compute_hash(&HashAlgo::PHash, &test_image("baboon.png")).unwrap();
    assert!(matches!(result, HashResult::Perceptual(_)));
}

#[test]
fn dhash_returns_perceptual() {
    let result = compute_hash(&HashAlgo::DHash, &test_image("baboon.png")).unwrap();
    assert!(matches!(result, HashResult::Perceptual(_)));
}

#[test]
fn whash_returns_perceptual() {
    let result = compute_hash(&HashAlgo::WHash, &test_image("baboon.png")).unwrap();
    assert!(matches!(result, HashResult::Perceptual(_)));
}

#[test]
fn perceptual_hash_deterministic() {
    let path = test_image("lena.bmp");
    let h1 = compute_hash(&HashAlgo::PHash, &path).unwrap();
    let h2 = compute_hash(&HashAlgo::PHash, &path).unwrap();
    match (h1, h2) {
        (HashResult::Perceptual(a), HashResult::Perceptual(b)) => assert_eq!(a, b),
        _ => panic!("expected Perceptual results"),
    }
}

#[test]
fn perceptual_hash_similar_images_close() {
    // lena color 256 and 512 are the same subject at different resolutions
    let h1 = compute_hash(&HashAlgo::PHash, &test_image("lena_color_256.tif")).unwrap();
    let h2 = compute_hash(&HashAlgo::PHash, &test_image("lena_color_512.tif")).unwrap();
    match (h1, h2) {
        (HashResult::Perceptual(a), HashResult::Perceptual(b)) => {
            let distance = (a ^ b).count_ones();
            assert!(
                distance <= 10,
                "same subject should have low hamming distance, got {}",
                distance
            );
        }
        _ => panic!("expected Perceptual results"),
    }
}

#[test]
fn perceptual_hash_different_images_distant() {
    let h1 = compute_hash(&HashAlgo::PHash, &test_image("baboon.png")).unwrap();
    let h2 = compute_hash(&HashAlgo::PHash, &test_image("boat.png")).unwrap();
    match (h1, h2) {
        (HashResult::Perceptual(a), HashResult::Perceptual(b)) => {
            let distance = (a ^ b).count_ones();
            assert!(
                distance > 5,
                "different images should have higher hamming distance, got {}",
                distance
            );
        }
        _ => panic!("expected Perceptual results"),
    }
}

#[test]
fn perceptual_hash_non_image_errors() {
    let err = compute_hash(&HashAlgo::PHash, &test_image("unknown.txt")).unwrap_err();
    assert!(err.to_string().contains("HASH_DECODE_FAILED"));
}

#[test]
fn perceptual_gray_vs_color_same_subject() {
    // lena gray 512 and lena color 512 - same subject, different color spaces
    let h1 = compute_hash(&HashAlgo::DHash, &test_image("lena_gray_512.tif")).unwrap();
    let h2 = compute_hash(&HashAlgo::DHash, &test_image("lena_color_512.tif")).unwrap();
    match (h1, h2) {
        (HashResult::Perceptual(a), HashResult::Perceptual(b)) => {
            let distance = (a ^ b).count_ones();
            assert!(
                distance <= 15,
                "same subject gray/color should be somewhat close, got {}",
                distance
            );
        }
        _ => panic!("expected Perceptual results"),
    }
}
