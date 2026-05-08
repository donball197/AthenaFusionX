import os, re

path = "crates/runtime_core/src/neural_agent.rs"
if not os.path.exists(path):
    print("\x1b[31m[FAULT]\x1b[0m crates/runtime_core/src/neural_agent.rs not found.")
    exit(1)

with open(path, "r") as f:
    data = f.read()

# Target the exact comment and block from the grep output
pattern = re.compile(
    r"// ⚡ CRITICAL: Filter out Gemma 4 reasoning/thought parts.*?if let Some\(token\) = part\[\"text\"\]\.as_str\(\)\s*\{.*?\n\s*\}", 
    re.DOTALL
)

suture = """// ⚡ EVOLUTION: Route thoughts to telemetry instead of dropping them
                                let is_thought = part["thought"].as_bool().unwrap_or(false);
                                let has_thinking_key = part.get("thinking").is_some();
                                
                                if is_thought || has_thinking_key {
                                    let thought = part["text"].as_str().unwrap_or("");
                                    let _ = self.tx.send(json!({"type": "NEURAL_THOUGHT", "payload": thought}).to_string());
                                    continue; 
                                }

                                if let Some(token) = part["text"].as_str() {
                                    full_response.push_str(token);
                                    let _ = self.tx.send(json!({"type": "NEURAL_STREAM", "payload": token}).to_string());
                                }"""

if pattern.search(data):
    new_data = pattern.sub(suture, data)
    with open(path, "w") as f:
        f.write(new_data)
    print("\x1b[32m[OK]\x1b[0m Cortical separation perfectly stitched.")
else:
    print("\x1b[31m[FAULT]\x1b[0m Regex failed to match. Check the exact pattern.")
