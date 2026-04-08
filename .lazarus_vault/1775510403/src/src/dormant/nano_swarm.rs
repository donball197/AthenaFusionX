use std::process::Command;
use serde::Serialize;
use tokio::sync::broadcast;
use crate::resonator::ResonatorCore; // Import the Zig Interface

#[derive(Clone, Serialize, Debug)]
pub struct AgentStatus {
    pub id: u32,
    pub name: String,
    pub target: String,
    pub state: String,
}

#[derive(Clone)]
pub struct NanoSwarm {
    tx: broadcast::Sender<AgentStatus>,
}

impl NanoSwarm {
    pub fn new(tx: broadcast::Sender<AgentStatus>) -> Self {
        Self { tx }
    }

    pub fn get_roster(&self) -> String {
        "- SENTINEL (Rust Compiler)\n- INQUISITOR (Python Validator)".to_string()
    }

    pub fn trigger_diagnostics(&self, file_path: String) {
        let tx = self.tx.clone();
        
        tokio::spawn(async move {
            if file_path.ends_with(".rs") {
                // 1. Notify Web UI (WebSocket)
                let _ = tx.send(AgentStatus { id: 1, name: "SENTINEL".into(), target: "Compiling...".into(), state: "active".into() });
                
                // 2. Notify Zig/NPU (Hardware Pulse)
                ResonatorCore::sync_agent(1, "active");

                let output = Command::new("cargo").arg("check").output();
                match output {
                    Ok(out) if out.status.success() => {
                        let _ = tx.send(AgentStatus { id: 1, name: "SENTINEL".into(), target: "SECURE".into(), state: "idle".into() });
                        ResonatorCore::sync_agent(1, "idle");
                    },
                    _ => {
                        let _ = tx.send(AgentStatus { id: 1, name: "SENTINEL".into(), target: "COMPILE FAIL".into(), state: "alert".into() });
                        ResonatorCore::sync_agent(1, "alert"); 
                    }
                }
            }
        });
    }
}
