use crate::FlashEntry;
use regex::Regex;
use std::path::PathBuf;

/// Parse a string like [(0x0, bootloader.bin), (0x10000, app.bin)]
pub fn parse_file_list(input: &str) -> Result<Vec<FlashEntry>, String> {
    let trimmed = input.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err("Input must be a list enclosed in square brackets".to_string());
    }

    let re = Regex::new(r"\(\s*(0x[0-9a-fA-F]+|\d+)\s*,\s*([^)]+)\s*\)").unwrap();
    let mut entries = Vec::new();
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut cursor = 0;
    let mut first = true;

    for cap in re.captures_iter(inner) {
        let matched = cap.get(0).unwrap();
        let separator = inner[cursor..matched.start()].trim();
        if (first && !separator.is_empty()) || (!first && separator != ",") {
            return Err("Invalid file list syntax".to_string());
        }

        let addr = cap[1].to_string();
        let file = cap[2].trim().to_string();
        if file.is_empty() {
            return Err("Empty file path in entry".to_string());
        }
        entries.push(FlashEntry {
            addr,
            file_path: PathBuf::from(file),
        });
        cursor = matched.end();
        first = false;
    }

    if !inner[cursor..].trim().is_empty() {
        return Err("Invalid file list syntax".to_string());
    }

    if entries.is_empty() {
        return Err("No valid entries found".to_string());
    }

    Ok(entries)
}
