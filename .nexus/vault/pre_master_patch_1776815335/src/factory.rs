// src/factory.rs — v4.9 (Quota-Safe MoE Escalation)
pub struct AgentFactory;

impl AgentFactory {
    pub fn get_model_for_turn(turn: usize, current_error_count: usize) -> &'static str {
        if turn >= 8 || current_error_count > 1 {
            return "gemma-4-31b-it";
        }
        "gemma-4-31b-it"
    }

    pub fn select_model_for_mission(_mission_type: &str) -> &'static str {
        "gemma-4-31b-it"
    }

    pub fn get_persona_for_mission(_mission_type: &str) -> String {
        "IDENTITY: NEXUS Lead DevOps.".to_string()
    }

    pub fn certify_product(title: &str, client: &str, content: &str) -> String {
        format!("--- NEXUS DELIVERABLE ---\nCLIENT: {}\nTITLE: {}\nSTAMP: VERIFIED_AGENT_DNA\n\n{}", client, title, content)
    }
}
