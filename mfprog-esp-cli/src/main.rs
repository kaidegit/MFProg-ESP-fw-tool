use clap::Parser;
use mfprog_esp_lib::json::{compress_entries, compress_from_folder, default_output_dir};
use mfprog_esp_lib::parser::parse_file_list;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "MFProg-ESP-fw-tool",
    about = "ESP firmware compression tool (CLI)"
)]
struct Args {
    /// Input: folder path containing flasher_args.json, or file list [(addr, file), ...]
    #[arg(short = 'i', long = "input", required = true)]
    input: String,

    /// Output folder path
    #[arg(short = 'o', long = "output")]
    output: Option<String>,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let input_str = args.input.trim();
    let output_dir = match args.output {
        Some(o) => PathBuf::from(&o),
        None => {
            // If input is a folder, use default; otherwise use ./compressed_output
            let p = Path::new(input_str);
            if p.is_dir() {
                default_output_dir(p)
            } else {
                PathBuf::from("compressed_output")
            }
        }
    };

    let input_path = Path::new(input_str);

    if input_path.is_dir() {
        let canonical_input = input_path.canonicalize()?;
        println!("Folder mode");
        println!("  input : {}", canonical_input.display());
        println!("  output: {}", output_dir.display());
        compress_from_folder(&canonical_input, &output_dir)?;
        println!("Done. Output written to {}", output_dir.display());
    } else if input_str.starts_with('[') {
        println!("File list mode");
        let entries = parse_file_list(input_str)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        println!("  entries: {}", entries.len());
        println!("  output : {}", output_dir.display());
        compress_entries(&entries, &output_dir, None)?;
        println!("Done. Output written to {}", output_dir.display());
    } else {
        eprintln!("Error: input is neither a valid folder nor a file list starting with '['");
        std::process::exit(1);
    }

    Ok(())
}
