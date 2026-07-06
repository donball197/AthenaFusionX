import os

ee_path = "crates/runtime_core/src/epistemic_engine.rs"
with open(ee_path, "r") as f:
    ee_data = f.read()

# --- 1. PATCH THE TRANSLATOR ---
old_translator = """    pub async fn translate_to_action(client: &Client, api_key: &str, strategy: &str, proposed_action: &str) -> Result<String> {
        let prompt = format!("""

new_translator = """    pub async fn translate_to_action(client: &Client, api_key: &str, strategy: &str, proposed_action: &str) -> Result<String> {
        // --- ⚡ LOCAL HEURISTIC FAST-PATHS (0 RAM, 0 API CALLS) ---
        let action_lower = proposed_action.to_lowercase();
        
        // 1. Command Execution Fast-Path
        if action_lower.contains("execute") {
            if let Some(start) = proposed_action.find('`') {
                let rest = &proposed_action[start + 1..];
                if let Some(end) = rest.find('`') {
                    let cmd = &rest[..end];
                    let json_out = serde_json::json!([{
                        "type": "execute_command",
                        "command": cmd,
                        "reasoning": "Local fast-path translation."
                    }]);
                    println!("\\x1b[35m>> [TRANSLATOR] Local Fast-Path Engaged (Bypassing LLM)\\x1b[0m");
                    return Ok(json_out.to_string());
                }
            }
        }

        // 2. File Read Fast-Path
        if action_lower.starts_with("read the file") {
            let parts: Vec<&str> = proposed_action.split_whitespace().collect();
            if parts.len() >= 4 {
                let path = parts[3]; // Typically: "Read" "the" "file" "path/to/file"
                let json_out = serde_json::json!([{
                    "type": "read_file",
                    "path": path,
                    "reasoning": "Local fast-path translation."
                }]);
                println!("\\x1b[35m>> [TRANSLATOR] Local Fast-Path Engaged (Bypassing LLM)\\x1b[0m");
                return Ok(json_out.to_string());
            }
        }
        // --- END FAST-PATHS ---

        let prompt = format!("""

ee_data = ee_data.replace(old_translator, new_translator)

# --- 2. PATCH THE EVALUATOR ---
old_evaluator = """    pub async fn evaluate_evidence(client: &Client, api_key: &str, strategy: &str, evidence: &str) -> Result<CriticResponse> {
        let truncated_evidence: String = evidence.chars().take(8000).collect();"""

new_evaluator = """    pub async fn evaluate_evidence(client: &Client, api_key: &str, strategy: &str, evidence: &str) -> Result<CriticResponse> {
        // --- ⚡ LOCAL HEURISTIC SHORT-CIRCUITS (0 RAM, 0 API CALLS) ---
        
        // 1. Perfect Command Execution
        if evidence.contains("[CMD_OK]") && evidence.contains("exit status: 0") && !evidence.contains("STDERR:") {
            println!("\\x1b[33m>> [EVALUATOR] Local Heuristic Engaged: Clean Zero-Exit (Bypassing LLM)\\x1b[0m");
            return Ok(CriticResponse {
                critique: "[LOCAL HEURISTIC] Command executed cleanly with zero errors. Proceeding.".into(),
                confidence_modifier: 0.15,
            });
        }

        // 2. Successful File Read
        if evidence.contains("[READ_OK]") {
            println!("\\x1b[33m>> [EVALUATOR] Local Heuristic Engaged: File Read (Bypassing LLM)\\x1b[0m");
            return Ok(CriticResponse {
                critique: "[LOCAL HEURISTIC] Target file read successfully into context.".into(),
                confidence_modifier: 0.10,
            });
        }
        // --- END SHORT-CIRCUITS ---

        let truncated_evidence: String = evidence.chars().take(8000).collect();"""

ee_data = ee_data.replace(old_evaluator, new_evaluator)

with open(ee_path, "w") as f:
    f.write(ee_data)

print("[✓] API Starvation logic injected into Epistemic Engine.")
