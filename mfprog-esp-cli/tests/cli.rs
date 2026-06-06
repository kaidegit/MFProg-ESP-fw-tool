use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDir {
    path: std::path::PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mfprog_esp_fw_tool_cli_{}_{}_{}",
            name,
            std::process::id(),
            unique
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn file(&self, relative: &str, contents: &[u8]) -> std::path::PathBuf {
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

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_MFProg-ESP-fw-tool"))
}

#[test]
fn folder_mode_writes_compressed_outputs() {
    let temp = TestDir::new("folder_mode");
    temp.file("input/build/app.bin", b"abc");
    temp.file(
        "input/flasher_args.json",
        br#"{
  "flash_files": {
    "0x10000": "build/app.bin"
  }
}
"#,
    );
    let output = temp.path.join("out");

    let output_result = cli()
        .arg("-i")
        .arg(temp.path.join("input"))
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(output_result.status.success());
    assert!(output.join("app.bin.zl").is_file());
    let output_json = fs::read_to_string(output.join("flasher_args.json")).unwrap();
    assert!(output_json.contains("\"map\""));
    assert!(output_json.contains("\"app.bin.zl\""));
}

#[test]
fn file_list_mode_writes_compressed_outputs() {
    let temp = TestDir::new("file_list_mode");
    let input = temp.file("app.bin", b"abc");
    let output = temp.path.join("out");
    let file_list = format!("[(0x10000, {})]", input.display());

    let output_result = cli()
        .arg("-i")
        .arg(file_list)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(output_result.status.success());
    assert!(output.join("app.bin.zl").is_file());
    assert!(output.join("flasher_args.json").is_file());
}

#[test]
fn invalid_input_returns_failure() {
    let temp = TestDir::new("invalid_input");

    let output_result = cli()
        .arg("-i")
        .arg("not-a-folder-or-file-list")
        .arg("-o")
        .arg(temp.path.join("out"))
        .output()
        .unwrap();

    assert!(!output_result.status.success());
}
