use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use tree_sitter::Parser as TsParser;

#[derive(Parser, Debug)]
#[command(author, version, about = "Universal AST Patcher via Tree-sitter")]
struct Args {
    /// Target file to parse
    #[arg(short, long)]
    target: PathBuf,

    /// Language of the target file (rust, python, ts, js, zig, svelte)
    #[arg(short, long)]
    language: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut parser = TsParser::new();

    // Tree-sitter 0.23 exposes language definitions as constants (LANGUAGE / LANGUAGE_TYPESCRIPT)
    let language = match args.language.as_str() {
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "python" => tree_sitter_python::LANGUAGE.into(),
        "ts" | "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "js" | "javascript" => tree_sitter_javascript::LANGUAGE.into(),
        "zig" => tree_sitter_zig::LANGUAGE.into(),
        _ => anyhow::bail!("Unsupported language: {}. Supported: rust, python, ts, js, zig", args.language),
    };

    parser.set_language(&language).context("Error loading Tree-sitter grammar")?;

    let source_code = fs::read_to_string(&args.target)
        .with_context(|| format!("Failed to read target file: {:?}", args.target))?;

    let tree = parser.parse(&source_code, None)
        .context("Failed to parse source code")?;

    let root_node = tree.root_node();
    
    println!("✅ Successfully parsed {} as {}", args.target.display(), args.language);
    println!("🌲 AST Root Node Type: {:?}", root_node.kind());
    println!("📊 Total child nodes at root: {}", root_node.child_count());
    println!(">> Universal AST loaded. Ready for surgical replacements...");

    Ok(())
}
