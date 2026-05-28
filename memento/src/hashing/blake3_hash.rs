use std::fs::File;
use std::io::Read;

use super::HashResult;
use crate::error::Result;

const BUFFER_SIZE: usize = 1024 * 1024; // 1MB chunks

/// Hash the entire file (all bytes including metadata)
pub fn hash_file_full(path: &str) -> Result<HashResult> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; BUFFER_SIZE];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    Ok(HashResult::Hex(hash.to_hex().to_string()))
}

/// Hash only the image pixel data (decode image, discard metadata, hash raw pixels)
pub fn hash_file_content_only(path: &str) -> Result<HashResult> {
    let img = image::open(path).map_err(|e| crate::error::HashError::decode(path, e))?;

    // Convert to consistent format (RGBA8) and hash the raw pixel bytes
    let rgba = img.to_rgba8();
    let pixels = rgba.as_raw();

    let hash = blake3::hash(pixels);
    Ok(HashResult::Hex(hash.to_hex().to_string()))
}
