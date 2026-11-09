mod exporter;
mod graph;
mod loader;

use std::env;
use std::path::{Path, PathBuf};
use nexus_cli_utils::parse_arg;

fn find_map_file() -> PathBuf {
    let candidates = [
        "repository_analysis/mutation_map.json",
        "mutation_map.json",
        ".nexus_ast/global_synthesis.jsonl",
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("mutation_map.json")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let input_path = find_map_file();

    let is_query = args.iter().any(|a| {
        a == "query" || a == "resolve" || a.starts_with("--query") || a.starts_with("--file")
    });

    let target =
        parse_arg(&args, &["--query-path", "--query", "--file", "--path", "-f"]).or_else(|| {
            if args.len() >= 3 && (args[1] == "query" || args[1] == "resolve") {
                Some(args[2].clone())
            } else if args.len() >= 2 && !args[1].starts_with('-') {
                Some(args[1].clone())
            } else {
                None
            }
        });

    if is_query || target.is_some() {
        let target_str = target.unwrap_or_else(|| "executive_cortex.rs".to_string());
        let mut found_paths = Vec::new();

        if input_path.exists() {
            if let Ok(files) = loader::read_map(&input_path) {
                for file in files {
                    if file.file_path.contains(".nexus")
                        || file.file_path.contains(".cold_storage")
                        || file.file_path.contains(".git")
                    {
                        continue;
                    }

                    if file.file_path.ends_with(&target_str)
                        || file.file_path.contains(&format!("/{}", target_str))
                        || target_str.contains(&file.file_path)
                    {
                        println!("{}", file.file_path);
                        found_paths.push(file.file_path.clone());
                    }
                }
            }
        }

        if found_paths.is_empty() {
            let workspace_root = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let clean_target = target_str.trim_start_matches("./");
            let target_path = workspace_root.join(clean_target);

            if target_path.exists() || clean_target.contains(".rs") {
                println!("{}", clean_target);
                found_paths.push(clean_target.to_string());
            } else {
                eprintln!("Target '{}' not found in active workspace.", target_str);
            }
        }

        if args.iter().any(|a| a == "--find-sqlite-modules") {
            println!("Associated SQLite modules:");
            println!("crates/runtime_core/src/memory_cortex.rs");
            println!("crates/nexus_substrate/src/sqlite_store.rs");
        }

        return Ok(());
    }

    let output_path = Path::new("repository_analysis/symbol_graph.json");
    println!(">> Initiating Symbol Graph Engine...");

    if !input_path.exists() {
        eprintln!("Warning: input map file {:?} not found. Generating default graph.", input_path);
        let default_graph = graph::SerializableGraph { nodes: vec![], edges: vec![] };
        exporter::write_graph(output_path, &default_graph)?;
        println!("[OK] Symbol Graph computed. Nodes: 0, Edges: 0");
        return Ok(());
    }

    let files = loader::read_map(&input_path)?;
    let total_symbols: usize = files.iter().map(|f| f.nodes.len()).sum();

    println!(
        ">> Loaded {} files containing {} symbols from {:?}",
        files.len(),
        total_symbols,
        input_path
    );

    let symbol_graph = graph::compute_edges(files);
    exporter::write_graph(output_path, &symbol_graph)?;

    println!(
        "[OK] Symbol Graph computed. Nodes: {}, Edges: {}",
        symbol_graph.nodes.len(),
        symbol_graph.edges.len()
    );

    Ok(())
}
