import os

# --- 1. SUTURE: orchestrator.rs (Purge Decoy Fields) ---
fpath_orch = "src/orchestrator.rs"
if os.path.exists(fpath_orch):
    with open(fpath_orch, "r") as f:
        code = f.read()
    
    # Replace the bloated properties object with the streamlined core
    old_props = """"properties": {
                        "type": { 
                            "type": "STRING",
                            "enum": ["read_file", "write_file", "execute_command", "halt", "search_memory", "export_report", "fetch_ui_snapshot"]
                        },
                        "path": { "type": "STRING" },
                        "content": { "type": "STRING" },
                        "command": { "type": "STRING" },
                        "reasoning": { "type": "STRING" },
                        "query": { "type": "STRING" },
                        "title": { "type": "STRING" }
                    },"""
    new_props = """"properties": {
                        "type": { 
                            "type": "STRING",
                            "enum": ["read_file", "write_file", "execute_command", "halt"]
                        },
                        "path": { "type": "STRING" },
                        "content": { "type": "STRING" },
                        "command": { "type": "STRING" },
                        "reasoning": { "type": "STRING" }
                    },"""
    
    code = code.replace(old_props, new_props)
    with open(fpath_orch, "w") as f:
        f.write(code)
    print("\x1b[32m>> [SUTURE] Orchestrator schema pruned.\x1b[0m")

# --- 2. SUTURE: json_parser.rs (Precise Serde Errors) ---
fpath_parser = "src/json_parser.rs"
parser_code = """use anyhow::{anyhow, Result};
use crate::agent_protocol::AgentAction;

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    // Because we enforce application/json at the API level, the output is guaranteed JSON.
    // We strip any accidental markdown fences just in case.
    let text = response.trim().replace("```json", "").replace("```", "").trim().to_string();

    // ⚡ IRON FEEDBACK: Expose the exact serde struct error to the LLM
    match serde_json::from_str::<Vec<AgentAction>>(&text) {
        Ok(actions) => Ok(actions),
        Err(e) => Err(anyhow!("SCHEMA_MISMATCH: {}", e))
    }
}
"""
with open(fpath_parser, "w") as f:
    f.write(parser_code)
print("\x1b[32m>> [SUTURE] json_parser.rs upgraded to feed precise errors.\x1b[0m")

