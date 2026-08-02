#[derive(Debug, Clone, PartialEq)]
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
    /// Default safe-mode: The agent can read, but cannot mutate or execute.
    pub fn strict_lockdown() -> Self {
        Self { active_capabilities: vec![Capability::ReadSubstrate] }
    }

    /// Validates the intended action against granted capabilities before it hits the OS
    pub fn validate_intent(&self, action_type: &str) -> Result<(), String> {
        let required_token = match action_type {
            "read_file" | "search_memory" => Capability::ReadSubstrate,
            "write_file" | "diff_patch" | "patch_function" => Capability::WriteSubstrate,
            "execute_command" => Capability::ExecuteSystemCall,
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

/// Explicit Machine-Readable Tool Contracts to prevent CLI flag-guessing.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolContract {
    pub name: &'static str,
    pub binary_path: &'static str,
    pub flags: &'static [&'static str],
    pub usage_pattern: &'static str,
}

pub const TOOL_CONTRACTS: &[ToolContract] = &[
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
];
