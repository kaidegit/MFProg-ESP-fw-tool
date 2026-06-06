mod common;

use common::TestDir;
use mfprog_esp_lib::json::{
    compress_entries, compress_from_folder, extract_flash_entries_from_json,
};
use mfprog_esp_lib::FlashEntry;
use serde_json::{json, Value};
use std::fs;
use std::io;

#[test]
fn compress_entries_refuses_non_empty_output_dir_without_deleting_it() {
    let temp = TestDir::new("existing_output");
    let input = temp.file("bootloader.bin", b"boot");
    let output = temp.path.join("out");
    fs::create_dir_all(&output).unwrap();
    let sentinel = output.join("keep.txt");
    fs::write(&sentinel, b"do not delete").unwrap();

    let err = compress_entries(
        &[FlashEntry {
            addr: "0x0".to_string(),
            file_path: input,
        }],
        &output,
        None,
    )
    .unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&sentinel).unwrap(), b"do not delete");
}

#[test]
fn compress_entries_accepts_empty_existing_output_dir() {
    let temp = TestDir::new("empty_existing_output");
    let input = temp.file("bootloader.bin", b"boot");
    let output = temp.path.join("out");
    fs::create_dir_all(&output).unwrap();

    compress_entries(
        &[FlashEntry {
            addr: "0x0".to_string(),
            file_path: input,
        }],
        &output,
        None,
    )
    .unwrap();

    assert!(output.join("bootloader.bin.zl").is_file());
    assert!(output.join("flasher_args.json").is_file());
}

#[test]
fn extract_flash_entries_rejects_absolute_paths() {
    let temp = TestDir::new("absolute_paths");
    let json = json!({
        "flash_files": {
            "0x0": "/tmp/bootloader.bin"
        }
    });

    let err = extract_flash_entries_from_json(&json, &temp.path).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn extract_flash_entries_rejects_parent_dir_paths() {
    let temp = TestDir::new("parent_dir_paths");
    let json = json!({
        "flash_files": {
            "0x0": "../bootloader.bin"
        }
    });

    let err = extract_flash_entries_from_json(&json, &temp.path).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn extract_flash_entries_accepts_project_relative_paths() {
    let temp = TestDir::new("relative_paths");
    let bootloader = temp.file("bootloader.bin", b"boot").canonicalize().unwrap();
    let app = temp.file("build/app.bin", b"app").canonicalize().unwrap();
    let json = json!({
        "flash_files": {
            "0x0": "bootloader.bin",
            "0x10000": "build/app.bin"
        }
    });

    let entries = extract_flash_entries_from_json(&json, &temp.path).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|entry| entry.file_path == bootloader));
    assert!(entries.iter().any(|entry| entry.file_path == app));
}

#[cfg(unix)]
#[test]
fn extract_flash_entries_rejects_symlink_escape() {
    let temp = TestDir::new("symlink_escape");
    let outside = TestDir::new("symlink_escape_outside");
    let outside_file = outside.file("outside.bin", b"outside");
    std::os::unix::fs::symlink(&outside_file, temp.path.join("link.bin")).unwrap();
    let json = json!({
        "flash_files": {
            "0x0": "link.bin"
        }
    });

    let err = extract_flash_entries_from_json(&json, &temp.path).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn compress_entries_succeeds_for_new_output_dir() {
    let temp = TestDir::new("new_output");
    let input = temp.file("app.bin", b"firmware");
    let output = temp.path.join("out");

    compress_entries(
        &[FlashEntry {
            addr: "0x10000".to_string(),
            file_path: input,
        }],
        &output,
        None,
    )
    .unwrap();

    assert!(output.join("app.bin.zl").is_file());
    assert!(output.join("flasher_args.json").is_file());
}

#[test]
fn compress_entries_rejects_duplicate_output_names() {
    let temp = TestDir::new("duplicate_outputs");
    let input_a = temp.file("a/app.bin", b"a");
    let input_b = temp.file("b/app.bin", b"b");
    let output = temp.path.join("out");

    let err = compress_entries(
        &[
            FlashEntry {
                addr: "0x10000".to_string(),
                file_path: input_a,
            },
            FlashEntry {
                addr: "0x20000".to_string(),
                file_path: input_b,
            },
        ],
        &output,
        None,
    )
    .unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert!(!output.exists());
}

#[test]
fn compress_entries_flattens_flash_files_paths_when_using_input_json() {
    let temp = TestDir::new("flatten_flash_files");
    let input = temp.file("build/app.bin", b"app");
    let output = temp.path.join("out");
    let input_json = json!({
        "flash_files": {
            "0x10000": "build/app.bin"
        }
    });

    compress_entries(
        &[FlashEntry {
            addr: "0x10000".to_string(),
            file_path: input,
        }],
        &output,
        Some(&input_json),
    )
    .unwrap();

    let output_json_str = fs::read_to_string(output.join("flasher_args.json")).unwrap();
    let output_json: Value = serde_json::from_str(&output_json_str).unwrap();
    let flash_files = output_json
        .get("flash_files")
        .and_then(|v| v.as_object())
        .unwrap();
    assert_eq!(
        flash_files.get("0x10000").and_then(|v| v.as_str()),
        Some("app.bin")
    );
}

#[test]
fn compress_entries_writes_expected_json_metadata() {
    let temp = TestDir::new("json_metadata");
    let input = temp.file("build/app.bin", b"abc");
    let output = temp.path.join("out");
    let input_json = json!({
        "flash_files": {
            "0x10000": "build/app.bin"
        },
        "md5": "old-md5",
        "compressed": true,
        "chip": "esp32"
    });

    compress_entries(
        &[FlashEntry {
            addr: "0x10000".to_string(),
            file_path: input,
        }],
        &output,
        Some(&input_json),
    )
    .unwrap();

    let output_json_str = fs::read_to_string(output.join("flasher_args.json")).unwrap();
    let output_json: Value = serde_json::from_str(&output_json_str).unwrap();
    let obj = output_json.as_object().unwrap();
    assert!(!obj.contains_key("md5"));
    assert!(!obj.contains_key("compressed"));
    assert_eq!(obj.get("chip").and_then(|v| v.as_str()), Some("esp32"));

    let map_entry = output_json
        .get("map")
        .and_then(|v| v.as_object())
        .and_then(|map| map.get("app.bin"))
        .and_then(|v| v.as_object())
        .unwrap();
    assert_eq!(
        map_entry.get("file").and_then(|v| v.as_str()),
        Some("app.bin.zl")
    );
    assert_eq!(
        map_entry.get("format").and_then(|v| v.as_str()),
        Some("deflate")
    );
    assert_eq!(map_entry.get("raw_size").and_then(|v| v.as_u64()), Some(3));
    assert_eq!(
        map_entry.get("raw_md5").and_then(|v| v.as_str()),
        Some("900150983cd24fb0d6963f7d28e17f72")
    );
    assert!(
        map_entry
            .get("stored_size")
            .and_then(|v| v.as_u64())
            .unwrap()
            > 0
    );
}

#[test]
fn compress_entries_rejects_overlapping_flash_regions() {
    let temp = TestDir::new("overlapping_regions");
    let bootloader = temp.file("bootloader.bin", b"boot");
    let app = temp.file("app.bin", b"app");
    let output = temp.path.join("out");

    let err = compress_entries(
        &[
            FlashEntry {
                addr: "0x0".to_string(),
                file_path: bootloader,
            },
            FlashEntry {
                addr: "0x2".to_string(),
                file_path: app,
            },
        ],
        &output,
        None,
    )
    .unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    assert!(!output.exists());
}

#[test]
fn compress_entries_accepts_adjacent_flash_regions() {
    let temp = TestDir::new("adjacent_regions");
    let bootloader = temp.file("bootloader.bin", b"boot");
    let app = temp.file("app.bin", b"app");
    let output = temp.path.join("out");

    compress_entries(
        &[
            FlashEntry {
                addr: "0x0".to_string(),
                file_path: bootloader,
            },
            FlashEntry {
                addr: "0x4".to_string(),
                file_path: app,
            },
        ],
        &output,
        None,
    )
    .unwrap();

    assert!(output.join("bootloader.bin.zl").is_file());
    assert!(output.join("app.bin.zl").is_file());
}

#[test]
fn compress_entries_rejects_invalid_flash_addr() {
    let temp = TestDir::new("invalid_addr");
    let input = temp.file("app.bin", b"app");
    let output = temp.path.join("out");

    let err = compress_entries(
        &[FlashEntry {
            addr: "not-an-addr".to_string(),
            file_path: input,
        }],
        &output,
        None,
    )
    .unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    assert!(!output.exists());
}

#[test]
fn compress_from_fixture_folder_writes_all_esp32c3_at_outputs() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("esp32c3-at");
    let temp = TestDir::new("esp32c3_at_fixture");
    let output = temp.path.join("out");

    compress_from_folder(&fixture, &output).unwrap();

    for file in [
        "bootloader.bin.zl",
        "partition-table.bin.zl",
        "ota_data_initial.bin.zl",
        "at_customize.bin.zl",
        "mfg_nvs.bin.zl",
        "esp-at.bin.zl",
        "flasher_args.json",
    ] {
        assert!(output.join(file).is_file(), "missing output {file}");
    }

    let output_json_str = fs::read_to_string(output.join("flasher_args.json")).unwrap();
    let output_json: Value = serde_json::from_str(&output_json_str).unwrap();
    assert_eq!(
        output_json
            .get("extra_esptool_args")
            .and_then(|v| v.get("chip"))
            .and_then(|v| v.as_str()),
        Some("esp32c3")
    );
    assert_eq!(
        output_json
            .get("map")
            .and_then(|v| v.as_object())
            .map(|map| map.len()),
        Some(6)
    );
}
