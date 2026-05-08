use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Capability {
    ReadFile,
    WriteFile,
    ExecuteProcess,
    NetworkAccess,
    MemoryModify,
}

#[derive(Debug, Clone)]
pub struct IsolationBoundary {
    pub allowed_capabilities: std::collections::HashSet<Capability>,
    pub run_as_wasm: bool,
}

impl IsolationBoundary {
    pub fn enforce(&self, intent: &Capability) -> Result<(), String> {
        if !self.allowed_capabilities.contains(intent) {
            return Err(format!(
                "SECURITY FAULT: Capability {:?} denied by sandbox isolation.",
                intent
            ));
        }
        Ok(())
    }
}
