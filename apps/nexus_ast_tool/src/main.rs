use clap::Parser;
use nexus_ast_tool::agent::RefactorAgent;
use std::process;

/// Project Nexus Singularity - Micro-Agent AST Orchestrator
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the target Rust source file
    #[arg(short, long)]
    file: String,

    /// Mutation Strategy (e.g., ReplaceFunctionBody, RenameSymbol, MakePublic, InsertEarlyReturn)
    #[arg(short, long)]
    strategy: String,

    /// The target symbol/function name to mutate
    #[arg(long)]
    symbol: String,

    /// The payload/code to inject
    #[arg(short, long)]
    payload: String,
}

fn main() {
    let cli = Cli::parse();

    println!("==> [CLI Wrapper] Waking RefactorAgent for {}...", cli.file);
    
    let agent = RefactorAgent::new(&cli.file);
    
    match agent.apply_refactoring(&cli.strategy, &cli.symbol, &cli.payload) {
        Ok(_) => {
            println!("==> [CLI Wrapper] Refactoring completed successfully.");
            process::exit(0);
        }
        Err(e) => {
            eprintln!("==> [CLI Wrapper] CRITICAL FAILURE: {}", e);
            // Non-zero exit code ensures AI agents know the mutation failed syntax validation
            process::exit(1); 
        }
    }
}
