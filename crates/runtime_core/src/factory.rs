use std::time::{SystemTime, UNIX_EPOCH};

pub struct AgentFactory;

impl AgentFactory {
    pub fn get_model_for_turn(turn: usize, current_error_count: usize) -> &'static str {
        if turn >= 8 || current_error_count > 1 { "gemma-4-26b-a4b-it" } else { "gemma-4-26b-a4b-it" }
    }

    pub fn select_model_for_mission(mission_type: &str) -> &'static str {
        match mission_type {
            "heavy_reasoning" | "refactor" | "security" | "dream" => "gemma-4-26b-a4b-it",
            _ => "gemma-4-26b-a4b-it",
        }
    }

    pub fn get_persona_for_mission(mission_type: &str) -> String {
        match mission_type {
            "security" => "IDENTITY: NEXUS Security Auditor. ROLE: Identify logic flaws, path traversal, unsafe unwraps, and auth gaps. OUTPUT: ONLY a valid JSON array. No prose.".to_string(),
            "docs" => "IDENTITY: NEXUS Archivist. ROLE: Generate technical markdown, README files, and architecture maps. OUTPUT: ONLY a valid JSON array of write_file actions.".to_string(),
            "qa" => "IDENTITY: NEXUS QA Agent. ROLE: Ensure 100% test coverage and zero compiler warnings. WORKFLOW: cargo check → cargo test → fix failures → halt. OUTPUT: ONLY a valid JSON array.".to_string(),
            "dream" => "IDENTITY: NEXUS Archivist. ROLE: Synthesize memory strands into architectural insights. OUTPUT: Plain text summary — no JSON required.".to_string(),
            _ => "IDENTITY: NEXUS Lead DevOps. ROLE: Write high-performance Rust code and manage system operations. OUTPUT: ONLY a valid JSON array. No prose.".to_string(),
        }
    }

    pub fn certify_product(title: &str, client: &str, content: &str) -> String {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        format!("--- NEXUS DELIVERABLE ---\nCLIENT: {}\nTITLE: {}\nSTAMP: VERIFIED_AGENT_DNA\nTS: {}\n\n{}", client, title, ts, content)
    }
}
