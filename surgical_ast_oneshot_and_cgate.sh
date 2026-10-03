#!/usr/bin/env bash
# surgical_ast_oneshot_and_cgate.sh
set -euo pipefail

ROOT="${1:-$HOME/AthenaFusionX}"
TARGET="$ROOT/crates/runtime_core/src/cortical_loop.rs"

[[ -f "$TARGET" ]] || { echo "ERROR: $TARGET not found" >&2; exit 1; }

if grep -q 'AST_UNADDRESSABLE_PIVOTED' "$TARGET"; then
  echo "ALREADY APPLIED: one-shot marker present"
  exit 0
fi

python3 - "$TARGET" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()

# --- 1) One-shot guard inside the existing recovery block ---
old_inject = '''            if let Some(path) = path {
                let recon = format!("read_file:{}", path);
                let hid = session
                    .graph
                    .nodes
                    .iter()
                    .find(|(_, n)| n.content == recon)
                    .map(|(id, _)| *id)
                    .unwrap_or_else(|| {
                        session.graph.add_node(
                            NodeKind::Hypothesis,
                            &recon,
                            0.58,
                            100,
                        )
                    });
                session.best_hypothesis_id = Some(hid);
                active_strategy = recon;
                self.log(
                    "\\x1b[1;36m>> [CORTEX AST-UNADDRESSABLE RECOVERY]\\x1b[0m \\
                     Symbol has no Tree-sitter body; pivoting to read_file."
                        .to_string(),
                );
            } else {'''

new_inject = '''            if let Some(path) = path {
                let pivot_marker = format!("[AST_UNADDRESSABLE_PIVOTED]:{}", path);
                let already_pivoted = session.local_evidence.contains(&pivot_marker);

                let recon = format!("read_file:{}", path);
                let hid = session
                    .graph
                    .nodes
                    .iter()
                    .find(|(_, n)| n.content == recon)
                    .map(|(id, _)| *id)
                    .unwrap_or_else(|| {
                        session.graph.add_node(
                            NodeKind::Hypothesis,
                            &recon,
                            0.58,
                            100,
                        )
                    });

                // Always keep strategy/hypothesis coherent, but only demote/log once.
                session.best_hypothesis_id = Some(hid);
                active_strategy = recon;

                if !already_pivoted {
                    session.local_evidence.push_str("\\n");
                    session.local_evidence.push_str(&pivot_marker);
                    session.local_evidence.push('\\n');
                    self.log(
                        "\\x1b[1;36m>> [CORTEX AST-UNADDRESSABLE RECOVERY]\\x1b[0m \\
                         Symbol has no Tree-sitter body; pivoting to read_file."
                            .to_string(),
                    );
                }
            } else {'''

if old_inject not in text:
    print("ERROR: inject anchor not found (whitespace drift?)", file=sys.stderr)
    sys.exit(2)

text = text.replace(old_inject, new_inject, 1)

# --- 2) Gate the critic-veto re-mandate so it cannot fight the pivot ---
# Match the enhanced_mission branch that re-forces patch_function.
old_c = '''        } else if patch_function_strategy && critic_veto_present {
            format!(
                "{}\\n\\n[CRITIC VETO RECOVERY MANDATE]\\nThe previous patch_function proposal was explicitly rejected'''

new_c = '''        } else if patch_function_strategy && critic_veto_present && !tree_sitter_unaddressable {
            format!(
                "{}\\n\\n[CRITIC VETO RECOVERY MANDATE]\\nThe previous patch_function proposal was explicitly rejected'''

if old_c not in text:
    print("WARN: C-gate anchor not found; skipping enhanced_mission gate (check manually)", file=sys.stderr)
else:
    text = text.replace(old_c, new_c, 1)
    print("OK: gated critic-veto re-mandate on !tree_sitter_unaddressable")

path.write_text(text)
print(f"OK: one-shot pivot guard applied in {path}")
PY

echo
echo "=== Verify ==="
grep -n 'AST_UNADDRESSABLE_PIVOTED\|!tree_sitter_unaddressable' "$TARGET" || true

echo
echo "=== Compile ==="
(cd "$ROOT" && cargo check -p nexus_runtime_core 2>&1 | tail -20)
