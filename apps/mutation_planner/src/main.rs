mod gatekeeper;
mod nexus_api;
mod telemetry;

use std::env;
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run -p mutation_planner -- \"<User Prompt>\"");
        std::process::exit(1);
    }

    let user_prompt = &args[1];
    let impact_path = "repository_analysis/impact.json";

    println!(">> Initiating Executive Cortex Gatekeeper (v1.9 Baseline)...");

    let report = telemetry::load_impact(impact_path)?;
    println!(">> Ingested telemetry for: {}", report.target_node);
    println!(">> Assessed Risk: {}", report.risk_level);

    let plan = gatekeeper::evaluate_risk(&report, user_prompt);

    let system_prompt = match &plan {
        gatekeeper::ActionPlan::AutoPatch(p) => {
            println!(">> [ROUTING] Risk is LOW. Authorizing Auto-Patch loop.");
            p.clone()
        }
        gatekeeper::ActionPlan::ReviewRequired(p) => {
            println!(">> [ROUTING] Risk is MEDIUM. Mandating LLM Side-Effect Review.");
            p.clone()
        }
        gatekeeper::ActionPlan::HumanApproval(p) => {
            println!(
                ">> [ROUTING] Risk is CRITICAL. Halting execution. Requesting EVOLUTION_REPORT.md."
            );
            p.clone()
        }
    };

    let response = nexus_api::dispatch_to_planner(system_prompt).await?;

    // Phase 8: Persistence Routing
    match plan {
        gatekeeper::ActionPlan::HumanApproval(_) => {
            let report_path = Path::new("repository_analysis/EVOLUTION_REPORT.md");
            fs::write(report_path, &response)?;
            println!(
                ">> [PERSISTENCE] Critical Evolution Report safely written to: {:?}",
                report_path
            );
        }
        gatekeeper::ActionPlan::AutoPatch(_) => {
            let patch_path = Path::new("repository_analysis/proposed_patch.diff");
            fs::write(patch_path, &response)?;
            println!(">> [PERSISTENCE] Low-risk patch proposal written to: {:?}", patch_path);
        }
        gatekeeper::ActionPlan::ReviewRequired(_) => {
            let review_path = Path::new("repository_analysis/review_notes.md");
            fs::write(review_path, &response)?;
            println!(">> [PERSISTENCE] Review notes written to: {:?}", review_path);
        }
    }

    Ok(())
}
