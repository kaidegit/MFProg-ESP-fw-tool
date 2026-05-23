use crate::{compress::compress_file, FlashEntry};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

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
    let canonical_base_dir = base_dir.canonicalize()?;
    let flash_files = json
        .get("flash_files")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "No flash_files found in JSON")
        })?;

    let mut entries = Vec::new();
    for (addr, filename_val) in flash_files {
        let filename = filename_val.as_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "flash_files values must be strings",
            )
        })?;
        let safe_path = validate_project_relative_path(filename)?;
        let file_path = canonical_base_dir.join(safe_path).canonicalize()?;
        if !file_path.starts_with(&canonical_base_dir) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "flash_files path resolves outside project folder: {}",
                    filename
                ),
            ));
        }
        entries.push(FlashEntry {
            addr: addr.clone(),
            file_path,
        });
    }
    Ok(entries)
}

fn validate_project_relative_path(filename: &str) -> io::Result<&Path> {
    let path = Path::new(filename);
    if filename.trim().is_empty() || path.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "flash_files path must not be empty",
        ));
    }

    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "flash_files path must stay inside project folder: {}",
                        filename
                    ),
                ));
            }
        }
    }

    Ok(path)
}

pub fn compress_entries(
    entries: &[FlashEntry],
    output_dir: &Path,
    input_json: Option<&Value>,
) -> io::Result<()> {
    if output_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Output directory already exists: {}", output_dir.display()),
        ));
    }

    let mut map_entries: BTreeMap<String, MapEntry> = BTreeMap::new();
    let mut planned_outputs: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();

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
        if let Some(existing_input) =
            planned_outputs.insert(output_path.clone(), entry.file_path.clone())
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "Multiple input files would write the same output file: {} ({} and {})",
                    output_path.display(),
                    existing_input.display(),
                    entry.file_path.display()
                ),
            ));
        }
    }

    let output_json_path = output_dir.join("flasher_args.json");
    if planned_outputs.contains_key(&output_json_path) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "Compressed output would conflict with metadata file: {}",
                output_json_path.display()
            ),
        ));
    }

    fs::create_dir_all(output_dir)?;

    for entry in entries {
        let filename = entry
            .file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let relative_path = filename.clone();
        let compressed_name = format!("{}.zl", relative_path);
        let output_path = output_dir.join(&compressed_name);
        if output_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Output file already exists: {}", output_path.display()),
            ));
        }

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

    if output_json_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Output file already exists: {}", output_json_path.display()),
        ));
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "mfprog_esp_fw_tool_{}_{}_{}",
                name,
                std::process::id(),
                unique
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn file(&self, relative: &str, contents: &[u8]) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&path, contents).unwrap();
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn compress_entries_refuses_existing_output_dir_without_deleting_it() {
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
}
