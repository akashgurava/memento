use img_hash::{HashAlg, HasherConfig};

use super::HashResult;
use crate::error::{HashError, Result};

/// Compute perceptual hash (DCT-based)
pub fn compute_phash(path: &str) -> Result<HashResult> {
    compute_perceptual(path, HashAlg::Mean)
}

/// Compute difference hash (gradient-based)
pub fn compute_dhash(path: &str) -> Result<HashResult> {
    compute_perceptual(path, HashAlg::Gradient)
}

/// Compute wavelet hash
pub fn compute_whash(path: &str) -> Result<HashResult> {
    compute_perceptual(path, HashAlg::DoubleGradient)
}

fn compute_perceptual(path: &str, alg: HashAlg) -> Result<HashResult> {
    let img = image::open(path).map_err(|e| HashError::decode(path, e))?;

    let hasher = HasherConfig::new()
        .hash_alg(alg)
        .hash_size(8, 8) // 64-bit hash
        .to_hasher();

    let hash = hasher.hash_image(&img);
    let bytes = hash.as_bytes();

    // Convert 8 bytes to i64
    let value = if bytes.len() >= 8 {
        i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ])
    } else {
        // Pad with zeros if hash is shorter
        let mut padded = [0u8; 8];
        padded[..bytes.len()].copy_from_slice(bytes);
        i64::from_le_bytes(padded)
    };

    Ok(HashResult::Perceptual(value))
}
