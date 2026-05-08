#!/bin/bash
echo "=== Integrating model_config.rs ==="

# Add to Cargo.toml
if ! grep -q "model_config" crates/runtime_core/src/lib.rs; then
  echo "pub mod model_config;" >> crates/runtime_core/src/lib.rs
  echo "Added mod declaration to lib.rs"
fi

# Update factory.rs
cat << 'FACTORY' > crates/runtime_core/src/factory.rs
use crate::model_config;

pub struct AgentFactory;

impl AgentFactory {
    pub fn get_model_for_turn(_turn: usize, _current_error_count: usize) -> &'static str {
        model_config::get_model()
    }

    pub fn select_model_for_mission(_mission_type: &str) -> &'static str {
        model_config::get_model()
    }

    pub fn get_persona_for_mission(mission_type: &str) -> String {
        match mission_type {
            "security" => "IDENTITY: NEXUS Security Auditor...".to_string(),
            "docs" => "IDENTITY: NEXUS Archivist...".to_string(),
            "qa" => "IDENTITY: NEXUS QA Agent...".to_string(),
            "dream" => "IDENTITY: NEXUS Archivist...".to_string(),
            _ => "IDENTITY: NEXUS Lead DevOps...".to_string(),
        }
    }

    pub fn certify_product(title: &str, client: &str, content: &str) -> String {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("--- NEXUS DELIVERABLE ---\nCLIENT: {}\nTITLE: {}\nSTAMP: VERIFIED_AGENT_DNA\nTS: {}\n\n{}", client, title, ts, content)
    }
}
FACTORY

echo "Updated factory.rs"

# Quick update for other major files
echo "Updating orchestrator.rs, neural_agent.rs, conductor.rs..."

find crates/runtime_core -name "*.rs" -exec sed -i 's/"gemma-4-26b-a4b-it"/crate::model_config::get_model()/g' {} +
find crates/runtime_core -name "*.rs" -exec sed -i 's/gemma-4-26b-a4b-it/crate::model_config::get_model()/g' {} +

echo "=== Integration complete ==="
echo "Now run: cargo check -p nexus_runtime_core"
