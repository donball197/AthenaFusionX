#!/usr/bin/env bash
# apply_remaining_fixes.sh
#
# Continuation after apply_orchestration_fixes.sh halted at the C step.
#   [1] cortical_loop.rs   - D (lost: the aborted run never wrote the file)
#   [2] cortical_loop.rs   - C (flexible gate match + region diagnostics)
#   [3] cognition_graph.rs - E (never reached in the aborted run)
#
# Usage:
#   ./apply_remaining_fixes.sh [REPO_ROOT]      # default $HOME/AthenaFusionX
#   SKIP_CHECK=1 ./apply_remaining_fixes.sh     # skip cargo check
#   RUN_TESTS=1 ./apply_remaining_fixes.sh      # run targeted tests too
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

    def write(self):
        if not self.changed:
            print(f"  [info] {self.path.name}: no changes to flush")
            return
        backup = self.path.with_name(self.path.name + f".bak.remaining.{TS}")
        shutil.copy2(self.path, backup)
        self.path.write_text(self.text)
        self.changed = False
        print(f"  [write] {self.path.name}  (backup: {backup.name})")


# ---------------------------------------------------------------- [1] D
print("\n=== [1/3] cortical_loop.rs: D, hypothesis regeneration + honest belief ===")
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
cl.write()   # flush immediately: an edit must never be lost to a later step


# ---------------------------------------------------------------- [2] C
print("\n=== [2/3] cortical_loop.rs: C, veto-mandate gate on post-recovery strategy ===")
cl = File("crates/runtime_core/src/cortical_loop.rs")   # re-read flushed state
lines = cl.text.split("\n")

# Diagnostics first: show every candidate gate line with context.
candidates = [
    (i, l) for i, l in enumerate(lines)
    if "critic_veto_present" in l and "else if" in l
]
print(f"  [diag] candidate gate lines: {len(candidates)}")
for i, l in candidates:
    print(f"         {i + 1}: {l.strip()}")

if any('active_strategy.starts_with("patch_function:")' in l for _, l in candidates):
    print("  [skip] C: gate already keys on active_strategy")
elif len(candidates) == 1:
    idx, old_line = candidates[0]
    indent = old_line[:len(old_line) - len(old_line.lstrip())]
    new_line = indent + '} else if active_strategy.starts_with("patch_function:") && critic_veto_present {'
    print(f"  [old]  {old_line.strip()}")
    print(f"  [new]  {new_line.strip()}")
    lines[idx] = new_line
    cl.text = "\n".join(lines)
    cl.changed = True
    print("  [ok]   C: mandate gate now uses post-recovery active_strategy")
else:
    print("  [diag] context around candidates:")
    for i, _ in candidates:
        lo, hi = max(0, i - 3), min(len(lines), i + 4)
        for j in range(lo, hi):
            mark = ">>" if j == i else "  "
            print(f"         {mark} {j + 1}: {lines[j]}")
    sys.exit(
        "ERROR: [C] cannot disambiguate the gate line (see diagnostics above). "
        "Nothing written for this step."
    )
cl.write()


# ---------------------------------------------------------------- [3] E
print("\n=== [3/3] cognition_graph.rs: E, 0.15 floor inside propagate_beliefs ===")
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
grep -n "HYPOTHESIS REGENERATION\|best_belief" \
  "$REPO_ROOT/crates/runtime_core/src/cortical_loop.rs" | head
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
