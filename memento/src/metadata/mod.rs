pub mod image;
pub mod video;

/// A metadata entry: (namespace, tag, value_text, value_int, value_real)
pub type MetadataEntry = (String, String, Option<String>, Option<i64>, Option<f64>);

/// Extract all metadata from a file based on its type
pub fn extract_metadata(path: &str, file_type: &str) -> Vec<MetadataEntry> {
    match file_type {
        "image" => image::extract_image_metadata(path),
        "video" => video::extract_video_metadata(path),
        _ => Vec::new(),
    }
}
