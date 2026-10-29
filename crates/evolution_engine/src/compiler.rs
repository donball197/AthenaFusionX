use crate::genome::Genome;
use std::fs;
use std::path::Path;

pub struct PromptCompiler;

impl PromptCompiler {
    /// Dynamically scans the apps/ directory to build a live whitelist of authorized tools.
    fn get_live_toolchain_whitelist() -> String {
        let mut tools = Vec::new();
        // The Daemon runs from the workspace root (AthenaFusionX/)
        let apps_dir = Path::new("apps");
        
        if let Ok(entries) = fs::read_dir(apps_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    // Crate names are the directory names in apps/
                    if file_type.is_dir() {
                        tools.push(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
        }
        
        if tools.is_empty() {
            return "No native tools currently registered.".to_string();
        }
        
        tools.join(", ")
    }

    pub fn compile_planner_prompt(genome: &Genome) -> String {
        let live_tools = Self::get_live_toolchain_whitelist();

        format!(
            "SYSTEM_POLICY: [PLANNER]\nStyle: {:?}\nDecomposition: {}\nBranching: {}\nRisk Threshold: {:.2}\n\n\
            STRICT CONSTRAINTS: Follow the defined architectural patterns.\n\n\
            AVAILABLE TOOLCHAIN:\n\
            [{}]\n\n\
            TOOL INSTRUCTION: AST PATCHER\n\
            When you need to mutate or rewrite Rust code, NEVER use standard sed, awk, or diff. YOU MUST use the semantic AST Patcher.\n\
            1. Write your completely new, valid Rust function or impl block into a temporary file (e.g., /tmp/mutation.rs).\n\
            2. Invoke the tool exactly like this:\n\
               ./target/release/ast_patcher --target <path/to/file.rs> --symbol <function_name> --patch-file /tmp/mutation.rs\n\
            3. The tool will safely parse the AST, replace the target symbol, and auto-format. Do not worry about exact line numbers.",
            genome.genes.planning.style,
            genome.genes.planning.decomposition_depth,
            genome.genes.planning.branch_factor,
            genome.genes.risk_threshold,
            live_tools
        )
    }

    pub fn compile_critic_prompt(genome: &Genome) -> String {
        let live_tools = Self::get_live_toolchain_whitelist();

        format!(
            "SYSTEM_POLICY: [CRITIC]\nStyle: {:?}\nVeto Threshold: {:.2}\nEvidence Weight: {:.2}\n\n\
            STRICT CONSTRAINTS: Evaluate without bias according to strict metrics.\n\n\
            CRITICAL RULE 4: TOOLCHAIN TRUST & EXPLORATORY GRACE\n\
            The following native tools are currently compiled, registered, and PRE-AUTHORIZED for execution:\n\
            [{}]\n\
            If the Strategist proposes executing ANY of these tools via target/release/ or cargo run, you MUST ASSUME the tool exists and is authorized. Do NOT veto execution on the basis of missing prerequisite files (like trace logs or candidate JSONs). Allow exploratory tool execution.",
            genome.genes.critic.style,
            genome.genes.critic.veto_threshold,
            genome.genes.critic.evidence_weight,
            live_tools
        )
    }
}
