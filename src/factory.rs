/**
 * NEXUS AGENT FACTORY — v4.7 (No Gemini Escalation)
 */

pub struct AgentFactory;

impl AgentFactory {
    /// get_model_for_turn: Removed gemini-2.5-pro to prevent quota errors.
    /// Uses gemma-4-31b-it for complex recovery or deep turns.
    pub fn get_model_for_turn(turn: usize, current_error_count: usize) -> &'static str {
        if turn >= 8 || current_error_count > 1 {
            return "gemma-4-31b-it"; 
        }
        "gemma-4-26b-a4b-it"
    }

    pub fn select_model_for_mission(mission_type: &str) -> &'static str {
        match mission_type {
            "security" | "audit" | "recovery" => "gemma-4-31b-it",
            _ => "gemma-4-26b-a4b-it",
        }
    }

    pub fn get_persona_for_mission(mission_type: &str) -> String {
        match mission_type {
            "devops" => "IDENTITY: NEXUS Lead DevOps. FOCUS: Git isolation and build stability.".to_string(),
            "security" => "IDENTITY: NEXUS SecOps. FOCUS: FFI boundaries and memory safety.".to_string(),
            "architect" => "IDENTITY: NEXUS Architect. FOCUS: VMO substrate and Zig muscle integration.".to_string(),
            _ => "IDENTITY: NEXUS Generalist.".to_string(),
        }
    }

    pub fn certify_product(title: &str, client: &str, content: &str) -> String {
        format!("--- NEXUS DELIVERABLE ---\nCLIENT: {}\nTITLE: {}\nSTAMP: VERIFIED_AGENT_DNA\n\n{}", client, title, content)
    }
}
