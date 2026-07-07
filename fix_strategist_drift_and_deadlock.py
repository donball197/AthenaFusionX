import sys

FILES = {}

FILES["crates/runtime_core/src/epistemic_engine.rs"] = {
    "anchor": '''    pub async fn propose_action(client: &Client, api_key: &str, strategy: &str, evidence_so_far: &str) -> Result<String> {
        let prompt = format!("You are the Action Strategist.\\nSTRATEGY: {}\\nEVIDENCE GATHERED SO FAR:\\n{}\\nPropose the exact next step to fulfill the strategy. CRITICAL RULE: NEVER run `find /` or search the root directory! Only search the current working directory (`.`). OUTPUT ONLY YOUR PROPOSED NEXT STEP IN PLAIN TEXT.", strategy, evidence_so_far);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.2, "maxOutputTokens": 1024 }});
        Self::safe_api_call(client, api_key, &get_light_models(), &payload).await
    }''',
    "replacement": '''    pub async fn propose_action(client: &Client, api_key: &str, mission: &str, strategy: &str, evidence_so_far: &str) -> Result<String> {
        let prompt = format!("You are the Action Strategist.\\nORIGINAL DIRECTIVE (stay strictly within its scope -- do not investigate other files unless the directive or evidence explicitly points there): {}\\nSTRATEGY: {}\\nEVIDENCE GATHERED SO FAR:\\n{}\\nPropose the exact next step to fulfill the strategy. CRITICAL RULE: NEVER run `find /` or search the root directory! Only search the current working directory (`.`). OUTPUT ONLY YOUR PROPOSED NEXT STEP IN PLAIN TEXT.", mission, strategy, evidence_so_far);
        let payload = serde_json::json!({"contents": [{"role": "user", "parts": [{"text": prompt}]}], "generationConfig": { "temperature": 0.2, "maxOutputTokens": 1024 }});
        Self::safe_api_call(client, api_key, &get_light_models(), &payload).await
    }''',
}

FILES["crates/runtime_core/src/cortical_loop.rs"] = {
    "anchor": '''use crate::executor::RoboticExecutor;
use crate::agent_protocol::AgentAction;''',
    "replacement": '''use crate::executor::RoboticExecutor;
use crate::agent_protocol::AgentAction;
use crate::logic_engine::LogicEngine;''',
}

FILES["crates/runtime_core/src/cortical_loop.rs"]["anchor_2"] = '''        let mut current_best_belief = 0.0;

        for turn in 1..=15 {'''
FILES["crates/runtime_core/src/cortical_loop.rs"]["replacement_2"] = '''        let mut current_best_belief = 0.0;
        let mut observation_log: Vec<String> = Vec::new();

        for turn in 1..=15 {'''

FILES["crates/runtime_core/src/cortical_loop.rs"]["anchor_3"] = '''                // \u26a1 DEFENSE IN DEPTH 2: Hard bound the accumulated evidence history
                if evidence_history.len() > 16000 {
                    let start = evidence_history.len() - 16000;
                    evidence_history = format!("...[EVIDENCE HISTORY TRUNCATED]...
{}", &evidence_history[start..]);
                }
                
                // \u26a1 DEFENSE IN DEPTH 2: Hard bound the accumulated evidence history
                if evidence_history.len() > 16000 {
                    let start = evidence_history.len() - 16000;
                    evidence_history = format!("...[EVIDENCE HISTORY TRUNCATED]...
{}", &evidence_history[start..]);
                }
                
                if evidence_history.is_empty() { evidence_history = "None yet.".to_string(); }

                println!("\\x1b[36m>> [STRATEGIST] Planning next maneuver...\\x1b[0m");
                let proposed_action = match EpistemicEngine::propose_action(&self.client, &self.api_key, &h_content, &evidence_history).await {'''
FILES["crates/runtime_core/src/cortical_loop.rs"]["replacement_3"] = '''                // DEFENSE IN DEPTH 2: Hard bound the accumulated evidence history
                if evidence_history.len() > 16000 {
                    let start = evidence_history.len() - 16000;
                    evidence_history = format!("...[EVIDENCE HISTORY TRUNCATED]...
{}", &evidence_history[start..]);
                }

                if evidence_history.is_empty() { evidence_history = "None yet.".to_string(); }

                if LogicEngine::detect_deadlock(&observation_log) {
                    println!("\\x1b[90m>> [CORTEX] Deadlock detected -- repeated action with zero progress. Forcing strategy pivot.\\x1b[0m");
                    self.graph.add_edge(h_id, h_id, EdgeRelation::Contradicts, 0.4);
                    self.graph.propagate_beliefs();
                    continue;
                }

                println!("\\x1b[36m>> [STRATEGIST] Planning next maneuver...\\x1b[0m");
                let proposed_action = match EpistemicEngine::propose_action(&self.client, &self.api_key, mission, &h_content, &evidence_history).await {'''

FILES["crates/runtime_core/src/cortical_loop.rs"]["anchor_4"] = '''                                    let preview = evidence.lines().take(5).collect::<Vec<_>>().join("\\n");
                                    println!("\\x1b[32m>> [EVIDENCE ACQUIRED]\\x1b[0m\\n{}", preview);'''
FILES["crates/runtime_core/src/cortical_loop.rs"]["replacement_4"] = '''                                    observation_log.push(evidence.clone());
                                    let preview = evidence.lines().take(5).collect::<Vec<_>>().join("\\n");
                                    println!("\\x1b[32m>> [EVIDENCE ACQUIRED]\\x1b[0m\\n{}", preview);'''


def check_anchor(path, key, anchor):
    try:
        with open(path, "r", encoding="utf-8") as f:
            content = f.read()
    except FileNotFoundError:
        print("FATAL: " + path + " not found.", file=sys.stderr)
        sys.exit(1)
    count = content.count(anchor)
    if count == 0:
        print("FATAL: anchor '" + key + "' not found in " + path + ".", file=sys.stderr)
        sys.exit(1)
    if count > 1:
        print("FATAL: anchor '" + key + "' matched " + str(count) + " times in " + path + ", expected 1.", file=sys.stderr)
        sys.exit(1)
    return content


def main():
    loaded = {}
    for path, spec in FILES.items():
        content = check_anchor(path, "primary", spec["anchor"])
        for extra in ("anchor_2", "anchor_3", "anchor_4"):
            if extra in spec:
                content = check_anchor(path, extra, spec[extra])
        loaded[path] = content

    for path, spec in FILES.items():
        content = loaded[path]
        content = content.replace(spec["anchor"], spec["replacement"])
        for extra, rep in (("anchor_2", "replacement_2"), ("anchor_3", "replacement_3"), ("anchor_4", "replacement_4")):
            if extra in spec:
                content = content.replace(spec[extra], spec[rep])
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
        print("OK: patched " + path)

    print()
    print("Next: cargo check -p nexus_runtime_core -j 2")


if __name__ == "__main__":
    main()
