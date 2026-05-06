import os
import re

# 1. Gut the parser. It now exclusively parses the raw JSON array guaranteed by the API.
with open("src/json_parser.rs", "w") as f:
    f.write("""// src/json_parser.rs — v2.0 [STRUCTURED_OUTPUTS_STABLE]
use crate::agent_protocol::AgentAction;
use anyhow::{anyhow, Result};

pub fn extract_actions(response: &str) -> Result<Vec<AgentAction>> {
    serde_json::from_str::<Vec<AgentAction>>(response).map_err(|e| {
        let preview = &response[..response.len().min(400)];
        anyhow!("JSON_DECODE_FRACTURE: {} | Raw block:\\n{}", e, preview)
    })
}
""")

SCHEMA = """
        "generationConfig": {
            "temperature": 0.4,
            "responseMimeType": "application/json",
            "responseSchema": {
                "type": "ARRAY",
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "reasoning": { "type": "STRING", "description": "Mandatory step-by-step logic (max 3 sentences)" },
                        "type": { "type": "STRING" },
                        "path": { "type": "STRING" },
                        "content": { "type": "STRING" },
                        "command": { "type": "STRING" },
                        "query": { "type": "STRING" },
                        "name": { "type": "STRING" },
                        "func_name": { "type": "STRING" },
                        "input": { "type": "NUMBER" },
                        "size": { "type": "INTEGER" },
                        "handle": { "type": "STRING" },
                        "data": { "type": "STRING" },
                        "enabled": { "type": "BOOLEAN" },
                        "id": { "type": "STRING" },
                        "label": { "type": "STRING" },
                        "kind": { "type": "STRING" },
                        "source": { "type": "STRING" },
                        "directive": { "type": "STRING" },
                        "tag": { "type": "STRING" },
                        "title": { "type": "STRING" }
                    },
                    "required": ["reasoning", "type"]
                }
            }
        }
"""

for fpath in ["src/orchestrator.rs", "src/task_manager.rs", "src/neural_agent.rs", "src/executor.rs"]:
    if not os.path.exists(fpath): continue
    with open(fpath, "r") as f:
        text = f.read()
    
    changed = False
    
    # Inject generationConfig schema
    if "responseSchema" not in text and "generationConfig" in text:
        text = re.sub(r'"generationConfig"\s*:\s*\{[^}]*\}', SCHEMA.strip(), text, flags=re.DOTALL)
        changed = True

    # Strip XML tag instructions and map reasoning to JSON
    if "Reasoning: ONLY inside <THOUGHT>" in text:
        text = text.replace("Reasoning: ONLY inside <THOUGHT></THOUGHT> (max 3 sentences)", "Reasoning: Place logic in the 'reasoning' JSON field.")
        text = text.replace("Actions:   ONLY inside <ACTION></ACTION> as a valid JSON array", "Actions: MUST return a raw JSON array of action objects.")
        changed = True

    if changed:
        with open(fpath, "w") as f:
            f.write(text)
        print(f">> [PATCH] Applied Structured Output schema & Prompt Update to {fpath}")

print(">> [PATCH] json_parser.rs gutted and replaced with native serde_json parsing.")
