import os, re

path = "crates/runtime_core/src/neural_agent.rs"
if not os.path.exists(path):
    print("\x1b[31m[FAULT]\x1b[0m crates/runtime_core/src/neural_agent.rs not found.")
    exit(1)

with open(path, "r") as f:
    data = f.read()

# Target the specific candidates array loop
pattern = re.compile(
    r"if let Some\(parts\) = val\.pointer\(\"/candidates/0/content/parts\"\)\.and_then\(\|v\| v\.as_array\(\)\)\s*\{.*?for part in parts\s*\{.*?\n\s*\}\n\s*\}", 
    re.DOTALL
)

suture = """if let Some(parts) = val.pointer("/candidates/0/content/parts").and_then(|v| v.as_array()) {
            for part in parts {
                // THE 2026 SUTURE: Detect and skip the LLM internal monologue
                let is_thought = part["thought"].as_bool().unwrap_or(false);
                let has_thinking_key = part.get("thinking").is_some();
                
                if is_thought || has_thinking_key {
                    // Log thoughts to telemetry but DO NOT add to the JSON string
                    let thought = part["text"].as_str().unwrap_or("");
                    let _ = self.tx.send(serde_json::json!({"type": "NEURAL_THOUGHT", "payload": thought}).to_string());
                    continue; 
                }

                if let Some(token) = part["text"].as_str() {
                    full_response.push_str(token);
                    let _ = self.tx.send(serde_json::json!({"type": "NEURAL_STREAM", "payload": token}).to_string());
                }
            }
        }"""

if pattern.search(data):
    with open(path, "w") as f:
        f.write(pattern.sub(suture, data))
    print("\x1b[32m[OK]\x1b[0m Cortical separation stitched successfully.")
else:
    print("\x1b[31m[FAULT]\x1b[0m Could not locate the target JSON parsing loop. Manual injection required.")
