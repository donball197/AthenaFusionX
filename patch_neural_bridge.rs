// ⚡ EVOLUTION: Filtering 'Thought' tokens from 2026 API Streams
// Apply this logic to crates/runtime_core/src/neural_agent.rs

/*
   Inside the while let Some(chunk_result) = stream.next().await loop:
*/

if let Ok(val) = serde_json::from_str::<Value>(clean) {
    if let Some(parts) = val.pointer("/candidates/0/content/parts").and_then(|v| v.as_array()) {
        for part in parts {
            // ⚡ THE 2026 SUTURE: Detect and skip the LLM's internal monologue
            let is_thought = part["thought"].as_bool().unwrap_or(false);
            let has_thinking_key = part.get("thinking").is_some();
            
            if is_thought || has_thinking_key {
                // Log thoughts to telemetry but DO NOT add to the JSON string
                let thought = part["text"].as_str().unwrap_or("");
                let _ = self.tx.send(json!({"type": "NEURAL_THOUGHT", "payload": thought}).to_string());
                continue; 
            }

            if let Some(token) = part["text"].as_str() {
                full_response.push_str(token);
                let _ = self.tx.send(json!({"type": "NEURAL_STREAM", "payload": token}).to_string());
            }
        }
    }
}
