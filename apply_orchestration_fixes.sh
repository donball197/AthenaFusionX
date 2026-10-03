#!/usr/bin/env bash
# apply_orchestration_fixes.sh
#
# Lands the remaining audit fixes against exact anchors:
#   [1] contract_status.rs  - remove missing-artifact early return (mission fix)
#   [2] algorithms/strategy/src/lib.rs - B: multi-strategy patch targets (3 sites)
#   [3] epistemic_engine.rs - B: update the pinned test expectation
#   [4] cortical_loop.rs    - D: no-selectable-hypothesis recovery + honest belief
#   [5] cognition_graph.rs  - E: 0.15 belief floor inside propagate_beliefs
#   [6] cortical_loop.rs    - C: scope veto-mandate gate to post-recovery strategy
#
# Usage:
#   ./apply_orchestration_fixes.sh [REPO_ROOT]     # default $HOME/AthenaFusionX
#   SKIP_CHECK=1 ./apply_orchestration_fixes.sh    # skip cargo check
#   RUN_TESTS=1 ./apply_orchestration_fixes.sh     # run targeted tests too
#
# Idempotent: re-running skips anything already applied.
set -euo pipefail

[[ $# -le 1 ]] || { echo "usage: $0 [REPO_ROOT]" >&2; exit 2; }
command -v python3 >/dev/null || { echo "ERROR: python3 is required." >&2; exit 2; }

REPO_ROOT="${1:-${REPO_ROOT:-$HOME/AthenaFusionX}}"
[[ -d "$REPO_ROOT" ]] || { echo "ERROR: not a directory: $REPO_ROOT" >&2; exit 2; }

python3 - "$REPO_ROOT" <<'PYEOF'
import pathlib, shutil, sys, time

root = pathlib.Path(sys.argv[1])
TS = time.strftime("%Y%m%d-%H%M%S")

class File:
    def __init__(self, rel):
        self.path = root / rel
        if not self.path.is_file():
            sys.exit(f"ERROR: target not found: {self.path}")
        self.text = self.path.read_text()
        self.changed = False

    def replace_once(self, old, new, label):
        if new in self.text:
            print(f"  [skip] {label} (already applied)")
            return
        n = self.text.count(old)
        if n != 1:
            sys.exit(f"ERROR: [{label}] anchor matched {n}x in {self.path}; nothing written.")
        self.text = self.text.replace(old, new)
        self.changed = True
        print(f"  [ok]   {label}")

    def insert_before(self, anchor, block, label):
        if block.strip() in self.text:
            print(f"  [skip] {label} (already applied)")
            return
        n = self.text.count(anchor)
        if n != 1:
            sys.exit(f"ERROR: [{label}] anchor matched {n}x in {self.path}; nothing written.")
        self.text = self.text.replace(anchor, block + anchor)
        self.changed = True
        print(f"  [ok]   {label}")

    def write(self):
        if not self.changed:
            return
        backup = self.path.with_name(self.path.name + f".bak.fixes.{TS}")
        shutil.copy2(self.path, backup)
        self.path.write_text(self.text)
        print(f"  [write] {self.path.name}  (backup: {backup.name})")


# ---------------------------------------------------------------- [1]
print("\n=== [1/6] contract_status.rs: missing artifacts must not early-return ===")
c = File("crates/runtime_core/src/contract_status.rs")

c.replace_once(
"""        if !missing.is_empty() {
            return ContractStatus::NoEvidence;
        }

        let has_mutation = has_mutation_marker(local_evidence);""",
"""        // REGRESSION FIX (missing-artifact early return): missing artifacts
        // must NOT unconditionally classify NoEvidence. They guarantee
        // contract_satisfied == false below, so the flow reaches the final
        // block and correctly returns PartialEvidence when mutation or recon
        // markers are present.
        let has_mutation = has_mutation_marker(local_evidence);""",
    "delete unconditional NoEvidence early return")

c.replace_once(
"""        let contract_satisfied = crate::completion_engine::SubgoalCompletionEngine::evaluate(
            &mission_contract,
            local_evidence,
            current_belief,
        );""",
"""        // Missing artifacts can never satisfy the contract, but must not
        // short-circuit the classification.
        let contract_satisfied = missing.is_empty()
            && crate::completion_engine::SubgoalCompletionEngine::evaluate(
                &mission_contract,
                local_evidence,
                current_belief,
            );""",
    "fold missing.is_empty() into contract_satisfied")
c.write()


# ---------------------------------------------------------------- [2]
print("\n=== [2/6] algorithms/strategy/src/lib.rs: B, multi-strategy patch targets ===")
s = File("algorithms/strategy/src/lib.rs")

HELPER = """    /// B (multi-strategy patch targets): a patch target must never lock the
    /// cognition graph into a single hypothesis. Index 0 keeps the original
    /// patch_function strategy (and therefore the deterministic 0.51 turn-1
    /// tiebreak); indices 1.. are competing strategies that keep the node
    /// alive when the patch target proves unaddressable.
    fn patch_strategies(path: &str, function: &str) -> Vec<String> {
        vec![
            format!("patch_function:{}:{}", path, function),
            format!("read_file:{}", path),
            format!("replace_lines:{}", path),
        ]
    }

    /// Split a compound `path:function` target (PATCH_FUNCTION_OK form).
    fn patch_strategies_from_target(target: &str) -> Vec<String> {
        let mut parts = target.splitn(2, ':');
        let path = parts.next().unwrap_or("").trim();
        let function = parts.next().unwrap_or("").trim();

        if function.is_empty() {
            vec![format!("patch_function:{}", target)]
        } else {
            Self::patch_strategies(path, function)
        }
    }

"""
s.insert_before(
    "    fn extract_patch_target(problem: &str, problem_lower: &str) -> Option<(String, String)> {",
    HELPER,
    "insert patch_strategies helper")

# site 1: extract_patch_target branch
s.replace_once(
    '            return Some(vec![format!("patch_function:{}:{}", path, function)]);',
    "            return Some(Self::patch_strategies(&path, &function));",
    "site 1: extract_patch_target branch")
# site 2a: PATCH_FUNCTION_OK with compound target
s.replace_once(
    '                    return Some(vec![format!("patch_function:{}", target)]);',
    "                    return Some(Self::patch_strategies_from_target(target));",
    "site 2a: PATCH_FUNCTION_OK compound target")
# site 2b: PATCH_FUNCTION_OK with synthesized path + bare function
s.replace_once(
    '                    return Some(vec![format!("patch_function:{}:{}", path, target)]);',
    "                    return Some(Self::patch_strategies(path, target));",
    "site 2b: PATCH_FUNCTION_OK synthesized path")
# site 3: legacy imperative fallback
s.replace_once(
    '                    return Some(vec![format!("patch_function:{}:{}", path, func)]);',
    "                    return Some(Self::patch_strategies(path, func));",
    "site 3: legacy patch_function fallback")
s.write()


# ---------------------------------------------------------------- [3]
print("\n=== [3/6] epistemic_engine.rs: update pinned test expectation ===")
t = File("crates/runtime_core/src/epistemic_engine.rs")
t.replace_once(
    '        assert_eq!(strategies, vec!["patch_function:apps/math_utils/src/lib.rs:add".to_string()]);',
    '        assert_eq!(strategies, vec!["patch_function:apps/math_utils/src/lib.rs:add".to_string(), '
    '"read_file:apps/math_utils/src/lib.rs".to_string(), '
    '"replace_lines:apps/math_utils/src/lib.rs".to_string()]);',
    "compound_patch_mission now expects 3 competing strategies")
t.write()


# ---------------------------------------------------------------- [4] + [6]
print("\n=== [4/6] cortical_loop.rs: D, hypothesis regeneration + honest belief ===")
cl = File("crates/runtime_core/src/cortical_loop.rs")

cl.replace_once(
"""        let best_node = match session.graph.select_best_hypothesis().cloned() {
            Some(node) => node,
            None => {
                return Ok(CortexState::Fail {
                    belief: 0.0,
                    evidence: "No viable hypotheses left.".to_string(),
                })
            }
        };""",
"""        let best_node = match session.graph.select_best_hypothesis().cloned() {
            Some(node) => node,
            None => {
                // D (no-selectable-hypothesis recovery): a node must not die
                // merely because every hypothesis decayed below the 0.15
                // selection floor. Log the condition (it was previously a
                // silent turn), then attempt ONE regeneration pass. Content
                // identical to any existing hypothesis -- including the dead
                // one that just fell below the floor -- is skipped, so this
                // can never resurrect a poisoned strategy.
                self.log(
                    "\\x1b[1;31m>> [CORTEX] No selectable hypothesis (threshold 0.15). Attempting one regeneration.\\x1b[0m"
                        .to_string(),
                );
                session.budget.record_call();
                if let Ok(hyps) = EpistemicEngine::generate_hypotheses(
                    &self.client,
                    &self.api_key,
                    mission,
                    world_context,
                )
                .await
                {
                    for h in hyps {
                        let already = session.graph.nodes.values().any(|n| n.content == h);
                        if already {
                            continue;
                        }
                        let hid = session.graph.add_node(NodeKind::Hypothesis, &h, 0.51, 100);
                        session.best_hypothesis_id = Some(hid);
                        self.log(format!(
                            "\\x1b[1;36m>> [CORTEX HYPOTHESIS REGENERATION]\\x1b[0m Injected `{}`.",
                            h
                        ));
                        return Ok(CortexState::Strategize);
                    }
                }
                self.log(
                    "\\x1b[1;31m>> [CORTEX] Regeneration produced no new hypotheses; failing node.\\x1b[0m"
                        .to_string(),
                );
                // Report the real best surviving belief instead of a
                // hardcoded 0.0: DAG telemetry previously printed
                // "best belief 0.00" as a literal, not a measurement.
                let best_belief = session
                    .graph
                    .nodes
                    .values()
                    .filter(|n| matches!(n.kind, NodeKind::Hypothesis))
                    .map(|n| n.belief_score)
                    .fold(0.0_f32, f32::max);
                return Ok(CortexState::Fail {
                    belief: best_belief,
                    evidence: "No viable hypotheses left.".to_string(),
                })
            }
        };""",
    "D: None branch regenerates once, reports real belief")

print("\n=== [6/6] cortical_loop.rs: C, veto-mandate gate on post-recovery strategy ===")
cl.replace_once(
    "        } else if patch_function_strategy && critic_veto_present {",
    '        } else if active_strategy.starts_with("patch_function:") && critic_veto_present {',
    "C: mandate skipped when AST-UNADDRESSABLE recovery pivoted to read_file")
cl.write()


# ---------------------------------------------------------------- [5]
print("\n=== [5/6] cognition_graph.rs: E, 0.15 floor inside propagate_beliefs ===")
g = File("crates/runtime_core/src/cognition_graph.rs")
g.replace_once(
    "                node.belief_score = DAMPING * candidate + (1.0 - DAMPING) * node.belief_score;",
    """                node.belief_score = DAMPING * candidate + (1.0 - DAMPING) * node.belief_score;

                // E (belief floor): the 0.15 selection floor must hold at the
                // graph boundary. propagate_beliefs is the one chokepoint that
                // can drop a selectable hypothesis below the floor (a single
                // 0.40 Contradicts edge: 0.15 -> 0.1075 -> fixed point 0.10),
                // and the failure paths call it without going through
                // floor_hypothesis_belief_after_failure. Clamp only hypotheses
                // that were selectable BEFORE this pass: an explicit demotion
                // to 0.05 (e.g. HALT-VETO recovery) must stay dead.
                if matches!(node.kind, NodeKind::Hypothesis)
                    && before >= 0.15
                    && node.belief_score < 0.15
                {
                    node.belief_score = 0.15;
                }""",
    "E: floor hypothesis belief at 0.15 after propagation")
g.write()

print("\nAll steps applied.")
PYEOF

echo ""
echo "=== Verify markers ==="
grep -n "missing.is_empty()" "$REPO_ROOT/crates/runtime_core/src/contract_status.rs"
grep -n "patch_strategies" "$REPO_ROOT/algorithms/strategy/src/lib.rs" | head
grep -n "HYPOTHESIS REGENERATION\|best_belief" "$REPO_ROOT/crates/runtime_core/src/cortical_loop.rs" | head
grep -n 'active_strategy.starts_with("patch_function:") && critic_veto_present' \
  "$REPO_ROOT/crates/runtime_core/src/cortical_loop.rs"
grep -n "before >= 0.15" "$REPO_ROOT/crates/runtime_core/src/cognition_graph.rs"

if [[ "${SKIP_CHECK:-0}" == "1" ]]; then
    echo "SKIP_CHECK=1 -> cargo check skipped."
else
    echo ""
    echo "=== Compile check ==="
    ( cd "$REPO_ROOT" && cargo check -p nexus_runtime_core )
fi

if [[ "${RUN_TESTS:-0}" == "1" ]]; then
    echo ""
    echo "=== Targeted tests ==="
    ( cd "$REPO_ROOT" && \
      cargo test -p nexus_strategy_algorithms && \
      cargo test -p nexus_runtime_core compound_patch_mission && \
      cargo test -p nexus_runtime_core --lib cognition_graph )
fi
