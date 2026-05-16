use crate::{compress::compress_file, FlashEntry};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Serialize, Debug, Clone)]
pub struct MapEntry {
    pub file: String,
    pub format: String,
    pub raw_size: usize,
    pub raw_md5: String,
    pub stored_size: usize,
}

pub fn read_flasher_args(dir: &Path) -> io::Result<Value> {
    let path = dir.join("flasher_args.json");
    let contents = fs::read_to_string(&path)?;
    serde_json::from_str(&contents).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to parse JSON: {}", e),
        )
    })
}

pub fn extract_flash_entries_from_json(
    json: &Value,
    base_dir: &Path,
) -> io::Result<Vec<FlashEntry>> {
    let flash_files = json
        .get("flash_files")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "No flash_files found in JSON")
        })?;

    let mut entries = Vec::new();
    for (addr, filename_val) in flash_files {
        let filename = filename_val
            .as_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "flash_files values must be strings"))?;
        entries.push(FlashEntry {
            addr: addr.clone(),
            file_path: base_dir.join(filename),
        });
    }
    Ok(entries)
}

pub fn compress_entries(
    entries: &[FlashEntry],
    output_dir: &Path,
    input_json: Option<&Value>,
) -> io::Result<()> {
    if output_dir.exists() {
        fs::remove_dir_all(output_dir)?;
    }
    fs::create_dir_all(output_dir)?;

    let mut map_entries: BTreeMap<String, MapEntry> = BTreeMap::new();

    for entry in entries {
        if !entry.file_path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", entry.file_path.display()),
            ));
        }

        let filename = entry
            .file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Preserve relative subdir structure if the file is inside a folder
        let relative_path = filename.clone();

        let compressed_name = format!("{}.zl", relative_path);
        let output_path = output_dir.join(&compressed_name);
        let result = compress_file(&entry.file_path, &output_path)?;

        map_entries.insert(
            relative_path.clone(),
            MapEntry {
                file: compressed_name,
                format: "deflate".to_string(),
                raw_size: result.raw_size,
                raw_md5: result.raw_md5,
                stored_size: result.stored_size,
            },
        );
    }

    let mut output_json = if let Some(base) = input_json {
        base.clone()
    } else {
        let mut obj = serde_json::Map::new();
        let mut flash_files = serde_json::Map::new();
        for entry in entries {
            let filename = entry
                .file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            flash_files.insert(entry.addr.clone(), Value::String(filename));
        }
        obj.insert("flash_files".to_string(), Value::Object(flash_files));
        Value::Object(obj)
    };

    if let Some(obj) = output_json.as_object_mut() {
        obj.remove("md5");
        obj.remove("compressed");
        obj.insert(
            "map".to_string(),
            serde_json::to_value(&map_entries).unwrap_or(Value::Null),
        );
    }

    let output_json_path = output_dir.join("flasher_args.json");
    let output_str = serde_json::to_string_pretty(&output_json).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to serialize JSON: {}", e),
        )
    })?;
    fs::write(&output_json_path, output_str + "\n")?;

    Ok(())
}

pub fn compress_from_folder(input_dir: &Path, output_dir: &Path) -> io::Result<()> {
    let json = read_flasher_args(input_dir)?;
    let entries = extract_flash_entries_from_json(&json, input_dir)?;
    compress_entries(&entries, output_dir, Some(&json))
}

pub fn default_output_dir(input_dir: &Path) -> PathBuf {
    let base_name = input_dir.file_name().unwrap_or_default();
    let parent = input_dir.parent().unwrap_or_else(|| Path::new(""));
    parent.join(format!("{}_compressed", base_name.to_string_lossy()))
}
