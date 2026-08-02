use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApprovalDecision {
    ApprovedForStaging,
    RequiresHumanApproval,
    RejectedRiskThresholdExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateMutation {
    pub id: String,
    pub target_symbol: String,
    pub lines_changed: usize,
    pub test_coverage_pct: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedMutation {
    pub candidate_id: String,
    pub target_symbol: String,
    pub fitness_score: f32,
    pub decision: ApprovalDecision,
    pub rationale: String,
    /// True only if BOTH impact and health data came from real, validated
    /// reports, not from fail-conservative defaults or unverified input.
    pub verified_inputs: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImpactInput {
    pub risk_classification: String,
    pub blast_radius_score: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionHealthInput {
    pub execution_health: String,
    #[serde(default)]
    pub input_validated: bool,
}

fn rank_mutation(
    candidate: &CandidateMutation,
    impact: &ImpactInput,
    health: &ExecutionHealthInput,
    verified_inputs: bool,
) -> RankedMutation {
    let mut fitness = 1.0f32;
    let mut rationale = Vec::new();

    match impact.risk_classification.as_str() {
        "CRITICAL" => {
            fitness -= 0.40;
            rationale.push("Target is critical core architecture (-0.40)".to_string());
        }
        "HIGH" => {
            fitness -= 0.25;
            rationale.push("High blast radius (-0.25)".to_string());
        }
        "MODERATE" => {
            fitness -= 0.10;
            rationale.push("Moderate blast radius (-0.10)".to_string());
        }
        _ => {}
    }

    match health.execution_health.as_str() {
        "CRITICAL" => {
            fitness -= 0.50;
            rationale.push("Substrate execution health is CRITICAL (-0.50)".to_string());
        }
        "DEGRADED" => {
            fitness -= 0.20;
            rationale.push("Substrate execution health is DEGRADED (-0.20)".to_string());
        }
        "BRITTLE" => {
            fitness -= 0.10;
            rationale.push("Substrate execution health is BRITTLE (-0.10)".to_string());
        }
        "UNVERIFIABLE" => {
            fitness -= 0.20;
            rationale.push("Substrate execution health could not be verified from the supplied trace (-0.20)".to_string());
        }
        _ => {}
    }

    if candidate.test_coverage_pct > 80.0 {
        fitness += 0.15;
        rationale.push("High test coverage (+0.15)".to_string());
    } else if candidate.test_coverage_pct < 30.0 {
        fitness -= 0.15;
        rationale.push("Low test coverage (-0.15)".to_string());
    }

    if !verified_inputs {
        rationale.push(
            "WARNING: impact/health inputs were not supplied, failed to parse, or were not \
             recognized as validated trace data — fell back to conservative assumptions, \
             not measured data."
                .to_string(),
        );
    }

    let final_fitness = fitness.clamp(0.0, 1.0);

    let decision = if !verified_inputs {
        ApprovalDecision::RequiresHumanApproval
    } else if final_fitness >= 0.75 && impact.risk_classification != "CRITICAL" {
        ApprovalDecision::ApprovedForStaging
    } else if final_fitness >= 0.40 || impact.risk_classification == "CRITICAL" {
        ApprovalDecision::RequiresHumanApproval
    } else {
        ApprovalDecision::RejectedRiskThresholdExceeded
    };

    RankedMutation {
        candidate_id: candidate.id.clone(),
        target_symbol: candidate.target_symbol.clone(),
        fitness_score: (final_fitness * 100.0).round() / 100.0,
        decision,
        rationale: rationale.join("; "),
        verified_inputs,
    }
}

fn parse_arg(args: &[String], flags: &[&str]) -> Option<String> {
    for flag in flags {
        if let Some(i) = args.iter().position(|r| r == flag) {
            if let Some(val) = args.get(i + 1) {
                if !val.starts_with('-') {
                    return Some(val.clone());
                }
            }
        }
    }
    None
}

fn parse_positional(args: &[String]) -> Option<String> {
    let flags_with_values = [
        "--candidate",
        "--candidate-file",
        "--target",
        "--impact-report",
        "--impact",
        "--impact-file",
        "--health-report",
        "--health",
        "--health-file",
        "--output",
        "-o",
    ];
    let mut skip_next = false;
    for arg in args.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if flags_with_values.contains(&arg.as_str()) {
            skip_next = true;
            continue;
        }
        if !arg.starts_with('-') && !arg.ends_with(".json") {
            return Some(arg.clone());
        }
    }
    None
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    // 1. Resolve candidate / target symbol — hard error if nothing resolves.
    let candidate_path_str = parse_arg(&args, &["--candidate", "--candidate-file"]);
    let target_flag = parse_arg(&args, &["--target"]);
    let positional_target = parse_positional(&args);

    let candidate: CandidateMutation = if let Some(path_str) = candidate_path_str {
        let path = PathBuf::from(&path_str);
        if !path.exists() {
            eprintln!("ERROR: --candidate path does not exist: {}", path_str);
            std::process::exit(1);
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read candidate file {:?}", path))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse candidate JSON from {:?}", path))?
    } else if let Some(target) = target_flag.or(positional_target) {
        CandidateMutation {
            id: "mut-001".to_string(),
            target_symbol: target,
            lines_changed: 12,
            test_coverage_pct: 85.0,
        }
    } else {
        eprintln!(
            "ERROR: No candidate resolved. Pass --candidate <path-to-json>, --target <symbol>, or a positional target argument."
        );
        std::process::exit(1);
    };

    // 2. Resolve impact report — conservative default (CRITICAL) if absent/unparseable.
    let impact_path_str = parse_arg(&args, &["--impact-report", "--impact", "--impact-file"]);
    let (impact, impact_verified): (ImpactInput, bool) = match impact_path_str {
        Some(path_str) => {
            let path = PathBuf::from(&path_str);
            match fs::read_to_string(&path).ok().and_then(|c| serde_json::from_str::<ImpactInput>(&c).ok()) {
                Some(parsed) => (parsed, true),
                None => {
                    eprintln!("WARNING: --impact-report at {} missing or unparseable; falling back to CRITICAL.", path_str);
                    (ImpactInput { risk_classification: "CRITICAL".to_string(), blast_radius_score: 1.0 }, false)
                }
            }
        }
        None => (
            ImpactInput { risk_classification: "CRITICAL".to_string(), blast_radius_score: 1.0 },
            false,
        ),
    };

    // 3. Resolve health report — conservative default (DEGRADED) if absent/unparseable/unvalidated.
    let health_path_str = parse_arg(&args, &["--health-report", "--health", "--health-file"]);
    let (health, health_verified): (ExecutionHealthInput, bool) = match health_path_str {
        Some(path_str) => {
            let path = PathBuf::from(&path_str);
            match fs::read_to_string(&path).ok().and_then(|c| serde_json::from_str::<ExecutionHealthInput>(&c).ok()) {
                Some(parsed) if parsed.input_validated => (parsed, true),
                Some(_) => {
                    eprintln!(
                        "WARNING: --health-report at {} was well-formed JSON but input_validated=false \
                         (execution_auditor did not recognize its source as a real trace); treating as unverified.",
                        path_str
                    );
                    (ExecutionHealthInput { execution_health: "DEGRADED".to_string(), input_validated: false }, false)
                }
                None => {
                    eprintln!("WARNING: --health-report at {} missing or unparseable; falling back to DEGRADED.", path_str);
                    (ExecutionHealthInput { execution_health: "DEGRADED".to_string(), input_validated: false }, false)
                }
            }
        }
        None => (ExecutionHealthInput { execution_health: "DEGRADED".to_string(), input_validated: false }, false),
    };

    let verified_inputs = impact_verified && health_verified;

    let ranked = rank_mutation(&candidate, &impact, &health, verified_inputs);
    let json_output = serde_json::to_string_pretty(&ranked)?;

    let output_path_str = parse_arg(&args, &["--output", "-o"]);
    if let Some(out_str) = output_path_str {
        let out_path = PathBuf::from(&out_str);
        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&out_path, &json_output)
            .with_context(|| format!("Failed to write output to {:?}", out_path))?;
    }

    println!("{}", json_output);

    Ok(())
}
