pub mod mutation;
pub mod agent;

use anyhow::Result;

pub fn run() -> Result<()> {
    println!("Project Nexus Singularity v0.2.2 - AST Micro-Agent Kernel active.");
    // Example initialization check for the RefactorAgent
    let _agent = agent::RefactorAgent::new("Cargo.toml");
    Ok(())
}
