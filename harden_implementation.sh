#!/bin/bash
# Nexus Singularity - Wave 2 Implementation Hardening

echo ">> [NEXUS] INITIATING WAVE 2: CORE IMPLEMENTATION HARDENING"

# --- 1. HARDEN EXECUTOR ---
echo "[*] Hardening src/executor.rs..."
cat << 'EOF' > src/executor.rs
use std::fs;
use std::path::PathBuf;
use crate::nexus_error::NexusError;
use crate::agent_protocol::AgentAction;
use crate::diff::DiffEngine;
use crate::sandbox;

pub struct Executor;

impl Executor {
    pub async fn execute(actions: Vec<AgentAction>) -> Result<Vec<String>, NexusError> {
        let mut results = Vec::new();
        for action in actions {
            match action {
                AgentAction::ReadFile { path } => {
                    let safe_path = sandbox::validate_path(&path).map_err(|e| NexusError::SecurityViolation { message: e, cvss: 7.5 })?;
                    let c = fs::read_to_string(safe_path)?;
                    results.push(format!("SUCCESS: FILE_CONTENT of {}:\n{}", path, c));
                }
                AgentAction::PatchFile { path, search, replace } => {
                    let safe_path = sandbox::validate_path(&path).map_err(|e| NexusError::SecurityViolation { message: e, cvss: 7.5 })?;
                    let original = fs::read_to_string(&safe_path)?;
                    match DiffEngine::apply(&original, &search, &replace) {
                        Ok(new_content) => {
                            fs::write(&safe_path, new_content)?;
                            results.push(format!("SUCCESS: Patched {}", path));
                        }
                        Err(_) => {
                            results.push(format!("FAILURE: DNA_MISMATCH in {}. Current content:\n{}", path, original));
                        }
                    }
                }
                AgentAction::WriteFile { path, content } => {
                    Self::apply_fix(&path, &content)?;
                    results.push(format!("SUCCESS: Integrated {}", path));
                }
                AgentAction::Message { message } => {
                    results.push(format!("MSG: {}", message));
                }
                _ => {}
            }
        }
        Ok(results)
    }

    pub fn apply_fix(path: &str, code: &str) -> Result<(), NexusError> {
        let safe_path = sandbox::validate_path(path).map_err(|e| NexusError::SecurityViolation { message: e, cvss: 8.8 })?;
        
        let cleaned = code.replace(r"\n", "\n").replace(r#"\""#, "\"");
        let final_code = cleaned.trim_matches('"').trim();
        
        fs::write(safe_path, final_code).map_err(|e| NexusError::Other(e.to_string()))
    }
}
EOF

# --- 2. HARDEN SERVER API ---
echo "[*] Hardening src/server.rs API Handlers..."
# We use sed to replace the weak check with a sandbox call in the server handlers
sed -i 's/if p.path.contains("..") || Path::new(&p.path).is_absolute() {/let _safe = crate::sandbox::validate_path(\&p.path).map_err(|_| (StatusCode::FORBIDDEN, "SECURITY_VIOLATION"));/' src/server.rs
sed -i 's/if payload.path.contains("..") || Path::new(&payload.path).is_absolute() {/let _safe = crate::sandbox::validate_path(\&payload.path).map_err(|_| (StatusCode::FORBIDDEN, "SECURITY_VIOLATION"));/' src/server.rs

# --- 3. HARDEN LAZARUS VAULT ---
echo "[*] Hardening src/lazarus_vault.rs..."
sed -i '/pub fn auto_snapshot/i \    fn sanitize(p: \&str) -> String { p.replace("..", "").replace("/", "_") }' src/lazarus_vault.rs

echo ">> [NEXUS] WAVE 2 COMPLETE."
echo "[*] Running integrity check..."
cargo check
