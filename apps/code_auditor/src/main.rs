use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::time::Instant;
use tracing::info;
use tracing_subscriber::FmtSubscriber;

mod agent;

/// Nexus Singularity Code Auditor
#[derive(Parser, Debug)]
#[command(author, version, about = "Autonomous Code Analysis via Nexus Cortex")]
struct Args {
    /// Target directory to audit
    #[arg(short, long, default_value = ".")]
    target: String,

    /// Output path for the JSON report
    #[arg(short, long, default_value = "audit_report.json")]
    output: String,

    /// Override the model (e.g., nvidia/llama-3.3-nemotron-super-49b-v1.5)
    #[arg(short, long, default_value = "gemini-3.5-flash-lite")]
    model: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize minimal logging
    let subscriber = FmtSubscriber::builder().compact().finish();
    tracing::subscriber::set_global_default(subscriber).ok();

    info!("Code Auditor v{}", env!("CARGO_PKG_VERSION"));
    let start_time = Instant::now();

    let target_path = PathBuf::from(&args.target);
    let model_name = args.model.clone();

    // Dynamic Provider Routing (Modular Customer Support)
    let (api_key, provider) =
        if model_name.starts_with("nvidia/") || model_name.starts_with("deepseek-ai/") {
            (std::env::var("NVIDIA_API_KEY").unwrap_or_default(), "NVIDIA NIM Cloud")
        } else {
            (std::env::var("GEMINI_API_KEY").unwrap_or_default(), "Google AI Studio")
        };

    if api_key.is_empty() {
        anyhow::bail!("Missing API key for provider '{}'. Please export it.", provider);
    }

    println!("🤖 Initializing Nexus Executive Cortex...");
    println!("   Active Provider: {}", provider);
    println!("   Active Model:    {}", model_name);
    println!("   Target Path:     {}", target_path.display());

    let client = reqwest::Client::new();
    let orchestrator = std::sync::Arc::new(nexus_runtime_core::orchestrator::Orchestrator::new());

    // Initialize the hollowed-out agent wrapper
    let mut agent =
        agent::CodeAnalysisAgent::new(orchestrator, client, api_key, target_path, model_name, 5);

    println!("\n🔬 Running modular code analysis via Nexus Substrate...");
    let reported_issues = agent.run_analysis().await?;

    println!("\n📝 Generating audit report...");
    let duration = start_time.elapsed().as_secs_f64();

    // Write output to JSON
    let json_output = serde_json::to_string_pretty(&reported_issues)
        .context("Failed to serialize issues to JSON")?;

    std::fs::write(&args.output, json_output)
        .with_context(|| format!("Failed to write report to {}", args.output))?;

    println!("\n📊 Analysis Summary:");
    println!("   Total issues found: {}", reported_issues.len());
    println!("   Duration: {:.1}s", duration);
    println!("   ✅ Report saved to: {}", args.output);

    Ok(())
}
