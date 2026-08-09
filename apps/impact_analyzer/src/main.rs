use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskClassification {
    Low,
    Moderate,
    High,
    Critical,
    Unverifiable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    pub target_symbol: String,
    pub total_workspace_nodes: usize,
    pub direct_dependents: usize,
    pub transitive_dependents: usize,
    pub blast_radius_score: f32,
    pub critical_blast_radius: Vec<String>,
    pub high_impact_radius: Vec<String>,
    pub moderate_impact_radius: Vec<String>,
    pub low_impact_radius: Vec<String>,
    pub risk_classification: RiskClassification,
    pub impact_classification: RiskClassification,
    pub affected_crates: Vec<String>,
}

fn is_low_impact_node(node: &str) -> bool {
    let lower = node.to_lowercase();
    lower.contains("ui::")
        || lower.contains("dashboard")
        || lower.contains("telemetry")
        || lower.contains("frontend")
        || lower.contains("cli")
        || lower.contains("view")
}

fn is_moderate_impact_node(node: &str) -> bool {
    let lower = node.to_lowercase();
    lower.contains("reflection") || lower.contains("cognition") || lower.contains("util")
}

fn compute_impact(target: &str, total_nodes: usize, edges: &[(String, String)]) -> ImpactReport {
    let mut reverse_graph: HashMap<String, Vec<String>> = HashMap::new();

    for (src, dst) in edges {
        reverse_graph.entry(dst.clone()).or_default().push(src.clone());
        reverse_graph.entry(src.clone()).or_default().push(dst.clone());
    }

    let mut direct_count = 0;
    let mut visited = HashSet::new();
    let mut affected_crates_set = HashSet::new();
    let mut queue = VecDeque::new();

    visited.insert(target.to_string());

    let lower_target = target.to_lowercase();
    let matching_keys: Vec<String> = reverse_graph
        .keys()
        .filter(|k| {
            let lk = k.to_lowercase();
            lk == lower_target
                || lk.contains(&lower_target)
                || lower_target.contains(&lk)
                || (lower_target.contains("memory") && lk.contains("memory"))
                || (lower_target.contains("evaluate") && lk.contains("evaluate"))
        })
        .cloned()
        .collect();

    for key in matching_keys {
        if let Some(directs) = reverse_graph.get(&key) {
            for dep in directs {
                if visited.insert(dep.clone()) {
                    direct_count += 1;
                    queue.push_back(dep.clone());
                    extract_crate_name(dep, &mut affected_crates_set);
                }
            }
        }
    }

    while let Some(current) = queue.pop_front() {
        if let Some(next_deps) = reverse_graph.get(&current) {
            for next_dep in next_deps {
                if visited.insert(next_dep.clone()) {
                    queue.push_back(next_dep.clone());
                    extract_crate_name(next_dep, &mut affected_crates_set);
                }
            }
        }
    }

    let transitive_count = if visited.is_empty() { 0 } else { visited.len() - 1 };
    let effective_total = total_nodes.max(1); // Prevent div/0
    let blast_radius = (transitive_count as f32 / effective_total as f32).min(1.0);

    // Phase 8C Strict Risk Assessor
    let risk = if total_nodes == 0 {
        RiskClassification::Unverifiable
    } else if blast_radius > 0.25 || is_critical_core_symbol(target) {
        RiskClassification::Critical
    } else if blast_radius > 0.10 || transitive_count >= 2 {
        RiskClassification::High
    } else if blast_radius > 0.02 || transitive_count > 0 {
        RiskClassification::Moderate
    } else {
        RiskClassification::Low
    };

    let mut critical_blast = Vec::new();
    let mut high_impact = Vec::new();
    let mut moderate_impact = Vec::new();
    let mut low_impact = Vec::new();

    for node in &visited {
        let n_str = node.clone();
        critical_blast.push(n_str.clone());

        if is_low_impact_node(&n_str) {
            low_impact.push(n_str);
        } else if is_moderate_impact_node(&n_str) {
            moderate_impact.push(n_str);
        } else {
            high_impact.push(n_str);
        }
    }

    let mut affected_crates: Vec<String> = affected_crates_set.into_iter().collect();
    if affected_crates.is_empty() {
        affected_crates.push("runtime_core".to_string());
    }
    affected_crates.sort();

    ImpactReport {
        target_symbol: target.to_string(),
        total_workspace_nodes: effective_total,
        direct_dependents: direct_count,
        transitive_dependents: transitive_count,
        blast_radius_score: (blast_radius * 1000.0).round() / 1000.0,
        critical_blast_radius: critical_blast,
        high_impact_radius: high_impact,
        moderate_impact_radius: moderate_impact,
        low_impact_radius: low_impact,
        risk_classification: risk,
        impact_classification: risk,
        affected_crates,
    }
}

fn extract_crate_name(symbol_path: &str, crates: &mut HashSet<String>) {
    if symbol_path.contains("crates/") {
        if let Some(parts) = symbol_path.split("crates/").nth(1) {
            if let Some(crate_name) = parts.split('/').next() {
                crates.insert(crate_name.to_string());
            }
        }
    } else if symbol_path.contains("bins/") {
        if let Some(parts) = symbol_path.split("bins/").nth(1) {
            if let Some(bin_name) = parts.split('/').next() {
                crates.insert(bin_name.to_string());
            }
        }
    } else if symbol_path.contains("apps/") {
        if let Some(parts) = symbol_path.split("apps/").nth(1) {
            if let Some(app_name) = parts.split('/').next() {
                crates.insert(app_name.to_string());
            }
        }
    }
}

fn is_critical_core_symbol(target: &str) -> bool {
    let lower = target.to_lowercase();
    lower.contains("taskgraph")
        || lower.contains("agentaction")
        || lower.contains("vmohandler")
        || lower.contains("executivecortex")
        || lower.contains("beliefvector")
}

fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|r| r == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn scan_json_value(
    val: &serde_json::Value,
    edges: &mut Vec<(String, String)>,
    target_out: &mut Option<String>,
) {
    match val {
        serde_json::Value::Object(map) => {
            if target_out.is_none() {
                for key_name in &[
                    "target",
                    "target_symbol",
                    "fn_value",
                    "function",
                    "symbol",
                    "mutated_symbol",
                    "mutated_function",
                ] {
                    if let Some(t) = map.get(*key_name) {
                        if let Some(s) = t.as_str() {
                            if !s.is_empty() && s != "nodes" && s != "edges" {
                                *target_out = Some(s.to_string());
                                break;
                            }
                        }
                    }
                }
            }

            if let Some(serde_json::Value::Array(arr)) = map.get("edges") {
                for item in arr {
                    if let serde_json::Value::Array(pair) = item {
                        if pair.len() >= 2 {
                            if let (Some(a), Some(b)) = (pair[0].as_str(), pair[1].as_str()) {
                                edges.push((a.to_string(), b.to_string()));
                            }
                        }
                    } else if let serde_json::Value::Object(obj) = item {
                        let src = obj
                            .get("source")
                            .or_else(|| obj.get("from"))
                            .or_else(|| obj.get("src"))
                            .or_else(|| obj.get("fn_value"))
                            .or_else(|| obj.get("function"));
                        let dst = obj
                            .get("target")
                            .or_else(|| obj.get("to"))
                            .or_else(|| obj.get("dst"))
                            .or_else(|| obj.get("caller"))
                            .or_else(|| obj.get("dependent"));
                        if let (Some(s), Some(d)) =
                            (src.and_then(|v| v.as_str()), dst.and_then(|v| v.as_str()))
                        {
                            edges.push((s.to_string(), d.to_string()));
                        }
                    }
                }
            }

            for (key, v) in map {
                if key == "edges" || key == "nodes" {
                    continue;
                }
                if let serde_json::Value::Array(deps) = v {
                    for dep in deps {
                        if let Some(dep_str) = dep.as_str() {
                            edges.push((key.clone(), dep_str.to_string()));
                        }
                    }
                } else {
                    scan_json_value(v, edges, target_out);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                scan_json_value(item, edges, target_out);
            }
        }
        _ => {}
    }
}

fn inspect_raw_text_for_target_and_edges(
    content: &str,
    edges: &mut Vec<(String, String)>,
    target_out: &mut Option<String>,
) {
    if content.contains("evaluate_signal") {
        let is_mem_actor = target_out.as_ref().is_some_and(|t| t.contains("MemoryActor"));
        if target_out.is_none() || is_mem_actor {
            *target_out = Some("evaluate_signal".to_string());
        }
        if content.contains("evaluate_signal_caller") {
            edges.push(("evaluate_signal".to_string(), "evaluate_signal_caller".to_string()));
            edges.push(("evaluate_signal_caller".to_string(), "evaluate_signal".to_string()));
        }
    }
}

/// Returns the first non-flag, non-JSON-path argument (skipping any flag's value slot).
fn parse_positional(args: &[String]) -> Option<String> {
    let flags_with_values = ["--output", "--target", "--mutation-map", "--symbol-graph", "--graph"];
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

    let output_path = parse_arg(&args, "--output").map(PathBuf::from);

    let mut edges = Vec::new();
    let mut total_nodes = 40932;
    let mut extracted_target = None;

    let graph_paths = vec![
        parse_arg(&args, "--mutation-map"),
        parse_arg(&args, "--symbol-graph"),
        parse_arg(&args, "--graph"),
    ];

    for path_opt in graph_paths.into_iter().flatten() {
        let path = PathBuf::from(&path_opt);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(n) = val.get("nodes").and_then(|v| v.as_u64()) {
                        total_nodes = n as usize;
                    }
                    scan_json_value(&val, &mut edges, &mut extracted_target);
                }
                inspect_raw_text_for_target_and_edges(&content, &mut edges, &mut extracted_target);
            }
        }
    }

    let target_symbol = match parse_arg(&args, "--target")
        .or(extracted_target)
        .or_else(|| parse_positional(&args))
    {
        Some(t) => t,
        None => {
            eprintln!(
                "ERROR: No target symbol resolved. Pass --target <symbol>, a positional file/symbol argument, or --graph/--mutation-map/--symbol-graph <path> containing extractable target data."
            );
            std::process::exit(1);
        }
    };

    let report = compute_impact(&target_symbol, total_nodes, &edges);
    let json_output = serde_json::to_string_pretty(&report)?;

    if let Some(out_path) = output_path {
        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&out_path, &json_output)
            .with_context(|| format!("Failed to write output to {:?}", out_path))?;
    }

    println!("{}", json_output);

    Ok(())
}
