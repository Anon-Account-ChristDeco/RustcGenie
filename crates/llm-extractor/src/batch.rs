use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::annotate::annotate;
use crate::json_parse::parse_llm_json;
use crate::prompt::build_prompt;

/// Walk `src_dir` for `.rs` files, wrap each with the prompt template,
/// and write the result to `prompt_dir` preserving the directory structure.
///
/// Output: `prompt_dir/<relative_path>/<stem>.txt`
///
/// These prompt files can then be fed to `batch_processor.py`.
pub fn prepare_prompts(src_dir: &Path, prompt_dir: &Path) {
    let files: Vec<PathBuf> = WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|e| e.into_path())
        .collect();

    eprintln!("Found {} .rs files in {}", files.len(), src_dir.display());

    for file in &files {
        let relative = file.strip_prefix(src_dir).unwrap_or(file);
        let stem = file.file_stem().unwrap_or_default().to_string_lossy();
        let parent = relative.parent().unwrap_or(Path::new(""));

        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[SKIP] {}: {e}", relative.display());
                continue;
            }
        };

        if source.trim().is_empty() {
            eprintln!("[EMPTY] {}", relative.display());
            continue;
        }

        let prompt = build_prompt(&source);
        let out_path = prompt_dir.join(parent).join(format!("{stem}.txt"));
        if let Some(p) = out_path.parent() {
            fs::create_dir_all(p).ok();
        }
        match fs::write(&out_path, &prompt) {
            Ok(_) => eprintln!("[OK] {}", relative.display()),
            Err(e) => eprintln!("[ERROR] writing {}: {e}", out_path.display()),
        }
    }
}

/// Read collected LLM JSON responses from `json_dir`, match each to its
/// original `.rs` source file in `src_dir`, annotate with node kinds,
/// and write refined JSON to `output_dir`.
///
/// Expects the directory layout produced by `batch_processor.py`:
///   `json_dir/<relative_path>/<stem>.json`
/// matched against:
///   `src_dir/<relative_path>/<stem>.rs`
pub fn annotate_batch(src_dir: &Path, json_dir: &Path, output_dir: &Path) {
    let json_files: Vec<PathBuf> = WalkDir::new(json_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .map(|e| e.into_path())
        .collect();

    eprintln!(
        "Found {} JSON files in {}",
        json_files.len(),
        json_dir.display()
    );

    fs::create_dir_all(output_dir).ok();

    let mut processed = 0usize;
    let mut errors = 0usize;
    let total = json_files.len();

    for json_file in &json_files {
        let relative = json_file.strip_prefix(json_dir).unwrap_or(json_file);
        let stem = json_file
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let parent = relative.parent().unwrap_or(Path::new(""));

        // Find matching source file
        let src_path = src_dir.join(parent).join(format!("{stem}.rs"));
        let source = match fs::read_to_string(&src_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ERROR] {}: source not found at {}: {e}", relative.display(), src_path.display());
                errors += 1;
                continue;
            }
        };

        // Read raw LLM JSON
        let raw_json = match fs::read_to_string(json_file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ERROR] {}: {e}", relative.display());
                errors += 1;
                continue;
            }
        };

        // Parse
        let llm_output = match parse_llm_json(&raw_json) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("[ERROR] {}: parse failed: {e}", relative.display());
                errors += 1;
                continue;
            }
        };

        // Annotate
        let annotated = annotate(&source, &llm_output);

        // Write
        let out_path = output_dir.join(parent).join(format!("{stem}.json"));
        if let Some(p) = out_path.parent() {
            fs::create_dir_all(p).ok();
        }

        match serde_json::to_string_pretty(&annotated) {
            Ok(json) => {
                fs::write(&out_path, json).ok();
                processed += 1;
                eprintln!("[{processed}/{total}] {}", relative.display());
            }
            Err(e) => {
                eprintln!("[ERROR] {}: serialize: {e}", relative.display());
                errors += 1;
            }
        }
    }

    eprintln!();
    eprintln!("Done: {processed} processed, {errors} errors out of {total} files");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_prompts_creates_files() {
        let tmp = std::env::temp_dir().join("llm-ext-test-prepare");
        let src = tmp.join("src");
        let prompts = tmp.join("prompts");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&src).unwrap();

        fs::write(src.join("hello.rs"), "fn main() {}").unwrap();

        prepare_prompts(&src, &prompts);

        let prompt_file = prompts.join("hello.txt");
        assert!(prompt_file.exists());
        let content = fs::read_to_string(&prompt_file).unwrap();
        assert!(content.contains("fn main() {}"));
        assert!(content.contains("Input Rust Code"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn annotate_batch_roundtrip() {
        let tmp = std::env::temp_dir().join("llm-ext-test-batch");
        let src = tmp.join("src");
        let json = tmp.join("json");
        let out = tmp.join("out");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&json).unwrap();

        // Source file
        fs::write(src.join("example.rs"), "use std::fmt;\nfn main() {}").unwrap();

        // Simulated LLM JSON response
        let llm_json = r#"{
            "intro-structures": ["use std::fmt;"],
            "fragments": [
                {
                    "fragment": "fmt",
                    "criteria": "1",
                    "dependencies": ["use std::fmt;"],
                    "placeholders": []
                }
            ]
        }"#;
        fs::write(json.join("example.json"), llm_json).unwrap();

        annotate_batch(&src, &json, &out);

        let result_path = out.join("example.json");
        assert!(result_path.exists());
        let result: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&result_path).unwrap()).unwrap();
        // Should have node_kind annotated
        assert!(result["fragments"][0]["node_kind"].is_string());

        let _ = fs::remove_dir_all(&tmp);
    }
}
