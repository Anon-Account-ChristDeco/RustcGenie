use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;

use component_extractor::{extract_from_dir, find_kind_family, parse_code, SnippetsWFile};
use serde_json;

fn extract_from_file(path: &Path) -> SnippetsWFile {
    let source = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("error: failed to read {}: {e}", path.display());
        process::exit(1);
    });
    let tree = parse_code(&source);
    let mut trees = HashMap::new();
    trees.insert(path.to_path_buf(), (source, tree));
    SnippetsWFile::new(trees)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: component-extractor <path> [--verbose] [--json]");
        eprintln!("  path: a .rs file or directory to extract from");
        process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
    let json = args.iter().any(|a| a == "--json");

    if !path.exists() {
        eprintln!("error: path does not exist: {}", path.display());
        process::exit(1);
    }

    let snippets = if path.is_dir() {
        extract_from_dir(&path)
    } else {
        extract_from_file(&path)
    };

    if json {
        let records = snippets.to_fragment_records();
        println!("{}", serde_json::to_string_pretty(&records).unwrap());
        return;
    }

    let mut kinds: Vec<_> = snippets.0.iter().map(|(&k, v)| (k, v)).collect::<Vec<(&str, &Vec<(PathBuf, Vec<u8>)>)>>();
    kinds.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    let total_snippets: usize = kinds.iter().map(|(_, v)| v.len()).sum();
    println!("{total_snippets} snippets across {} node kinds\n", kinds.len());

    for (kind, entries) in &kinds {
        let family = find_kind_family(kind);
        let family_str = match &family {
            component_extractor::KindFamilyFindResult::Unknown => String::new(),
            component_extractor::KindFamilyFindResult::Unique(f) => format!("  [{f:?}]"),
            component_extractor::KindFamilyFindResult::Ambiguous(fs) => {
                let names: Vec<String> = fs.iter().map(|f| format!("{f:?}")).collect();
                format!("  [{}]", names.join(", "))
            }
        };
        println!("  {kind}: {}{family_str}", entries.len());

        if verbose {
            for (file, text) in entries.iter().take(3) {
                let preview = String::from_utf8_lossy(text);
                let preview = if preview.len() > 80 {
                    format!("{}...", &preview[..80])
                } else {
                    preview.to_string()
                };
                let preview = preview.replace('\n', "\\n");
                println!("    {}: {preview}", file.display());
            }
            if entries.len() > 3 {
                println!("    ... and {} more", entries.len() - 3);
            }
        }
    }
}
