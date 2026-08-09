import os

print(">> Upgrading Subgoal Completion Engine...")

path = "crates/runtime_core/src/completion_engine.rs"
if os.path.exists(path):
    with open(path, "r") as f:
        content = f.read()

    old_func = """    pub fn infer_contract(node_id: &str, objective: &str) -> SubgoalContract {
        let obj_lower = objective.to_lowercase();
        let mut conditions = Vec::new();
        let mut is_mutation_task = false;

        if obj_lower.contains("write") || obj_lower.contains("export") || obj_lower.contains("save") || obj_lower.contains("output") {
            conditions.push(CompletionCondition::WriteMarkerAcquired);
            is_mutation_task = true;
        } else if obj_lower.contains("patch") || obj_lower.contains("apply") || obj_lower.contains("fix") || obj_lower.contains("remediate") {
            conditions.push(CompletionCondition::PatchMarkerAcquired);
            is_mutation_task = true;
        } else if obj_lower.contains("search") || obj_lower.contains("find") || obj_lower.contains("locate") {
            conditions.push(CompletionCondition::SearchMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        } else if obj_lower.contains("read") || obj_lower.contains("inspect") || obj_lower.contains("audit") {
            conditions.push(CompletionCondition::ReadMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        } else {
            conditions.push(CompletionCondition::CommandMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        }

        SubgoalContract {
            node_id: node_id.to_string(),
            objective: objective.to_string(),
            conditions,
            is_mutation_task,
        }
    }"""

    new_func = """    pub fn infer_contract(node_id: &str, objective: &str) -> SubgoalContract {
        let obj_lower = objective.trim().to_lowercase();
        let mut conditions = Vec::new();
        let mut is_mutation_task = false;

        // ⚡ SAFETY FIX: Prevent Reconnaissance Trap
        // If the objective starts with a recon verb, do not apply a mutation lock,
        // even if the word 'fix' or 'patch' appears later in the sentence.
        let is_recon_intent = obj_lower.starts_with("read") 
            || obj_lower.starts_with("analyze") 
            || obj_lower.starts_with("inspect") 
            || obj_lower.starts_with("search") 
            || obj_lower.starts_with("examine") 
            || obj_lower.starts_with("locate");

        if is_recon_intent {
            conditions.push(CompletionCondition::ReadMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        } else if obj_lower.contains("write") || obj_lower.contains("export") || obj_lower.contains("save") || obj_lower.contains("output") {
            conditions.push(CompletionCondition::WriteMarkerAcquired);
            is_mutation_task = true;
        } else if obj_lower.contains("patch") || obj_lower.contains("apply") || obj_lower.contains("fix") || obj_lower.contains("remediate") {
            conditions.push(CompletionCondition::PatchMarkerAcquired);
            is_mutation_task = true;
        } else if obj_lower.contains("search") || obj_lower.contains("find") || obj_lower.contains("locate") {
            conditions.push(CompletionCondition::SearchMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        } else if obj_lower.contains("read") || obj_lower.contains("inspect") || obj_lower.contains("audit") {
            conditions.push(CompletionCondition::ReadMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        } else {
            conditions.push(CompletionCondition::CommandMarkerAcquired);
            conditions.push(CompletionCondition::MinBeliefScore(0.65));
        }

        SubgoalContract {
            node_id: node_id.to_string(),
            objective: objective.to_string(),
            conditions,
            is_mutation_task,
        }
    }"""

    if old_func in content:
        content = content.replace(old_func, new_func)
        with open(path, "w") as f:
            f.write(content)
        print("[OK] CompletionEngine Patched: Intent-Aware Contract Inference Active.")
    else:
        print("[!] Target function signature not found. May already be patched.")
else:
    print(f"[ERROR] File not found: {path}")

