/**
 * NEXUS AGENT FACTORY — v2.6 (Industrial Fusion)
 * Handles Turn-based Escalation, Mission Selection, and Product Certification.
 */

pub struct AgentFactory;

impl AgentFactory {
    /// get_model_for_turn: Used by TaskManager to escalate intelligence if stuck.
    pub fn get_model_for_turn(turn: usize, current_error_count: usize) -> &'static str {
        // Turn 1-2: Sovereign MoE (Efficient)
        // Turn 3: Sovereign Dense Sister (Deep)
        // Turn 4+: Gemini CEO (Emergency Support)
        if turn >= 4 || current_error_count > 2 {
            return "gemini-2.5-pro";
        }
        if turn == 3 || current_error_count > 0 {
            return "gemma-4-31b-it";
        }
        "gemma-4-26b-a4b-it"
    }

    /// select_model_for_mission: Direct model assignment for specific industries.
    pub fn select_model_for_mission(mission_type: &str) -> &'static str {
        match mission_type {
            "robotics" | "ros" => "gemini-robotics-er-1.5-preview",
            "security" | "audit" => "gemma-4-31b-it",
            "coding" | "refactor" => "gemma-4-26b-a4b-it",
            _ => "gemini-2.0-flash-lite"
        }
    }

    pub fn certify_product(title: &str, client: &str, content: &str) -> String {
        format!(
            "--- NEXUS INDUSTRIAL DELIVERABLE ---\nCLIENT: {}\nTITLE: {}\nSUBSTRATE: ARM64_SOVEREIGN\n------------------------------------\n\n{}\n\n------------------------------------\nSTAMP: VERIFIED_AGENT_DNA",
            client, title, content
        )
    }
}
