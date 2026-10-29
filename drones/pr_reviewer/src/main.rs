use anyhow::Result;
use clap::Parser;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "Nexus PR Reviewer Substrate Drone")]
struct Args {
    #[arg(short, long)]
    target_dir: PathBuf,
}

#[derive(serde::Serialize)]
struct ReviewReport {
    status: String,
    risk_score: u8,
    violations: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!(">> [PR_REVIEWER_DRONE] Scanning target substrate at {}", args.target_dir.display());

    let mut violations = Vec::new();
    let mut risk_score = 0;

    // Scan files for substrate anti-patterns
    for entry in walkdir::WalkDir::new(&args.target_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().and_then(|s| s.to_str()) == Some("rs") {
            let content = fs::read_to_string(entry.path()).unwrap_or_default();

            if content.contains(".unwrap()") && !content.contains("unwrap_or") {
                violations.push(format!("Unwrapped panic vector in {:?}", entry.path()));
                risk_score += 10;
            }
            if content.contains("std::process::Command") && !content.contains("ActionGate") {
                violations
                    .push(format!("Unsanitized system command invocation in {:?}", entry.path()));
                risk_score += 40;
            }
        }
    }

    let report = ReviewReport {
        status: if risk_score > 30 { "REJECTED" } else { "APPROVED" }.to_string(),
        risk_score,
        violations,
    };

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
