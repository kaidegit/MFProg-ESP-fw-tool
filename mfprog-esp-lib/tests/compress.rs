mod common;

use common::TestDir;
use flate2::read::ZlibDecoder;
use mfprog_esp_lib::compress::{compress_file, compute_md5, pad_to};
use std::fs;
use std::io::Read;

#[test]
fn pad_to_uses_ff_padding_until_alignment() {
    let cases = [
        (Vec::new(), Vec::new()),
        (vec![0x01], vec![0x01, 0xff, 0xff, 0xff]),
        (vec![0x01, 0x02, 0x03], vec![0x01, 0x02, 0x03, 0xff]),
        (vec![0x01, 0x02, 0x03, 0x04], vec![0x01, 0x02, 0x03, 0x04]),
        (
            vec![0x01, 0x02, 0x03, 0x04, 0x05],
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0xff, 0xff, 0xff],
        ),
    ];

    for (mut input, expected) in cases {
        pad_to(&mut input, 4);
        assert_eq!(input, expected);
    }
}

#[test]
fn compute_md5_returns_original_file_digest() {
    let temp = TestDir::new("md5");
    let input = temp.file("input.bin", b"abc");

    let digest = compute_md5(&input).unwrap();

    assert_eq!(digest, "900150983cd24fb0d6963f7d28e17f72");
}

#[test]
fn compress_file_writes_zlib_data_with_padding_and_original_metadata() {
    let temp = TestDir::new("compress_file");
    let input = temp.file("input.bin", b"abc");
    let output = temp.path.join("nested").join("input.bin.zl");

    let result = compress_file(&input, &output).unwrap();

    assert_eq!(result.raw_size, 3);
    assert_eq!(result.raw_md5, "900150983cd24fb0d6963f7d28e17f72");
    assert_eq!(
        result.stored_size,
        fs::metadata(&output).unwrap().len() as usize
    );

    let compressed = fs::read(output).unwrap();
    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).unwrap();
    assert_eq!(decompressed, b"abc\xff");
}
