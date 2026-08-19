import os

def patch_file(path, old_text, new_text, description):
    try:
        with open(path, "r") as f:
            data = f.read()
        if old_text in data:
            with open(path, "w") as f:
                f.write(data.replace(old_text, new_text))
            print(f"[OK] {description}")
        else:
            print(f"[WARN] Target text not found for: {description}")
    except Exception as e:
        print(f"[ERROR] Could not patch {path}: {e}")

# 1. FIX V2 PLANNER SABOTAGE
patch_file(
    "crates/runtime_core/src/orchestrator_v2/planner.rs",
    """"CRITICAL RULE [GRANULARITY]: YOU MUST BREAK COMPLEX DIRECTIVES INTO AT LEAST 3 DISTINCT NODES. A single node 'task_1' is a FAILURE. If auditing or analyzing, create separate nodes for 'discovery', 'analysis', and 'reporting'. If modifying files, create one node per file.""",
    """"CRITICAL RULE [GRANULARITY]: Create exactly as many nodes as necessary. If the directive is to write or generate a single cohesive document, DO NOT split it into multiple nodes; handle it in a single node to maintain context. Only decompose into multiple nodes if the task requires distinct, sequential phases (e.g., discover, then analyze, then write).""",
    "Fixed Orchestrator V2 Planner Granularity Rule"
)

# 2. FIX OUTCOME VERIFIER POISON PILL
patch_file(
    "crates/runtime_core/src/outcome_verifier.rs",
    """        // Fallback for non-mutation writes
        if local_evidence.contains("[WRITE_OK]") || local_evidence.contains("[CMD_OK]") {
            CompletionStatus::Approved
        } else {""",
    """        // Fallback for non-mutation writes
        if local_evidence.contains("[WRITE_OK]") || local_evidence.contains("[DIFF_PATCH_OK]") || local_evidence.contains("[PATCH_FUNCTION_OK]") {
            CompletionStatus::Approved
        } else {""",
    "Removed [CMD_OK] bypass in Outcome Verifier"
)

# 3. FIX OMNI-GUARD POISON PILL (In two places: OMNI-GUARD and HALT check)
loop_path = "crates/runtime_core/src/cortical_loop.rs"
with open(loop_path, "r") as f: loop_data = f.read()

loop_data = loop_data.replace(
    """                let has_prior_evidence = if is_write_task {
                    session.local_evidence.lines().any(|l| l.starts_with("[WRITE_OK]") || l.starts_with("[CMD_OK]"))
                } else if is_mutation_task {""",
    """                let has_prior_evidence = if is_write_task {
                    session.local_evidence.lines().any(|l| l.starts_with("[WRITE_OK]"))
                } else if is_mutation_task {"""
)

loop_data = loop_data.replace(
    """                    let has_prior_evidence = if is_write_task {
                        session
                            .local_evidence
                            .lines()
                            .any(|l| l.starts_with("[WRITE_OK]") || l.starts_with("[CMD_OK]"))
                    } else if is_mutation_task {""",
    """                    let has_prior_evidence = if is_write_task {
                        session
                            .local_evidence
                            .lines()
                            .any(|l| l.starts_with("[WRITE_OK]"))
                    } else if is_mutation_task {"""
)
with open(loop_path, "w") as f: f.write(loop_data)
print("[OK] Removed [CMD_OK] bypasses in Cortical Loop Guards")

# 4. FIX CORTICAL LOOP ESCAPE HATCHES (Anti-loop and Exhaustion)
patch_file(
    "crates/runtime_core/src/cortical_loop.rs",
    """                if session.is_action_attempted(&action_key) {
                    if session.is_subgoal_satisfied(current_belief)
                        || (!session.local_evidence.trim().is_empty() && current_belief >= 0.60)
                    {
                        self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Loop detected but evidence is sufficient. Halting.\x1b[0m".to_string());
                        return Ok(CortexOutcome::CompletionProposed {""",
    """                if session.is_action_attempted(&action_key) {
                    if session.is_subgoal_satisfied(current_belief) {
                        self.log("\x1b[1;36m>> [SUBGOAL_COMPLETE] Loop detected but contract is mathematically satisfied. Halting.\x1b[0m".to_string());
                        return Ok(CortexOutcome::CompletionProposed {""",
    "Closed belief >= 0.60 anti-loop escape hatch"
)

patch_file(
    "crates/runtime_core/src/cortical_loop.rs",
    """        Ok(CortexOutcome::CompletionProposed {
            belief: session.max_observed_belief.max(0.70),
            evidence: final_evidence,
        })
    }""",
    """        self.log("\x1b[1;31m>> [CORTEX FAULT] Task exhausted budget without satisfying contract conditions.\x1b[0m".to_string());
        Ok(CortexOutcome::TurnsExhausted {
            best_belief: session.max_observed_belief,
            evidence: final_evidence,
        })
    }""",
    "Closed turns exhausted hallucination"
)

# 5. FIX COMPLETION ENGINE (Enforce dead contracts)
patch_file(
    "crates/runtime_core/src/completion_engine.rs",
    """    pub fn evaluate(contract: &SubgoalContract, local_evidence: &str, _belief_score: f32) -> bool {
        // ⚡ DETERMINISTIC VERIFIER INTERFACE
        let verifier_result = crate::outcome_verifier::OutcomeVerifier::verify(&contract.node_id, local_evidence);
        matches!(verifier_result, crate::outcome_verifier::CompletionStatus::Approved)
    }""",
    """    pub fn evaluate(contract: &SubgoalContract, local_evidence: &str, belief_score: f32) -> bool {
        // ⚡ FIX: Actually enforce the contract conditions inferred earlier.
        for condition in &contract.conditions {
            match condition {
                CompletionCondition::MinBeliefScore(min_score) => {
                    if belief_score < *min_score { return false; }
                }
                CompletionCondition::WriteMarkerAcquired => {
                    if !local_evidence.contains("[WRITE_OK]") { return false; }
                }
                CompletionCondition::PatchMarkerAcquired => {
                    if !local_evidence.contains("[PATCH_FUNCTION_OK]") && !local_evidence.contains("[DIFF_PATCH_OK]") && !local_evidence.contains("[REPLACE_LINES_OK]") { return false; }
                }
                CompletionCondition::ReadMarkerAcquired => {
                    if !local_evidence.contains("[READ_OK]") && !local_evidence.contains("[READ_CHUNK_OK]") { return false; }
                }
                CompletionCondition::SearchMarkerAcquired => {
                    if !local_evidence.contains("[SEARCH_CODE_OK]") && !local_evidence.contains("[SEARCH_OK]") { return false; }
                }
                _ => {} 
            }
        }
        let verifier_result = crate::outcome_verifier::OutcomeVerifier::verify(&contract.node_id, local_evidence);
        matches!(verifier_result, crate::outcome_verifier::CompletionStatus::Approved)
    }""",
    "Enforced dead SubgoalContracts in Completion Engine"
)

# 6. FIX EXECUTOR PATH TRAVERSAL
patch_file(
    "crates/runtime_core/src/executor.rs",
    """                MotorAction::WriteFile { path, content } => {
                    // WriteFile intentionally does NOT call sanitize() here.
                    // A write target legitimately not existing yet is the
                    // normal case (creating a new file), not an error --
                    // sanitize() is built around "resolve an existing
                    // path or fail/fallback", which is wrong for this one
                    // action type. Resolve the parent directory directly
                    // instead, workspace-root-relative or absolute, and
                    // create parent dirs as needed.
                    let raw_path = Path::new(&path);
                    let workspace_root = std::env::current_dir()
                        .and_then(std::fs::canonicalize)
                        .map_err(|e| anyhow!("PATH_TRAVERSAL_DENIED: could not resolve workspace root ({})", e))?;
                    let safe_path = if raw_path.is_absolute() {
                        raw_path.to_path_buf()
                    } else {
                        workspace_root.join(raw_path)
                    };""",
    """                MotorAction::WriteFile { path, content } => {
                    let raw_path = Path::new(&path);
                    if raw_path.is_absolute() {
                        return Err(anyhow!("PATH_TRAVERSAL_DENIED: Absolute paths are forbidden. Use paths relative to the workspace."));
                    }
                    let workspace_root = std::env::current_dir()
                        .and_then(std::fs::canonicalize)
                        .map_err(|e| anyhow!("PATH_TRAVERSAL_DENIED: could not resolve workspace root ({})", e))?;
                    let safe_path = workspace_root.join(raw_path);
                    if !safe_path.starts_with(&workspace_root) {
                        return Err(anyhow!("PATH_TRAVERSAL_DENIED: {} resolves outside the workspace root", path));
                    }""",
    "Closed WriteFile absolute path traversal vulnerability"
)
