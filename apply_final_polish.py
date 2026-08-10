import os

print(">> Initiating Final Substrate Polish...")

def patch_file(path, old, new):
    if not os.path.exists(path):
        print(f"[ERROR] {path} not found.")
        return
    with open(path, "r") as f:
        content = f.read()
    if old in content:
        content = content.replace(old, new)
        with open(path, "w") as f:
            f.write(content)
        print(f"[OK] Patched {path}")
    else:
        print(f"[!] Target signature not found in {path}. Skipping.")

# 1. Fix DAG Planner Missing ID Collisions
planner_old = """                let id = item.get("id").and_then(|v| v.as_str()).map(ToString::to_string)
                    .unwrap_or_else(|| {
                        tracing::warn!("Missing task ID in graph, auto-generating fallback: task_{}", idx + 1);
                        format!("task_{}", idx + 1)
                    });"""
planner_new = """                let id = item.get("id").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(ToString::to_string)
                    .unwrap_or_else(|| {
                        let f_id = format!("task_fallback_{}", uuid::Uuid::new_v4().simple());
                        tracing::warn!("Missing task ID in graph, auto-generating unique fallback: {}", f_id);
                        f_id
                    });"""
patch_file("crates/runtime_core/src/orchestrator_v2/planner.rs", planner_old, planner_new)


# 2. Fix Shell-Chaining Command Fractures (Anti-Chaining Prompt)
epistemic_old = "CRITICAL RULE 6: SCRIPT ENVIRONMENT: Any proposed inline scripts (Python/Bash) MUST rely strictly on standard system libraries (e.g., os, sys, json in Python). Do NOT assume third-party packages like 'toml' or 'requests' are installed."
epistemic_new = """CRITICAL RULE 6: SCRIPT ENVIRONMENT: Any proposed inline scripts (Python/Bash) MUST rely strictly on standard system libraries (e.g., os, sys, json in Python). Do NOT assume third-party packages like 'toml' or 'requests' are installed.
CRITICAL RULE 7: ANTI-CHAINING: NEVER chain commands using '&&', '||', or ';'. You MUST execute EXACTLY ONE command per turn. If you need to run multiple commands, propose the first one now, and wait for the evidence before proposing the next."""
patch_file("crates/runtime_core/src/epistemic_engine.rs", epistemic_old, epistemic_new)


# 3. Fix Stale Parser Fallbacks (Ruthless JSON Extraction)
parser_old = """    let trimmed = text.replace("```json", "").replace("```", "");
    let clean_text = sanitize_json_escapes(trimmed.trim());

    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&clean_text) {
        return Ok(actions);
    }"""
parser_new = """    let mut text_clean = text;
    if text_clean.starts_with("```json") {
        text_clean = text_clean.trim_start_matches("```json").trim_start();
    } else if text_clean.starts_with("```") {
        text_clean = text_clean.trim_start_matches("```").trim_start();
    }
    text_clean = text_clean.trim_end_matches("```").trim_end();

    let json_str = if let (Some(start), Some(end)) = (text_clean.find('['), text_clean.rfind(']')) {
        &text_clean[start..=end]
    } else if let (Some(start), Some(end)) = (text_clean.find('{'), text_clean.rfind('}')) {
        &text_clean[start..=end]
    } else {
        text_clean
    };

    let clean_text = sanitize_json_escapes(json_str);

    if let Ok(actions) = serde_json::from_str::<Vec<AgentAction>>(&clean_text) {
        return Ok(actions);
    }"""
patch_file("crates/runtime_core/src/json_parser.rs", parser_old, parser_new)

print(">> Polish Complete.")
