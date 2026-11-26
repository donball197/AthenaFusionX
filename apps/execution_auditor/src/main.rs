use anyhow::{Context, Result};
use nexus_cli_utils::parse_arg;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionHealth {
    Healthy,
    Degraded,
    Brittle,
    Critical,
    Unverifiable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionAuditReport {
    pub path_denials: usize,
    pub translator_failures: usize,
    pub strategy_retries: usize,
    pub premature_halts: usize,
    pub average_belief_delta: f32,
    pub execution_health: ExecutionHealth,
    pub input_validated: bool,
}

impl ExecutionAuditReport {
    pub fn compute(
        path_denials: usize,
        translator_failures: usize,
        strategy_retries: usize,
        premature_halts: usize,
        belief_deltas: &[f32],
        input_validated: bool,
    ) -> Self {
        let average_belief_delta = if belief_deltas.is_empty() {
            0.0
        } else {
            belief_deltas.iter().sum::<f32>() / belief_deltas.len() as f32
        };

        let execution_health = if !input_validated {
            ExecutionHealth::Unverifiable
        } else if path_denials >= 5 || translator_failures >= 3 {
            ExecutionHealth::Critical
        } else if path_denials >= 1 || translator_failures >= 1 || premature_halts >= 1 {
            ExecutionHealth::Degraded
        } else if strategy_retries >= 3 {
            ExecutionHealth::Brittle
        } else {
            ExecutionHealth::Healthy
        };

        Self {
            path_denials,
            translator_failures,
            strategy_retries,
            premature_halts,
            average_belief_delta: (average_belief_delta * 100.0).round() / 100.0,
            execution_health,
            input_validated,
        }
    }
}

fn looks_like_execution_trace(content: &str) -> bool {
    const MARKERS: &[&str] = &[
        "[CORTEX]",
        "[STRATEGIST",
        "[CRITIC]",
        "[EVIDENCE ACQUIRED]",
        "[CMD_OK]",
        "[EVALUATION]",
        "STRATEGIST PROPOSAL",
        "DAG NODE",
    ];
    MARKERS.iter().any(|m| content.contains(m))
}

fn analyze_log_content(content: &str) -> ExecutionAuditReport {
    let input_validated = looks_like_execution_trace(content);

    let mut path_denials = 0;
    let mut translator_failures = 0;
    let mut strategy_retries = 0;
    let mut premature_halts = 0;
    let mut belief_deltas = Vec::new();

    if input_validated {
        let mut last_belief: Option<f32> = None;

        for line in content.lines() {
            let lower = line.to_lowercase();

            if lower.contains("path_traversal_denied")
                || lower.contains("sandbox_violation")
                || lower.contains("policydenial")
            {
                path_denials += 1;
            }

            if lower.contains("failed to parse agentaction json")
                || lower.contains("parser_fracture")
                || lower.contains("cortex fault")
            {
                translator_failures += 1;
            }

            if lower.contains("strategy completely abandoned")
                || lower.contains("pivoting graph")
                || lower.contains("veto. proposal rejected")
            {
                strategy_retries += 1;
            }

            if lower.contains("halting early") || lower.contains("early halt") {
                premature_halts += 1;
            }

            if line.contains("Strategy Belief mathematically updated to") {
                if let Some(val_str) =
                    line.split("Strategy Belief mathematically updated to").nth(1)
                {
                    if let Ok(current_belief) = val_str.trim().parse::<f32>() {
                        if let Some(prev) = last_belief {
                            let delta = (current_belief - prev).abs();
                            belief_deltas.push(delta);
                        }
                        last_belief = Some(current_belief);
                    }
                }
            }
        }
    }

    ExecutionAuditReport::compute(
        path_denials,
        translator_failures,
        strategy_retries,
        premature_halts,
        &belief_deltas,
        input_validated,
    )
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let file_path = parse_arg(&args, &["--trace-file", "--file", "-f", "--trace"]).or_else(|| {
        args.iter().skip(1).find(|s| !s.starts_with('-') && !s.ends_with(".json")).cloned()
    });

    let log_content = if let Some(path_str) = file_path {
        let log_path = PathBuf::from(&path_str);
        if log_path.exists() {
            fs::read_to_string(&log_path)
                .with_context(|| format!("Failed to read log file: {:?}", log_path))?
        } else {
            let mut buffer = String::new();
            let _ = io::stdin().read_to_string(&mut buffer);
            buffer
        }
    } else {
        let mut buffer = String::new();
        let _ = io::stdin().read_to_string(&mut buffer);
        buffer
    };

    let report = analyze_log_content(&log_content);

    if !report.input_validated {
        eprintln!(
            "WARNING: input did not contain recognizable CORTEX execution-trace markers \
             ([CORTEX], [CMD_OK], [EVALUATION], etc.). Reporting execution_health as UNVERIFIABLE \
             rather than fabricating a HEALTHY result from unrelated input."
        );
    }

    let json_output = serde_json::to_string_pretty(&report)?;

    let output_path_str = parse_arg(&args, &["--output", "-o"]);
    if let Some(out_str) = output_path_str {
        let out_path = PathBuf::from(&out_str);
        nexus_cli_utils::write_text_output(&out_path, &json_output)?;
    }

    println!("{}", json_output);

    Ok(())
}
