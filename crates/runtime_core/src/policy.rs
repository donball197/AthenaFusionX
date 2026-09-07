use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Capability {
    ReadSubstrate,
    WriteSubstrate,
    ExecuteSystemCall,
    HaltCognition,
}

pub struct ExecutionSandbox {
    pub active_capabilities: Vec<Capability>,
}

impl ExecutionSandbox {
    pub fn strict_lockdown() -> Self {
        Self { active_capabilities: vec![Capability::ReadSubstrate] }
    }

    pub fn validate_intent(&self, action_type: &str) -> Result<(), String> {
        let required_token = match action_type {
            "read_file" | "read_file_chunk" | "search_code" | "search_memory" | "scrape_url"
            | "read_function" => Capability::ReadSubstrate,
            "write_file" | "diff_patch" | "patch_function" | "replace_lines"
            | "lazarus_snapshot" | "export_report" | "synthesize_report" => {
                Capability::WriteSubstrate
            }
            "execute_command" | "run_tests" | "git_rollback" => Capability::ExecuteSystemCall,
            "halt" => Capability::HaltCognition,
            _ => return Err(format!("SECURITY_FAULT: Unknown action taxonomy '{}'", action_type)),
        };

        if self.active_capabilities.contains(&required_token) {
            Ok(())
        } else {
            Err(format!("POLICY_VIOLATION: Agent lacks {:?} capability.", required_token))
        }
    }
}

/// Explicit Machine-Readable Tool Contracts to prevent CLI flag-guessing and false-positive Critic vetoes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolContract {
    pub name: &'static str,
    pub binary_path: &'static str,
    pub flags: &'static [&'static str],
    pub usage_pattern: &'static str,
}

pub const TOOL_CONTRACTS: &[ToolContract] = &[
    ToolContract {
        name: "nexus_scrape",
        binary_path: "target/release/nexus_scrape",
        flags: &["url", "--extract"],
        usage_pattern: "nexus_scrape <URL> --extract \"<prompt>\" (Scrapes a website and uses an LLM to extract specific data as instructed by the prompt. Always outputs to stdout)",
    },
    ToolContract {
        name: "execute_command",
        binary_path: "internal::agent_action",
        flags: &["command"],
        usage_pattern: "execute_command <bash_command> (Executes shell command or binary in sandboxed PodmanShield environment)",
    },
    ToolContract {
        name: "search_memory",
        binary_path: "internal::agent_action",
        flags: &["query"],
        usage_pattern: "search_memory '<query>' (Queries native fastembed vector recall in nexus_substrate)",
    },
    ToolContract {
        name: "search_code",
        binary_path: "internal::agent_action",
        flags: &["path", "query"],
        usage_pattern: "search_code <path> <query> (Fast recursive search for keywords/patterns across all files in a directory)",
    },
    ToolContract {
        name: "read_file",
        binary_path: "internal::agent_action",
        flags: &["path"],
        usage_pattern: "read_file <filepath>",
    },
    ToolContract {
        name: "read_file_chunk",
        binary_path: "internal::agent_action",
        flags: &["path", "start_line", "max_lines"],
        usage_pattern: "read_file_chunk <filepath> <start_line> <max_lines> (paginated read for files too large for read_file)",
    },
    ToolContract {
        name: "replace_lines",
        binary_path: "internal::agent_action",
        flags: &["path", "start_line", "end_line", "replacement"],
        usage_pattern: "replace_lines <path> <start_line> <end_line> <replacement> (Surgically replaces lines without touching the rest of the file)",
    },
    ToolContract {
        name: "patch_function",
        binary_path: "internal::agent_action",
        flags: &["path", "function_name", "new_body"],
        usage_pattern: "patch_function <filepath> <function_name> <new_body> (Surgically replaces an entire Rust function via AST, ignoring whitespace issues. ALWAYS PREFERRED OVER write_file FOR RUST CODE.)",
    },
    ToolContract {
        name: "diff_patch",
        binary_path: "internal::agent_action",
        flags: &["path", "search", "replace"],
        usage_pattern: "diff_patch <filepath> <search> <replace> (Replaces exact string occurrences in a file)",
    },
    ToolContract {
        name: "run_tests",
        binary_path: "internal::agent_action",
        flags: &["test_target"],
        usage_pattern: "run_tests <test_target> (Runs cargo test on the specified package/module to verify code logic. Leave blank to test the whole workspace)",
    },
    ToolContract {
        name: "write_file",
        binary_path: "internal::agent_action",
        flags: &["path", "content"],
        usage_pattern: "write_file <filepath> <content> (DANGEROUS: Overwrites entire file. Prefer patch_function or replace_lines)",
    },
    ToolContract {
        name: "benchmark_runner",
        binary_path: "target/release/benchmark_runner",
        flags: &["--target", "--output", "--iterations"],
        usage_pattern: "benchmark_runner --target <symbol> --output /tmp/benchmark.json OR benchmark_runner <target> --output /tmp/benchmark.json",
    },
    ToolContract {
        name: "impact_analyzer",
        binary_path: "target/release/impact_analyzer",
        flags: &["--target", "--symbol-graph", "--mutation-map", "--output"],
        usage_pattern: "impact_analyzer --target <symbol> --output /tmp/impact.json OR impact_analyzer <target> --output /tmp/impact.json",
    },
    ToolContract {
        name: "execution_auditor",
        binary_path: "target/release/execution_auditor",
        flags: &["--trace-file", "--file", "-f", "--output"],
        usage_pattern: "execution_auditor --trace-file <log_path> --output /tmp/audit.json OR execution_auditor <log_path>",
    },
    ToolContract {
        name: "mutation_ranker",
        binary_path: "target/release/mutation_ranker",
        flags: &["--candidate", "--impact-report", "--health-report", "--output"],
        usage_pattern: "mutation_ranker --candidate <target> --impact-report /tmp/impact.json --health-report /tmp/audit.json --output /tmp/verdict.json",
    },
    ToolContract {
        name: "synthesize_report",
        binary_path: "internal::agent_action",
        flags: &["output_path", "source_files"],
        usage_pattern: "synthesize_report <output_path> <source_files> (Compiles and concatenates multiple source files into a final report)",
    },
];
