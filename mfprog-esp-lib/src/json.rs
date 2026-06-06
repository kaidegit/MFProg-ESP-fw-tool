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
            format!("解析 flasher_args.json 失败: {}", e),
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
            io::Error::new(io::ErrorKind::InvalidData, "JSON 中缺少 flash_files 字段")
        })?;

    let mut entries = Vec::new();
    for (addr, filename_val) in flash_files {
        let filename = filename_val.as_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "flash_files 的值必须是字符串")
        })?;
        let safe_path = validate_project_relative_path(filename)?;
        let file_path = canonical_base_dir
            .join(safe_path)
            .canonicalize()
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!("固件文件不存在或无法访问: {} ({})", filename, e),
                )
            })?;
        if !file_path.starts_with(&canonical_base_dir) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("固件路径解析后位于项目文件夹外: {}", filename),
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
            "flash_files 路径不能为空",
        ));
    }

    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("flash_files 路径必须位于项目文件夹内: {}", filename),
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
        if !output_dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("输出路径已存在且不是文件夹: {}", output_dir.display()),
            ));
        }
        if output_dir.read_dir()?.next().transpose()?.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("输出文件夹不是空文件夹: {}", output_dir.display()),
            ));
        }
    }

    let mut map_entries: BTreeMap<String, MapEntry> = BTreeMap::new();
    let mut planned_outputs: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    let mut flash_regions = Vec::new();

    for entry in entries {
        if !entry.file_path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("固件文件不存在: {}", entry.file_path.display()),
            ));
        }
        let addr = parse_flash_addr(&entry.addr)?;
        let size = entry.file_path.metadata()?.len();
        flash_regions.push(FlashRegion {
            addr,
            size,
            raw_addr: entry.addr.clone(),
            file_path: entry.file_path.clone(),
        });

        let filename = entry
            .file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let relative_path = filename.clone();

        let compressed_name = format!("{}.zl", relative_path);
        let output_path = output_dir.join(&compressed_name);
        if let Some(existing_input) =
            planned_outputs.insert(output_path.clone(), entry.file_path.clone())
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "多个输入文件会写入同一个输出文件: {} ({} 和 {})",
                    output_path.display(),
                    existing_input.display(),
                    entry.file_path.display()
                ),
            ));
        }
    }

    validate_non_overlapping_flash_regions(&mut flash_regions)?;

    let output_json_path = output_dir.join("flasher_args.json");
    if planned_outputs.contains_key(&output_json_path) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("压缩输出会与元数据文件冲突: {}", output_json_path.display()),
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
                format!("输出文件已存在: {}", output_path.display()),
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
        if let Some(Value::Object(flash_files)) = obj.get_mut("flash_files") {
            for (_, val) in flash_files.iter_mut() {
                if let Value::String(s) = val {
                    if let Some(name) = Path::new(s).file_name() {
                        *s = name.to_string_lossy().to_string();
                    }
                }
            }
        }
    }

    if output_json_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("输出文件已存在: {}", output_json_path.display()),
        ));
    }

    let output_str = serde_json::to_string_pretty(&output_json).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("序列化 flasher_args.json 失败: {}", e),
        )
    })?;
    fs::write(&output_json_path, output_str + "\n")?;

    Ok(())
}

#[derive(Debug, Clone)]
struct FlashRegion {
    addr: u64,
    size: u64,
    raw_addr: String,
    file_path: PathBuf,
}

fn parse_flash_addr(addr: &str) -> io::Result<u64> {
    let trimmed = addr.trim();
    if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        u64::from_str_radix(&trimmed[2..], 16)
    } else {
        trimmed.parse::<u64>()
    }
    .map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("flash 地址无效: {} ({})", addr, e),
        )
    })
}

fn validate_non_overlapping_flash_regions(regions: &mut [FlashRegion]) -> io::Result<()> {
    regions.sort_by_key(|region| region.addr);
    for window in regions.windows(2) {
        let prev = &window[0];
        let curr = &window[1];
        let prev_end = prev.addr.checked_add(prev.size).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "flash 区域地址溢出: {} @ {} (大小 {})",
                    prev.file_path.display(),
                    prev.raw_addr,
                    prev.size
                ),
            )
        })?;

        if prev_end > curr.addr {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "flash 文件区域重叠: {} @ {} (大小 {}) 与 {} @ {} (大小 {})",
                    prev.file_path.display(),
                    prev.raw_addr,
                    prev.size,
                    curr.file_path.display(),
                    curr.raw_addr,
                    curr.size
                ),
            ));
        }
    }
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
}
