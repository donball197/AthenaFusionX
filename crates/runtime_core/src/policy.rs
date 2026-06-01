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
        Self { 
            active_capabilities: vec![Capability::ReadSubstrate] 
        }
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
