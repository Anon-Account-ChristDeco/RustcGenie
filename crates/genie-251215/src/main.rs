use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use clap::Parser;
use walkdir::WalkDir;

use genie_251215::code_structure;
use genie_251215::mutator::{Mutator, MutatorConfig, SnippetSource};
use genie_251215::seed_filter;
use genie_251215::snippet;

#[derive(Parser)]
#[command(about = "Structure-aware Rust code mutator for finding rustc ICEs")]
struct Cli {
    #[arg(short, long)]
    seeds_dir: String,

    #[arg(short, long)]
    ingredients_dir: String,

    #[arg(short, long)]
    output_dir: String,

    #[arg(short, long, default_value_t = 10)]
    mutations_per_seed: usize,

    #[arg(short, long, default_value_t = 2)]
    rounds: u32,

    /// Named ablation profile (overrides individual ablation flags)
    #[arg(long)]
    ablation_profile: Option<String>,

    /// Disable placeholder adaptation (for ablation study)
    #[arg(long, default_value_t = false)]
    no_placeholder_adaptation: bool,

    /// Disable dependency injection (for ablation study)
    #[arg(long, default_value_t = false)]
    no_dependency_injection: bool,
}

fn main() {
    let cli = Cli::parse();

    // Verify seeds directory
    let seeds_dir = Path::new(&cli.seeds_dir);
    if !seeds_dir.exists() || !seeds_dir.is_dir() {
        eprintln!("Error: Seeds directory does not exist or is not a directory.");
        std::process::exit(2);
    }

    // Verify ingredients directory
    let ingredients_dir = Path::new(&cli.ingredients_dir);
    if !ingredients_dir.exists() || !ingredients_dir.is_dir() {
        eprintln!("Error: Ingredients directory does not exist or is not a directory.");
        std::process::exit(2);
    }

    // Check output directory (create if needed)
    let output_dir = Path::new(&cli.output_dir);
    if !output_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(output_dir) {
            eprintln!("Error: Failed to create output directory: {e}");
            std::process::exit(3);
        }
    } else if output_dir
        .read_dir()
        .map(|mut i| i.next().is_some())
        .unwrap_or(false)
    {
        eprintln!("Error: Output directory is not empty.");
        std::process::exit(3);
    }

    // Canonicalize paths
    let seeds_dir = std::fs::canonicalize(seeds_dir).unwrap();
    let ingredients_dir = std::fs::canonicalize(ingredients_dir).unwrap();
    let output_dir = std::fs::canonicalize(output_dir).unwrap();

    // Collect seed files
    let files: Vec<PathBuf> = WalkDir::new(&seeds_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        .filter(|f| !format!("{:?}", f).contains("icemaker"))
        .filter(|f| !f.path().display().to_string().contains(".git"))
        .map(|f| f.path().to_owned())
        .filter(|pb| !seed_filter::ignore_file_for_splicing(pb))
        .collect();

    // Collect ingredient files
    let fragment_files: Vec<PathBuf> = WalkDir::new(&ingredients_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|f| f.path().extension() == Some(OsStr::new("json")))
        .map(|f| f.path().to_owned())
        .collect();

    eprintln!(
        "Found {} seed files and {} ingredient files",
        files.len(),
        fragment_files.len()
    );

    // Parse all seed files
    let seed_trees: HashMap<PathBuf, (Vec<u8>, tree_sitter::Tree)> = files
        .iter()
        .filter_map(|f| {
            let bytes = std::fs::read(f).ok()?;
            let tree = component_extractor::parse_code(&bytes);
            Some((f.clone(), (bytes, tree)))
        })
        .collect();

    // Extract code structures
    let seed_structures = code_structure::new_code_structures(&seed_trees);

    // Load annotated outputs
    let annotated_llm_outputs = snippet::load_annotated_outputs(&fragment_files);

    // Convert to fragments_set and filter
    let fragments_set: Vec<(PathBuf, Vec<llm_extractor::annotate::AnnotatedFragment>)> =
        annotated_llm_outputs
            .into_iter()
            .map(|(path, output)| (path, output.fragments))
            .collect();
    let fragments_set = snippet::filter_fragments(fragments_set);

    let fragments_hashmap: HashMap<PathBuf, Vec<llm_extractor::annotate::AnnotatedFragment>> =
        fragments_set.into_iter().collect();

    // Collect all snippets
    let (all_snippets, all_dependencies) = snippet::collect_all_snippets(
        &files,
        &seed_structures,
        &fragments_hashmap,
        &seeds_dir,
        &ingredients_dir,
    );

    // Build mutation config
    let config = match cli.ablation_profile.as_deref() {
        Some("source-only") => MutatorConfig {
            mutation_per_seed: cli.mutations_per_seed,
            enable_placeholder_adaptation: true,
            dep_injection_prob: 1.0,
            snippet_source: SnippetSource::SourceOnly,
            disable_misc_mutations: false,
        },
        Some("source-only-no-other") => MutatorConfig {
            mutation_per_seed: cli.mutations_per_seed,
            enable_placeholder_adaptation: false,
            dep_injection_prob: 0.0,
            snippet_source: SnippetSource::SourceOnly,
            disable_misc_mutations: false,
        },
        Some("source-only-no-other-no-misc") => MutatorConfig {
            mutation_per_seed: cli.mutations_per_seed,
            enable_placeholder_adaptation: false,
            dep_injection_prob: 0.0,
            snippet_source: SnippetSource::SourceOnly,
            disable_misc_mutations: true,
        },
        Some("no-dep-plc-misc") => MutatorConfig {
            mutation_per_seed: cli.mutations_per_seed,
            enable_placeholder_adaptation: false,
            dep_injection_prob: 0.0,
            snippet_source: SnippetSource::Both,
            disable_misc_mutations: true,
        },
        Some("half-dep-prob") => MutatorConfig {
            mutation_per_seed: cli.mutations_per_seed,
            enable_placeholder_adaptation: true,
            dep_injection_prob: 0.5,
            snippet_source: SnippetSource::Both,
            disable_misc_mutations: false,
        },
        Some(unknown) => {
            eprintln!("Error: unknown ablation profile '{unknown}'");
            std::process::exit(2);
        }
        None => MutatorConfig {
            mutation_per_seed: cli.mutations_per_seed,
            enable_placeholder_adaptation: !cli.no_placeholder_adaptation,
            dep_injection_prob: if cli.no_dependency_injection { 0.0 } else { 1.0 },
            snippet_source: SnippetSource::Both,
            disable_misc_mutations: false,
        },
    };

    // Log ablation configuration
    if let Some(profile) = &cli.ablation_profile {
        eprintln!("Ablation profile: {profile}");
    } else if cli.no_placeholder_adaptation || cli.no_dependency_injection {
        eprintln!(
            "Ablation mode: placeholder_adaptation={}, dependency_injection={}",
            !cli.no_placeholder_adaptation,
            !cli.no_dependency_injection
        );
    }

    let mutator = Mutator::new(
        output_dir,
        seed_trees,
        seed_structures,
        all_snippets,
        all_dependencies,
    );

    // Prepare RNG
    let mut rng = fastrand::Rng::new();

    for round in 0..cli.rounds {
        eprintln!("Round {} of mutation...", round + 1);
        mutator.mutate_n(&files, &config, &mut rng);
    }

    eprintln!("Done.");
}
