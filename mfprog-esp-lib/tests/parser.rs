use mfprog_esp_lib::parser::parse_file_list;
use std::path::PathBuf;

#[test]
fn parses_tuple_file_list() {
    let input = "[(0x0, bootloader.bin), (0x10000, app.bin)]";
    let entries = parse_file_list(input).unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].addr, "0x0");
    assert_eq!(entries[0].file_path, PathBuf::from("bootloader.bin"));
    assert_eq!(entries[1].addr, "0x10000");
    assert_eq!(entries[1].file_path, PathBuf::from("app.bin"));
}

#[test]
fn parses_decimal_addresses() {
    let input = "[(0, a.bin), (4096, b.bin)]";
    let entries = parse_file_list(input).unwrap();

    assert_eq!(entries[0].addr, "0");
    assert_eq!(entries[1].addr, "4096");
}

#[test]
fn rejects_missing_list_brackets() {
    let err = parse_file_list("(0x0, bootloader.bin)").unwrap_err();

    assert_eq!(err, "Input must be a list enclosed in square brackets");
}

#[test]
fn rejects_empty_list() {
    let err = parse_file_list("[]").unwrap_err();

    assert_eq!(err, "No valid entries found");
}

#[test]
fn rejects_invalid_addr() {
    let err = parse_file_list("[(xyz, app.bin)]").unwrap_err();

    assert_eq!(err, "Invalid file list syntax");
}

#[test]
fn rejects_garbage_between_valid_entries() {
    let err = parse_file_list("[(0x0, bootloader.bin), garbage, (0x10000, app.bin)]").unwrap_err();

    assert_eq!(err, "Invalid file list syntax");
}

#[test]
fn rejects_trailing_garbage_after_valid_entry() {
    let err = parse_file_list("[(0x0, bootloader.bin) garbage]").unwrap_err();

    assert_eq!(err, "Invalid file list syntax");
}
