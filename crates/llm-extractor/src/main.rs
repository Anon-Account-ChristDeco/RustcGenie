use std::fs;
use std::path::PathBuf;
use std::process;

use llm_extractor::annotate::annotate;
use llm_extractor::batch;
use llm_extractor::json_parse::parse_llm_json;
use llm_extractor::llm_client::call_llm;
use llm_extractor::prompt::build_prompt;

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  llm-extractor <rust-file> [--dry-run]");
    eprintln!("  llm-extractor --prepare <source-dir> <prompt-dir>");
    eprintln!("  llm-extractor --batch <source-dir> <json-dir> [<output-dir>]");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "--prepare" => {
            if args.len() < 4 {
                eprintln!("Usage: llm-extractor --prepare <source-dir> <prompt-dir>");
                process::exit(1);
            }
            let src_dir = PathBuf::from(&args[2]);
            let prompt_dir = PathBuf::from(&args[3]);
            batch::prepare_prompts(&src_dir, &prompt_dir);
        }
        "--batch" => {
            if args.len() < 4 {
                eprintln!("Usage: llm-extractor --batch <source-dir> <json-dir> [<output-dir>]");
                process::exit(1);
            }
            let src_dir = PathBuf::from(&args[2]);
            let json_dir = PathBuf::from(&args[3]);
            let output_dir = if args.len() >= 5 {
                PathBuf::from(&args[4])
            } else {
                json_dir.parent().unwrap_or(&json_dir).join("annotated")
            };
            batch::annotate_batch(&src_dir, &json_dir, &output_dir);
        }
        _ => {
            // Single file mode
            let rust_file = &args[1];
            let dry_run = args.iter().any(|a| a == "--dry-run");

            let source = match fs::read_to_string(rust_file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error reading {rust_file}: {e}");
                    process::exit(1);
                }
            };

            let prompt = build_prompt(&source);

            if dry_run {
                println!("{prompt}");
                return;
            }

            let raw_response = match call_llm(&prompt) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("LLM call failed: {e}");
                    process::exit(1);
                }
            };

            let llm_output = match parse_llm_json(&raw_response) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("Failed to parse LLM response: {e}");
                    eprintln!("Raw response:\n{raw_response}");
                    process::exit(1);
                }
            };

            let annotated = annotate(&source, &llm_output);

            match serde_json::to_string_pretty(&annotated) {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("Failed to serialize output: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
