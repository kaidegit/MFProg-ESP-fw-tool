use crate::CompressResult;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

pub fn compute_md5(filepath: &Path) -> io::Result<String> {
    let mut file = File::open(filepath)?;
    let mut hasher = md5::Context::new();
    let mut buffer = [0u8; 4096];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.consume(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.compute()))
}

pub fn pad_to(data: &mut Vec<u8>, alignment: usize) {
    let pad_mod = data.len() % alignment;
    if pad_mod != 0 {
        data.resize(data.len() + (alignment - pad_mod), 0xff);
    }
}

pub fn compress_file(input_path: &Path, output_path: &Path) -> io::Result<CompressResult> {
    let mut data = Vec::new();
    let mut file = File::open(input_path)?;
    file.read_to_end(&mut data)?;

    let raw_size = data.len();
    pad_to(&mut data, 4);

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(9));
    encoder.write_all(&data)?;
    let compressed = encoder.finish()?;

    let mut out_file = File::create(output_path)?;
    out_file.write_all(&compressed)?;

    let raw_md5 = compute_md5(input_path)?;

    Ok(CompressResult {
        raw_size,
        stored_size: compressed.len(),
        raw_md5,
    })
}
