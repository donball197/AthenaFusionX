#!/usr/bin/env bash
# surgical_verify_read_bypass.sh
# Fix: after an artifact is written, the mandatory post-write read-back
# (read_file on a required_artifact_path that already has [WRITE_OK] but no
# [READ_OK]) is vetoed by the LLM critic, which misreads the strategist's
# stale reasoning text as a write attempt and/or sees stale PATH_NOT_FOUND
# history. This is a deterministic bypass at the critic entry point.
# Confirmed blocker of plan 40cfaaba (2026-11-12).
set -euo pipefail

ROOT="$(pwd)"
FILE="$ROOT/crates/runtime_core/src/cortical_loop/critic.rs"
[[ -f "$FILE" ]] || { echo "FATAL: missing $FILE"; exit 1; }

TS=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="$ROOT/.surgical_backup_verify_read_$TS"
mkdir -p "$BACKUP_DIR"
cp -a "$FILE" "$BACKUP_DIR/"
echo "Backups → $BACKUP_DIR"

python3 - <<'PY'
from pathlib import Path
import sys

path = Path("crates/runtime_core/src/cortical_loop/critic.rs")
src = path.read_text()
orig = src

# Anchor: the existing proposal_advances_contract bypass. We insert our
# new bypass immediately after it so it takes precedence over the LLM call.
old = '''        if crate::contract_status::proposal_advances_contract(
            &actions,
            &session.contract,
            &session.local_evidence,
        ) {
            self.log(
                "\\x1b[1;36m>> [CRITIC BYPASS] Proposal advances contract deterministically; LLM critic skipped.\\x1b[0m"
                    .to_string(),
            );
            session.last_critic_rejected_actions = None;
            return Ok(CortexState::Execute { actions, proposal, belief });
        }'''

new = old + '''

        // VERIFICATION-READ BYPASS: a read_file on a required artifact that
        // has already been written ([WRITE_OK] in evidence) but not yet
        // read back ([READ_OK] absent) is the mandatory post-write
        // verification step. The action is strictly non-destructive, the
        // artifact exists, and executing it satisfies the pending
        // ReadPathAcquired for that path. This is deterministic — do not
        // ask the LLM critic to second-guess it. The critic has been
        // observed to misread the strategist's stale "write the report"
        // reasoning text as a write attempt, and to anchor on stale
        // PATH_NOT_FOUND events that pre-date the successful [WRITE_OK].
        // Confirmed blocker of plan 40cfaaba (2026-11-12): turns 33-60
        // exhausted on exactly this veto loop.
        {
            let verification_read = actions.iter().any(|a| {
                if let AgentAction::ReadFile { _thought_process: None, path, .. } = a {
                    session.contract.required_artifact_paths.iter().any(|art| &art.path == path)
                        && session.local_evidence.contains(&format!("[WRITE_OK] {}", path))
                        && !session.local_evidence.contains(&format!("[READ_OK] {}", path))
                } else {
                    false
                }
            });
            if verification_read {
                self.log(
                    "\\x1b[1;36m>> [CRITIC BYPASS] Post-write verification read of required artifact; LLM critic skipped.\\x1b[0m"
                        .to_string(),
                );
                session.last_critic_rejected_actions = None;
                return Ok(CortexState::Execute { actions, proposal, belief });
            }
        }'''

if old not in src:
    print("FATAL: proposal_advances_contract bypass site not found")
    sys.exit(1)
src = src.replace(old, new, 1)

path.write_text(src)
print(f"Patched {path}")
PY

echo
echo "========== VERIFY: patch present =========="
grep -n "VERIFICATION-READ BYPASS\|Post-write verification read" \
  crates/runtime_core/src/cortical_loop/critic.rs | head -10

echo
echo "========== CHECK =========="
cargo +nightly check -p nexus_runtime_core 2>&1 | tail -30

echo
echo "Backups → $BACKUP_DIR"
echo
echo "Next: rerun ./nexus.sh with the same audit mission and watch for:"
echo "  [CRITIC BYPASS] Post-write verification read of required artifact"
echo "  [READ_OK] verifier_contract_audit.md"
echo "  [EVENT] [CORTEX COMPLETE] or [EVENT] [NODE VERIFIED]"
