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

    for cap in re.captures_iter(trimmed) {
        let addr = cap[1].to_string();
        let file = cap[2].trim().to_string();
        if file.is_empty() {
            return Err("Empty file path in entry".to_string());
        }
        entries.push(FlashEntry {
            addr,
            file_path: PathBuf::from(file),
        });
    }

    if entries.is_empty() {
        return Err("No valid entries found".to_string());
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let input = "[(0x0, bootloader.bin), (0x10000, app.bin)]";
        let entries = parse_file_list(input).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].addr, "0x0");
        assert_eq!(entries[0].file_path, PathBuf::from("bootloader.bin"));
        assert_eq!(entries[1].addr, "0x10000");
        assert_eq!(entries[1].file_path, PathBuf::from("app.bin"));
    }

    #[test]
    fn test_parse_decimal_addr() {
        let input = "[(0, a.bin), (4096, b.bin)]";
        let entries = parse_file_list(input).unwrap();
        assert_eq!(entries[0].addr, "0");
        assert_eq!(entries[1].addr, "4096");
    }
}
