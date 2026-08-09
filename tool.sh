#!/usr/bin/env bash
set -euo pipefail

echo "======================================================"
echo ">> Initiating Phase 1: Polyglot AST Engine Consolidation"
echo "======================================================"

# 1. Scaffold the new crate
mkdir -p apps/nexus_ast_tool/src

# 2. Write the Polyglot Cargo.toml
cat > apps/nexus_ast_tool/Cargo.toml << 'EOF'
[package]
name = "nexus_ast_tool"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = { workspace = true }
clap = { version = "4.4", features = ["derive"] }
syn = { version = "2", features = ["full", "visit-mut", "extra-traits"] }
quote = "1"
prettyplease = "0.2"
tree-sitter = "0.23"
tree-sitter-rust = "0.23"
tree-sitter-python = "0.23"
tree-sitter-typescript = "0.23"
tree-sitter-javascript = "0.23"
tree-sitter-zig = "*"
tree-sitter-svelte = "*"
EOF

# 3. Write the Router logic in src/main.rs
cat > apps/nexus_ast_tool/src/main.rs << 'EOF'
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "Nexus Polyglot AST Mutation Engine")]
struct Args {
    /// Target file to parse and mutate
    #[arg(short, long)]
    target: PathBuf,

    /// Exact symbol (function/class) to replace
    #[arg(short, long)]
    symbol: String,

    /// File containing the replacement payload
    #[arg(short, long)]
    patch_file: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    let ext = args.target.extension().and_then(|s| s.to_str()).unwrap_or("");
    
    println!(">> [AST ENGINE] Intercepted mutation request for {} (Symbol: {})", args.target.display(), args.symbol);

    match ext {
        "rs" => {
            println!(">> Routing to native Rust Syn/Quote engine...");
            // TODO: Port logic from apps/ast_patcher/src/main.rs here
        }
        "py" | "js" | "ts" | "zig" | "svelte" => {
            println!(">> Routing to Tree-sitter {} engine...", ext);
            // TODO: Port logic from apps/universal_patcher/src/main.rs here
        }
        _ => anyhow::bail!("Unsupported DNA format for AST engine: {}", ext),
    }

    Ok(())
}
EOF

# 4. Inject the new crate into the root workspace Cargo.toml
# This uses python to safely append the member if it doesn't exist
python3 -c '
import sys

cargo_toml = "Cargo.toml"
try:
    with open(cargo_toml, "r") as f:
        data = f.read()
except FileNotFoundError:
    print(">> [ERROR] Root Cargo.toml not found. Run this from the AthenaFusionX root.")
    sys.exit(1)

if "\"apps/nexus_ast_tool\"" not in data:
    # Find the members array and insert it safely
    data = data.replace(
        "members = [", 
        "members = [\n    \"apps/nexus_ast_tool\","
    )
    with open(cargo_toml, "w") as f:
        f.write(data)
    print(">> [WORKSPACE] Injected apps/nexus_ast_tool into root Cargo.toml members array.")
else:
    print(">> [WORKSPACE] apps/nexus_ast_tool is already in members array.")
'

echo "======================================================"
echo ">> [SUCCESS] Polyglot AST scaffold complete."
echo ">> Run 'cargo check -p nexus_ast_tool' to verify."
echo "======================================================"
