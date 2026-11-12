#!/usr/bin/env bash
# patch_correction_cycle.sh — unfreeze the post-veto correction cycle.
#
# Fix A: critic_evidence now carries a [CRITIC PROPOSAL AUTHORITY] note for
#        write_file proposals, so the LLM critic judges the proposal against
#        SOURCE-CODE evidence rather than against a stale prior version of the
#        same file still sitting in local_evidence. Without this, a correct
#        correction after a prior rejection is vetoed on stale grounds, and
#        the strategist spins to death.
#
# Fix D: repeat-veto now gates on evidence growth. When the strategist retries
#        the same rejected action batch but has produced NEW evidence since
#        the last rejection (>= 500 bytes), the repeat-veto is skipped and the
#        critic re-evaluates. Prevents a single stale veto from deadlocking
#        the correction cycle by decaying belief 0.20 per retry to 0.00.
#
# Two files:
#   crates/runtime_core/src/cortical_loop/critic.rs
#   crates/runtime_core/src/cognition/epistemic_session.rs
#
# Usage:
#   ./patch_correction_cycle.sh             # dry-run
#   ./patch_correction_cycle.sh --apply     # apply + targeted tests
#   ./patch_correction_cycle.sh --apply --full  # also --lib suite
#   ./patch_correction_cycle.sh --help

set -euo pipefail

ROOT="$(pwd)"
CRITIC="$ROOT/crates/runtime_core/src/cortical_loop/critic.rs"
SESS="$ROOT/crates/runtime_core/src/cognition/epistemic_session.rs"
APPLY=0
FULL=0

for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    --full)  FULL=1 ;;
    -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

for f in "$CRITIC" "$SESS"; do
  [[ -f "$f" ]] || { echo "ERROR: $f not found (run from repo root)" >&2; exit 1; }
done

STAMP="$(date +%Y%m%d-%H%M%S)"
cp "$CRITIC" "$CRITIC.bak.$STAMP"
cp "$SESS" "$SESS.bak.$STAMP"
echo "backups:"
echo "  $CRITIC.bak.$STAMP"
echo "  $SESS.bak.$STAMP"

PATCH_CR="$(mktemp)"
PATCH_SE="$(mktemp)"
trap 'rm -f "$PATCH_CR" "$PATCH_SE"' EXIT

python3 - "$CRITIC" "$PATCH_CR" "$SESS" "$PATCH_SE" <<'PY'
import sys

cr_in, cr_out, se_in, se_out = sys.argv[1:5]

def expect_one(src, needle, label):
    n = src.count(needle)
    if n != 1:
        sys.stderr.write(f"ERROR[{label}]: expected 1 match, found {n}\n")
        sys.stderr.write(f"  needle starts: {needle[:120]!r}\n")
        sys.exit(3)

# ============================================================
# epistemic_session.rs: add the evidence-length snapshot field
# ============================================================
src = open(se_in, encoding='utf-8').read()

field_anchor = (
    '    #[serde(skip)]\n'
    '    pub last_critic_rejected_actions: Option<Vec<String>>,\n'
    '}\n'
)
field_new = (
    '    #[serde(skip)]\n'
    '    pub last_critic_rejected_actions: Option<Vec<String>>,\n'
    '    /// `local_evidence.len()` at the moment of the last critic rejection.\n'
    '    /// A retried proposal whose batch matches the last rejection is only\n'
    '    /// repeat-vetoed while evidence has not meaningfully grown since the\n'
    '    /// veto; once the strategist produces new evidence the critic gets a\n'
    '    /// re-look. Prevents a single stale veto from deadlocking a correction\n'
    '    /// cycle that is producing fresh reconnaissance.\n'
    '    #[serde(skip)]\n'
    '    pub last_critic_rejected_at_evidence_len: Option<usize>,\n'
    '}\n'
)
expect_one(src, field_anchor, "SE-field")
src = src.replace(field_anchor, field_new, 1)

init_anchor = (
    '            last_critic_rejected_actions: None,\n'
    '        }\n'
)
init_new = (
    '            last_critic_rejected_actions: None,\n'
    '            last_critic_rejected_at_evidence_len: None,\n'
    '        }\n'
)
expect_one(src, init_anchor, "SE-init")
src = src.replace(init_anchor, init_new, 1)
open(se_out, 'w', encoding='utf-8').write(src)

# ============================================================
# critic.rs: three rejects set the snapshot; repeat-veto gates
#            on evidence growth; critic_evidence gains the
#            supersession note.
# ============================================================
src = open(cr_in, encoding='utf-8').read()

# ---- 1. Protected-path reject (16-space indent block) ----
old = (
    '                session.last_critic_rejected_actions = Some(\n'
    '                    actions\n'
    '                        .iter()\n'
    '                        .map(crate::cortical::action_identity::action_key)\n'
    '                        .collect::<Vec<_>>(),\n'
    '                );\n'
)
new = (
    '                session.last_critic_rejected_actions = Some(\n'
    '                    actions\n'
    '                        .iter()\n'
    '                        .map(crate::cortical::action_identity::action_key)\n'
    '                        .collect::<Vec<_>>(),\n'
    '                );\n'
    '                session.last_critic_rejected_at_evidence_len =\n'
    '                    Some(session.local_evidence.len());\n'
)
expect_one(src, old, "CR-protected-reject")
src = src.replace(old, new, 1)

# ---- 2. Grounding-gate reject (20-space indent block) ----
old = (
    '                    session.last_critic_rejected_actions = Some(\n'
    '                        actions\n'
    '                            .iter()\n'
    '                            .map(crate::cortical::action_identity::action_key)\n'
    '                            .collect::<Vec<_>>(),\n'
    '                    );\n'
)
new = (
    '                    session.last_critic_rejected_actions = Some(\n'
    '                        actions\n'
    '                            .iter()\n'
    '                            .map(crate::cortical::action_identity::action_key)\n'
    '                            .collect::<Vec<_>>(),\n'
    '                    );\n'
    '                    session.last_critic_rejected_at_evidence_len =\n'
    '                        Some(session.local_evidence.len());\n'
)
expect_one(src, old, "CR-grounding-reject")
src = src.replace(old, new, 1)

# ---- 3. LLM-critic reject (variable-then-assign) ----
old = '                    session.last_critic_rejected_actions = Some(rejected_action_keys);\n'
new = (
    '                    session.last_critic_rejected_actions = Some(rejected_action_keys);\n'
    '                    session.last_critic_rejected_at_evidence_len =\n'
    '                        Some(session.local_evidence.len());\n'
)
expect_one(src, old, "CR-llm-reject")
src = src.replace(old, new, 1)

# ---- 4. Repeat-veto check gated on evidence growth ----
old = (
    '        let current_action_keys =\n'
    '            actions.iter().map(crate::cortical::action_identity::action_key).collect::<Vec<_>>();\n'
    '\n'
    '        if session.last_critic_rejected_actions.as_ref() == Some(&current_action_keys) {\n'
)
new = (
    '        let current_action_keys =\n'
    '            actions.iter().map(crate::cortical::action_identity::action_key).collect::<Vec<_>>();\n'
    '\n'
    '        // Evidence-growth gate: a retried action batch is only repeat-vetoed\n'
    '        // while evidence has not meaningfully grown since the last rejection.\n'
    '        // 500 bytes is larger than any single [CRITIC_VETO] + [CRITIC_REPEAT_VETO]\n'
    '        // message pair, so ordinary veto churn does not trip the gate; any real\n'
    '        // tool result (read, search, command) does.\n'
    '        const REPEAT_VETO_EVIDENCE_GROWTH_FLOOR: usize = 500;\n'
    '        let evidence_unchanged = session\n'
    '            .last_critic_rejected_at_evidence_len\n'
    '            .map_or(true, |len| {\n'
    '                session.local_evidence.len().saturating_sub(len)\n'
    '                    < REPEAT_VETO_EVIDENCE_GROWTH_FLOOR\n'
    '            });\n'
    '        if session.last_critic_rejected_actions.as_ref() == Some(&current_action_keys)\n'
    '            && evidence_unchanged\n'
    '        {\n'
)
expect_one(src, old, "CR-repeat-veto-gate")
src = src.replace(old, new, 1)

# ---- 5. Supersession note before the LLM critic call ----
old = (
    '        let critic_evidence = format!(\n'
    '            "{}{}",\n'
    '            critic_evidence,\n'
    '            contract_facts(&session.contract.conditions, &session.local_evidence)\n'
    '        );\n'
    '        match EpistemicEngine::criticize_proposal(\n'
)
new = (
    '        let critic_evidence = format!(\n'
    '            "{}{}",\n'
    '            critic_evidence,\n'
    '            contract_facts(&session.contract.conditions, &session.local_evidence)\n'
    '        );\n'
    '        // For write_file proposals, the proposal supersedes any prior version\n'
    '        // of the same path still visible in local_evidence. Without this note,\n'
    '        // the LLM critic compares the new content against the stale content\n'
    '        // captured by an earlier [READ_OK] / earlier write and vetoes the\n'
    '        // correction as a mismatch. The note is emitted unconditionally for\n'
    '        // write_file actions; when no prior version exists the phrase "any\n'
    '        // previous content" is vacuously satisfied and the note is harmless.\n'
    '        let supersession_note: String = actions\n'
    '            .iter()\n'
    '            .filter_map(|a| match a {\n'
    '                AgentAction::WriteFile { path, .. } => Some(format!(\n'
    '                    "\\n[CRITIC PROPOSAL AUTHORITY - {}] The proposed write_file \\\n'
    '                     REPLACES any previous content of this path visible in the \\\n'
    '                     evidence above. Judge the PROPOSAL CONTENT against the \\\n'
    '                     SOURCE-CODE evidence (READ_OK blocks for other paths, \\\n'
    '                     CMD_OK output, SEARCH_CODE_OK results). Do NOT reject a \\\n'
    '                     proposal because it differs from the previous version of \\\n'
    '                     the same file in local_evidence - that is the correct \\\n'
    '                     behavior for a correction after a prior rejection.\\n",\n'
    '                    path\n'
    '                )),\n'
    '                _ => None,\n'
    '            })\n'
    '            .collect();\n'
    '        let critic_evidence = format!("{}{}", critic_evidence, supersession_note);\n'
    '        match EpistemicEngine::criticize_proposal(\n'
)
expect_one(src, old, "CR-supersession-note")
src = src.replace(old, new, 1)

open(cr_out, 'w', encoding='utf-8').write(src)
PY

echo
echo "===== diff: epistemic_session.rs ====="
diff -u "$SESS" "$PATCH_SE" || true
echo "======================================"
echo
echo "===== diff: cortical_loop/critic.rs ====="
diff -u "$CRITIC" "$PATCH_CR" || true
echo "========================================="
echo

if [[ "$APPLY" -ne 1 ]]; then
  echo "dry-run only. re-run with --apply to install."
  exit 0
fi

install -m 0644 "$PATCH_CR" "$CRITIC"
install -m 0644 "$PATCH_SE" "$SESS"
echo "patched both files"

rollback() {
  echo >&2
  echo "!! verification failed; rolling back" >&2
  cp "$CRITIC.bak.$STAMP" "$CRITIC"
  cp "$SESS.bak.$STAMP" "$SESS"
  exit 1
}
trap 'rollback' ERR

echo ">> cargo check -p nexus_runtime_core ..."
cargo check -p nexus_runtime_core || rollback

echo ">> cargo test -p nexus_runtime_core --lib 'cortical_loop::critic' ..."
cargo test -p nexus_runtime_core --lib 'cortical_loop::critic' || rollback

echo ">> cargo test -p nexus_runtime_core --lib 'cognition::epistemic_session' ..."
cargo test -p nexus_runtime_core --lib 'cognition::epistemic_session' || rollback

if [[ "$FULL" -eq 1 ]]; then
  echo ">> cargo test -p nexus_runtime_core --lib ..."
  cargo test -p nexus_runtime_core --lib || rollback
fi

trap - ERR
echo
echo "OK: patch applied and verified."
echo "Next: cargo fmt -p nexus_runtime_core"
