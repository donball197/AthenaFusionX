#!/usr/bin/env bash
# surgical_ast_unaddressable_recovery.sh
# Apply AST-unaddressable recovery to cortical_loop.rs
set -euo pipefail

ROOT="${1:-$HOME/AthenaFusionX}"
TARGET="$ROOT/crates/runtime_core/src/cortical_loop.rs"

if [[ ! -f "$TARGET" ]]; then
  echo "ERROR: target not found: $TARGET" >&2
  exit 1
fi

# Idempotency: refuse to double-apply
if grep -q 'AST-UNADDRESSABLE RECOVERY' "$TARGET"; then
  echo "ALREADY APPLIED: AST-UNADDRESSABLE RECOVERY marker present in $TARGET"
  exit 0
fi

python3 - "$TARGET" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()

anchor = '''self.log(format!(
            "[CORTEX RECOVERY CHECK] strategy=`{}` patch_function={} critic_veto={} write_phase_ready={} evidence_bytes={}",
            active_strategy,
            patch_function_strategy,
            critic_veto_present,
            write_phase_ready,
            session.local_evidence.len()
        ));'''

# Tolerate minor whitespace differences on the closing ));
if anchor not in text:
    # Fallback: looser match on the log format string only
    alt_start = text.find('[CORTEX RECOVERY CHECK] strategy=')
    if alt_start < 0:
        print("ERROR: recovery-check anchor not found", file=sys.stderr)
        sys.exit(2)
    # Find end of that statement (next ');' after the format! call)
    semi = text.find(');', alt_start)
    if semi < 0:
        print("ERROR: could not locate end of recovery-check log statement", file=sys.stderr)
        sys.exit(2)
    insert_at = semi + 2
else:
    insert_at = text.find(anchor) + len(anchor)

insertion = r'''

        // --- AST-UNADDRESSABLE RECOVERY (Tree-sitter body missing) ---
        // When PatchFunction fails because the symbol has no Tree-sitter body,
        // demote the dead patch_function hypothesis and pivot to read_file of
        // the exact path named in the strategy. Do not guess a fallback path.
        // Do not synthesize replace_lines until READ_OK evidence exists.
        let tree_sitter_unaddressable = session.local_evidence.contains("has no Tree-sitter body")
            || session.local_evidence.contains("has no Tree-sitter body in Rust");

        if patch_function_strategy && tree_sitter_unaddressable {
            if let Some(h_id) = session.best_hypothesis_id {
                if let Some(node) = session.graph.nodes.get_mut(&h_id) {
                    if node.content.starts_with("patch_function:") {
                        node.belief_score = 0.05;
                    }
                }
            }

            let path = active_strategy
                .strip_prefix("patch_function:")
                .and_then(|rest| rest.split(':').next())
                .map(str::trim)
                .filter(|p| !p.is_empty());

            if let Some(path) = path {
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
                    "\x1b[1;36m>> [CORTEX AST-UNADDRESSABLE RECOVERY]\x1b[0m \
                     Symbol has no Tree-sitter body; pivoting to read_file."
                        .to_string(),
                );
            } else {
                self.log(
                    "\x1b[1;31m>> [CORTEX AST-UNADDRESSABLE RECOVERY]\x1b[0m \
                     Could not parse path from patch_function strategy; recovery aborted."
                        .to_string(),
                );
            }
        }
        // --- end AST-UNADDRESSABLE RECOVERY ---
'''

new_text = text[:insert_at] + insertion + text[insert_at:]
path.write_text(new_text)
print(f"OK: inserted AST-UNADDRESSABLE RECOVERY into {path}")
PY

echo
echo "=== Verify marker ==="
grep -n 'AST-UNADDRESSABLE RECOVERY' "$TARGET" || true

echo
echo "=== Compile check ==="
(cd "$ROOT" && cargo check -p nexus_runtime_core 2>&1 | tail -30)
