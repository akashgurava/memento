pub mod blake3_hash;
pub mod perceptual;

use crate::error::{HashError, Result};

#[derive(Debug, Clone)]
pub enum HashAlgo {
    Blake3Full,
    ContentBlake3,
    PHash,
    DHash,
    WHash,
}

#[derive(Debug, Clone)]
pub enum HashResult {
    Hex(String),
    Perceptual(i64),
}

impl HashAlgo {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "blake3" => Ok(HashAlgo::Blake3Full),
            "content_blake3" => Ok(HashAlgo::ContentBlake3),
            "phash" => Ok(HashAlgo::PHash),
            "dhash" => Ok(HashAlgo::DHash),
            "whash" => Ok(HashAlgo::WHash),
            _ => Err(HashError::unknown_algorithm(s)),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HashAlgo::Blake3Full => "blake3",
            HashAlgo::ContentBlake3 => "content_blake3",
            HashAlgo::PHash => "phash",
            HashAlgo::DHash => "dhash",
            HashAlgo::WHash => "whash",
        }
    }
}

/// Compute a hash for a file using the specified algorithm
pub fn compute_hash(algo: &HashAlgo, path: &str) -> Result<HashResult> {
    match algo {
        HashAlgo::Blake3Full => blake3_hash::hash_file_full(path),
        HashAlgo::ContentBlake3 => blake3_hash::hash_file_content_only(path),
        HashAlgo::PHash => perceptual::compute_phash(path),
        HashAlgo::DHash => perceptual::compute_dhash(path),
        HashAlgo::WHash => perceptual::compute_whash(path),
    }
}
