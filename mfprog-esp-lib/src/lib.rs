pub mod compress;
pub mod json;
pub mod parser;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FlashEntry {
    pub addr: String,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CompressResult {
    pub raw_size: usize,
    pub stored_size: usize,
    pub raw_md5: String,
}
