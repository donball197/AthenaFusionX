#!/usr/bin/env bash
# diagnose_and_finish.sh
#
# Inspect-then-apply for the remaining D / C / E fixes.
# Prints on-disk state first; applies only what is provably missing;
# aborts without writing when a region is ambiguous.
#
# Usage:
#   ./diagnose_and_finish.sh [REPO_ROOT]       # default $HOME/AthenaFusionX
#   SKIP_CHECK=1 ./diagnose_and_finish.sh      # skip cargo check
#   RUN_TESTS=1 ./diagnose_and_finish.sh       # run targeted tests too
set -euo pipefail

[[ $# -le 1 ]] || { echo "usage: $0 [REPO_ROOT]" >&2; exit 2; }
command -v python3 >/dev/null || { echo "ERROR: python3 is required." >&2; exit 2; }

REPO_ROOT="${1:-${REPO_ROOT:-$HOME/AthenaFusionX}}"
[[ -d "$REPO_ROOT" ]] || { echo "ERROR: not a directory: $REPO_ROOT" >&2; exit 2; }
[[ -f "$REPO_ROOT/crates/runtime_core/src/cortical_loop.rs" ]] || {
    echo "ERROR: $REPO_ROOT does not look like the AthenaFusionX root." >&2
    exit 2
}

python3 - "$REPO_ROOT" <<'PYEOF'
import pathlib, shutil, sys, time

root = pathlib.Path(sys.argv[1])
TS = time.strftime("%Y%m%d-%H%M%S")


def load(rel):
    p = root / rel
    if not p.is_file():
        sys.exit("ERROR: target not found: %s" % p)
    return p, p.read_text()


def save(p, text):
    backup = p.with_name(p.name + ".bak.finish." + TS)
    shutil.copy2(p, backup)
    p.write_text(text)
    print("  [write] %s  (backup: %s)" % (p.name, backup.name))


def show(text, needle, label, before=6, after=30):
    lines = text.split("\n")
    hits = [i for i, l in enumerate(lines) if needle in l]
    if not hits:
        print("  [diag] %s: needle not found -> %r" % (label, needle))
        return
    i = hits[0]
    lo, hi = max(0, i - before), min(len(lines), i + after)
    print("  [diag] %s: lines %d..%d" % (label, lo + 1, hi))
    for j in range(lo, hi):
        print("       %s %5d: %s" % (">>" if j == i else "  ", j + 1, lines[j]))


# ---------------------------------------------------------------- D
D_OLD = """        let best_node = match session.graph.select_best_hypothesis().cloned() {
            Some(node) => node,
            None => {
                return Ok(CortexState::Fail {
                    belief: 0.0,
                    evidence: "No viable hypotheses left.".to_string(),
                })
            }
        };"""

D_NEW = """        let best_node = match session.graph.select_best_hypothesis().cloned() {
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
        };"""

cl_path, cl = load("crates/runtime_core/src/cortical_loop.rs")

print("\n=== [D] cortex None-branch (hypothesis regeneration) ===")
has_old = D_OLD in cl
has_new = D_NEW in cl
has_marker = "HYPOTHESIS REGENERATION" in cl
raw_esc = "\x1b" in cl
print("  old anchor: %s | exact new text: %s | D marker: %s | raw ESC bytes: %s"
      % (has_old, has_new, has_marker, raw_esc))

if has_new:
    print("  [skip] D already applied (exact text present).")
    if raw_esc:
        cl = cl.replace("\x1b", "\\x1b")
        save(cl_path, cl)
        print("  [ok]   normalized raw ESC bytes to \\x1b source escapes")
elif has_marker and not has_old:
    # Applied by an earlier run with drifting wording. Do not re-apply;
    # just normalize escapes and trust the marker.
    print("  [skip] D already applied (marker present, wording variant).")
    if raw_esc:
        cl = cl.replace("\x1b", "\\x1b")
        save(cl_path, cl)
        print("  [ok]   normalized raw ESC bytes to \\x1b source escapes")
elif has_old:
    cl = cl.replace(D_OLD, D_NEW)
    save(cl_path, cl)
    print("  [ok]   D applied.")
else:
    show(cl, "select_best_hypothesis().cloned()", "D region")
    sys.exit("ERROR: [D] region matches neither old nor new text; nothing written.")


# ---------------------------------------------------------------- C
cl_path, cl = load("crates/runtime_core/src/cortical_loop.rs")

print("\n=== [C] veto-mandate gate ===")
lines = cl.split("\n")
cands = [(i, l) for i, l in enumerate(lines) if "critic_veto_present" in l and "else if" in l]
print("  [diag] candidate gate lines: %d" % len(cands))
for i, l in cands:
    print("         %d: %s" % (i + 1, l.strip()))

if any('active_strategy.starts_with("patch_function:")' in l for _, l in cands):
    print("  [skip] C already keyed on active_strategy.")
elif len(cands) == 1:
    i, old = cands[0]
    indent = old[:len(old) - len(old.lstrip())]
    new = indent + '} else if active_strategy.starts_with("patch_function:") && critic_veto_present {'
    print("  [old]  %s" % old.strip())
    print("  [new]  %s" % new.strip())
    lines[i] = new
    save(cl_path, "\n".join(lines))
    print("  [ok]   C rewritten.")
else:
    for i, _ in cands:
        lo, hi = max(0, i - 4), min(len(lines), i + 5)
        for j in range(lo, hi):
            print("       %s %5d: %s" % (">>" if j == i else "  ", j + 1, lines[j]))
    sys.exit("ERROR: [C] cannot disambiguate gate line (see diagnostics); nothing written.")


# ---------------------------------------------------------------- E
g_path, g = load("crates/runtime_core/src/cognition_graph.rs")

print("\n=== [E] 0.15 floor inside propagate_beliefs ===")
E_OLD = "                node.belief_score = DAMPING * candidate + (1.0 - DAMPING) * node.belief_score;"
E_NEW = """                node.belief_score = DAMPING * candidate + (1.0 - DAMPING) * node.belief_score;

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
                }"""

if "E (belief floor)" in g:
    print("  [skip] E already applied.")
elif E_OLD in g:
    if g.count(E_OLD) != 1:
        sys.exit("ERROR: [E] anchor matched %dx; nothing written." % g.count(E_OLD))
    g = g.replace(E_OLD, E_NEW)
    save(g_path, g)
    print("  [ok]   E applied.")
else:
    show(g, "DAMPING * candidate", "E region")
    sys.exit("ERROR: [E] anchor not found; nothing written.")

print("\nAll applicable steps applied.")
PYEOF

echo ""
echo "=== Verify markers ==="
grep -n "HYPOTHESIS REGENERATION\|best_belief" \
  "$REPO_ROOT/crates/runtime_core/src/cortical_loop.rs" | head
grep -n 'active_strategy.starts_with("patch_function:") && critic_veto_present' \
  "$REPO_ROOT/crates/runtime_core/src/cortical_loop.rs"
grep -n "before >= 0.15" "$REPO_ROOT/crates/runtime_core/src/cognition_graph.rs"

echo ""
echo "=== Escaping sanity (expect literal backslash-x-1-b, no raw ESC) ==="
grep -c $'\x1b' "$REPO_ROOT/crates/runtime_core/src/cortical_loop.rs" || true

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
