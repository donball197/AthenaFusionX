use crate::motor_cortex::{MotorAction, MotorCommand};
use crate::policy::ExecutionSandbox;
use anyhow::{anyhow, Result};

pub struct ActionGate {
    sandbox: ExecutionSandbox,
}

impl ActionGate {
    pub fn new() -> Self {
        Self {
            sandbox: ExecutionSandbox {
            active_capabilities: vec![
                crate::policy::Capability::ReadSubstrate,
                crate::policy::Capability::WriteSubstrate,
                crate::policy::Capability::ExecuteSystemCall,
                crate::policy::Capability::HaltCognition,
            ],
        },
        }
    }

    /// Enforces deterministic security inspection on sealed MotorCommands.
    pub fn approve(&self, command: &MotorCommand) -> Result<()> {
        match &command.action {
            MotorAction::ExecuteCommand { command: cmd_str } => {
                let cmd_lower = cmd_str.to_lowercase();
                if cmd_lower.contains("rm -rf /") || cmd_lower.contains("rm -rf ~") || cmd_lower.contains("mkfs") {
                    return Err(anyhow!("ACTION_GATE_SECURITY_VIOLATION: Destructive command blocked: '{}'", cmd_str));
                }
                if cmd_lower.contains("cat .env") || cmd_lower.contains("env |") || cmd_lower.contains("printenv") {
                    return Err(anyhow!("ACTION_GATE_SECURITY_VIOLATION: Secret inspection command blocked: '{}'", cmd_str));
                }
                self.sandbox.validate_intent("execute_command")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }

            MotorAction::WriteFile { path, content } => {
                if path.contains("..") {
                    return Err(anyhow!("ACTION_GATE_SECURITY_VIOLATION: Path traversal detected: '{}'", path));
                }
                if content.contains("GEMINI_API_KEY=") || content.contains("NVIDIA_API_KEY=") {
                    return Err(anyhow!("ACTION_GATE_SECURITY_VIOLATION: Plaintext API key leak blocked in WriteFile."));
                }
                self.sandbox.validate_intent("write_file")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }

            MotorAction::ReadFile { .. } => {
                self.sandbox.validate_intent("read_file")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }

            MotorAction::ReadFileChunk { .. } => {
                self.sandbox.validate_intent("read_file_chunk")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }

            MotorAction::SearchCode { .. } => {
                self.sandbox.validate_intent("search_code")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }

            MotorAction::ReplaceLines { .. } => {
                self.sandbox.validate_intent("replace_lines")
                    .map_err(|e| anyhow::anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }

            MotorAction::RunTests { .. } => {
                self.sandbox.validate_intent("run_tests")
                    .map_err(|e| anyhow::anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }

            MotorAction::DiffPatch { .. } => {
                self.sandbox.validate_intent("diff_patch")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }

            MotorAction::PatchFunction { .. } => {
                self.sandbox.validate_intent("patch_function")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }

            MotorAction::SearchMemory { .. } => {
                self.sandbox.validate_intent("search_memory")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }

            MotorAction::Halt { .. } => {
                self.sandbox.validate_intent("halt")
                    .map_err(|e| anyhow!("ACTION_GATE_POLICY_FAULT: {}", e))?;
            }
        }

        Ok(())
    }
}

impl Default for ActionGate {
    fn default() -> Self {
        Self::new()
    }
}
