import os

# 1. Ensure epistemic_session.rs has active_mutation_paths
p1 = "crates/runtime_core/src/cognition/epistemic_session.rs"
with open(p1, "r") as f:
    s1 = f.read()

if "active_mutation_paths" not in s1:
    s1 = s1.replace(
        "pub compile_remediation: crate::compile_remediation::CompileRemediationState,",
        "pub compile_remediation: crate::compile_remediation::CompileRemediationState,\n    #[serde(skip)]\n    pub active_mutation_paths: Vec<String>,"
    )
    s1 = s1.replace(
        "compile_remediation: crate::compile_remediation::CompileRemediationState::default(),",
        "compile_remediation: crate::compile_remediation::CompileRemediationState::default(),\n            active_mutation_paths: Vec::new(),"
    )
    with open(p1, "w") as f:
        f.write(s1)
print("[✓] Checked epistemic_session.rs")

# 2. Patch cortical_loop.rs
p2 = "crates/runtime_core/src/cortical_loop.rs"
with open(p2, "r") as f:
    s2 = f.read()

revert_block = """if session.compile_remediation.is_active() {
                            for path in &session.active_mutation_paths {
                                let _ = crate::podman::PodmanShield::execute(&format!("git checkout -- {}", path));
                            }
                            session.compile_remediation.reset();
                            session.active_mutation_paths.clear();
                        }"""

# Bounding error preview (15 lines, <= 1000 chars)
s2 = s2.replace(
    ".take(40)",
    ".take(15)"
)
if "err_preview.chars().take(1000)" not in s2:
    s2 = s2.replace(
        ".join(\"\\n\");\n\n                      let decision = session.compile_remediation.classify(false);",
        ".join(\"\\n\");\n                      let err_preview: String = err_preview.chars().take(1000).collect();\n\n                      let decision = session.compile_remediation.classify(false);"
    )

# RetainForRetry records mutation paths
s2 = s2.replace(
    "CompileDecision::RetainForRetry { iteration } => {",
    "CompileDecision::RetainForRetry { iteration } => {\n                              session.active_mutation_paths = mutation_paths.clone();"
)

# Success & Revert clear mutation paths
s2 = s2.replace(
    "session.compile_remediation.reset();\n\n                      let mut success_evidence =",
    "session.compile_remediation.reset();\n                      session.active_mutation_paths.clear();\n\n                      let mut success_evidence ="
)
s2 = s2.replace(
    "if revert_all_succeeded {\n                                  session.compile_remediation.reset();",
    "if revert_all_succeeded {\n                                  session.compile_remediation.reset();\n                                  session.active_mutation_paths.clear();"
)

# Rollback on CortexState::Fail
s2 = s2.replace(
    "CortexState::Fail { belief, .. } => {",
    "CortexState::Fail { belief, .. } => {\n                      " + revert_block
)

# Rollback on TurnsExhausted (call limit)
call_marker = ">> [SUBGOAL_BUDGET_EXHAUSTED] Cognitive call limit reached."
if call_marker in s2:
    idx = s2.find(call_marker)
    ret_idx = s2.find("return Ok(CortexOutcome::TurnsExhausted", idx)
    if "session.compile_remediation.is_active()" not in s2[idx:ret_idx]:
        s2 = s2[:ret_idx] + revert_block + "\n\n                        " + s2[ret_idx:]

# Rollback on TurnsExhausted (budget exhausted)
budget_marker = "} else if session.budget.is_exhausted(turn) {"
if budget_marker in s2:
    idx = s2.find(budget_marker)
    ret_idx = s2.find("return Ok(CortexOutcome::TurnsExhausted", idx)
    if "session.compile_remediation.is_active()" not in s2[idx:ret_idx]:
        s2 = s2[:ret_idx] + revert_block + "\n\n                " + s2[ret_idx:]

with open(p2, "w") as f:
    f.write(s2)
print("[✓] Checked cortical_loop.rs")
