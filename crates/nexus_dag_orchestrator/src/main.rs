use regex::Regex;
use std::fs;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    println!("==> [DAG Orchestrator] Initializing Semantic Remediation Pipeline...");

    let report_path = "bounty_report.md";
    if !Path::new(report_path).exists() {
        println!("[!] No bounty_report.md found. Run nexus_ast_tool first.");
        return Ok(());
    }

    let report_content = fs::read_to_string(report_path)?;

    // Extract the Deferred block
    let deferred_section_marker = "## Deferred: Multi-Site Taint";
    let Some(start_idx) = report_content.find(deferred_section_marker) else {
        println!("==> [DAG Orchestrator] No multi-site taints deferred. System is clean.");
        return Ok(());
    };

    let deferred_block = &report_content[start_idx..];

    // Regex to parse our exact output from polyglot.rs
    let re = Regex::new(
        r"- `([^`]+)` sink lines (\d+)-(\d+) \([^)]+\): query variable `?([^` ]+)`? built via concatenation at lines (\d+)-(\d+)",
    )?;

    let mut compiled_missions = Vec::new();

    for cap in re.captures_iter(deferred_block) {
        let file_path = &cap[1];
        let sink_start = &cap[2];
        let sink_end = &cap[3];
        let var_name = &cap[4];
        let assign_start = &cap[5];

        let mission = format!(
            "OBJECTIVE: Remediate a Multi-Site SQL Injection in `{}`.\n\
            CONTEXT: The AST Auditor detected that the variable `{}` is constructed via unsafe string concatenation around line {}, and later executed at a sink around lines {}-{}.\n\
            \n\
            EXECUTION PLAN:\n\
            1. Use `read_file_chunk` on `{}` starting slightly before line {} and ending after line {} to capture the full data flow.\n\
            2. Analyze how `{}` is constructed. It likely concatenates untrusted user input.\n\
            3. Use `diff_patch` or `replace_lines` to rewrite the assignment to use a parameterized query template (e.g., `?` placeholders).\n\
            4. Use `diff_patch` or `replace_lines` to rewrite the execution sink to bind the parameters securely via prepared statements.\n\
            5. Run `cargo check` or evaluate the AST to ensure syntax is valid.\n\
            6. Declare `halt` when the file is successfully patched.",
            file_path, var_name, assign_start, sink_start, sink_end,
            file_path, assign_start, sink_end, var_name
        );
        compiled_missions.push((file_path.to_string(), mission));
    }

    println!(
        "==> [DAG Orchestrator] Compiled {} high-context DAG Missions.\n",
        compiled_missions.len()
    );

    for (i, (target, mission)) in compiled_missions.iter().enumerate() {
        println!("\x1b[1;36m=== MISSION {} : {} ===\x1b[0m", i + 1, target);
        println!("{}\n", mission);
    }

    println!("==> [DAG Orchestrator] Ready to pipe into runtime_core::task_manager::send_task().");

    Ok(())
}
