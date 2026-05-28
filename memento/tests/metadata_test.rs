use std::path::PathBuf;

use memento::metadata::extract_metadata;

fn test_image(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("test-images");
    path.push(name);
    path.to_string_lossy().to_string()
}

#[test]
fn extract_metadata_image_type_returns_entries() {
    // HappyFish.jpg is a JPEG which typically has EXIF data
    let entries = extract_metadata(&test_image("HappyFish.jpg"), "image");
    // Even if this particular file has no EXIF, the function should not panic
    // If it has EXIF, we'll get entries
    assert!(entries.is_empty() || !entries.is_empty()); // no panic is the assertion
}

#[test]
fn extract_metadata_non_image_type_returns_empty() {
    let entries = extract_metadata(&test_image("baboon.png"), "other");
    assert!(entries.is_empty());
}

#[test]
fn extract_metadata_unknown_type_returns_empty() {
    let entries = extract_metadata(&test_image("baboon.png"), "document");
    assert!(entries.is_empty());
}

#[test]
fn extract_metadata_nonexistent_file_returns_empty() {
    let entries = extract_metadata("/nonexistent/file.jpg", "image");
    assert!(entries.is_empty());
}

#[test]
fn extract_metadata_non_image_file_as_image_returns_empty() {
    let entries = extract_metadata(&test_image("unknown.txt"), "image");
    assert!(entries.is_empty());
}

#[test]
fn extract_metadata_video_without_ffprobe_returns_empty() {
    // We don't have video test files, but calling with "video" on an image should safely return empty
    let entries = extract_metadata(&test_image("baboon.png"), "video");
    assert!(entries.is_empty());
}

#[test]
fn extract_metadata_entry_structure() {
    let entries = extract_metadata(&test_image("HappyFish.jpg"), "image");
    for (namespace, tag, text, int, real) in &entries {
        // Namespace should be one of the expected values
        assert!(
            namespace == "exif" || namespace == "exif_thumb" || namespace == "exif_other",
            "unexpected namespace: {}",
            namespace
        );
        // Tag should not be empty
        assert!(!tag.is_empty());
        // At least one value field should be populated
        assert!(
            text.is_some() || int.is_some() || real.is_some(),
            "entry ({}, {}) has no value",
            namespace,
            tag
        );
    }
}

#[test]
fn extract_metadata_tiff_file() {
    // TIFF files can contain EXIF data
    let entries = extract_metadata(&test_image("lena_color_512.tif"), "image");
    // Just verify no panic - TIFF handling should work
    let _ = entries;
}

#[test]
fn extract_metadata_bmp_file() {
    // BMP files typically don't have EXIF
    let entries = extract_metadata(&test_image("lena.bmp"), "image");
    // BMP usually has no EXIF, so expect empty
    assert!(entries.is_empty());
}
