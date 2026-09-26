#!/usr/bin/env bash
set -euo pipefail

ROOT="${HOME}/AthenaFusionx"
CORTEX="crates/runtime_core/src/cortical_loop.rs"

cd "$ROOT"
[[ -f "$CORTEX" ]] || { echo "REFUSED: $CORTEX not found" >&2; exit 1; }

python3 - "$CORTEX" <<'PY'
import sys
from pathlib import Path

p = Path(sys.argv[1])
s = p.read_text()

if "bypass_antiloop_for_retention" in s:
    print("SKIP: retention anti-loop bypass already present")
    sys.exit(0)

old = '''        for action in actions {
            let action_key = Self::action_key(&action);

            let attempt_count = session.global_action_attempt_count(&action_key);

            // STRICT ANTI-LOOP:
            // An identical action that has already crossed the execution
            // boundary is not new evidence. Drop it from the batch, but
            // let never-attempted sibling actions in the same proposal
            // proceed -- one stale action must not starve the rest.
            if attempt_count >= 1 {
                self.log(format!(
                    "\\x1b[1;31m>> [ANTI-LOOP] Duplicate action [{}] filtered from batch.\\x1b[0m",
                    action_key
                ));
                vetoed_keys.push(action_key);
            } else {
                filtered_actions.push(action);
            }

            // IMPORTANT:
            // Do NOT count the action here. This is the critic stage.
            // The counter will be recorded only after the action passes
            // the ActionGate and is actually sent to the executor.
        }'''

new = '''        for action in actions {
            let action_key = Self::action_key(&action);

            let attempt_count = session.global_action_attempt_count(&action_key);

            // STRICT ANTI-LOOP:
            // An identical action that has already crossed the execution
            // boundary is not new evidence. Drop it from the batch, but
            // let never-attempted sibling actions in the same proposal
            // proceed -- one stale action must not starve the rest.
            //
            // EXCEPTION — active compile remediation:
            // While a retention episode is open, the on-disk source is
            // known to have changed since the previously-recorded
            // attempt (the compiler failure left the source in a state
            // that the same patch would now correct). A repeated
            // PatchFunction is therefore a materially different action
            // against a different pre-image, even though its new_body
            // hashes identically. Permit it through the anti-loop filter
            // so the corrective patch the retention path instructs the
            // strategist to produce can actually execute.
            let bypass_antiloop_for_retention = session.compile_remediation.is_active()
                && matches!(action, AgentAction::PatchFunction { .. });

            if attempt_count >= 1 && !bypass_antiloop_for_retention {
                self.log(format!(
                    "\\x1b[1;31m>> [ANTI-LOOP] Duplicate action [{}] filtered from batch.\\x1b[0m",
                    action_key
                ));
                vetoed_keys.push(action_key);
            } else {
                if attempt_count >= 1 {
                    self.log(format!(
                        "\\x1b[1;33m>> [ANTI-LOOP] Permitting repeated action [{}] during active compile remediation.\\x1b[0m",
                        action_key
                    ));
                }
                filtered_actions.push(action);
            }

            // IMPORTANT:
            // Do NOT count the action here. This is the critic stage.
            // The counter will be recorded only after the action passes
            // the ActionGate and is actually sent to the executor.
        }'''

count = s.count(old)
if count != 1:
    print(f"REFUSED: expected exactly 1 anti-loop filter block, found {count}")
    sys.exit(2)

s = s.replace(old, new, 1)
p.write_text(s)
print("OK: anti-loop filter bypasses PatchFunction during active remediation")
PY

cargo check -p nexus_runtime_core
cargo test -p nexus_runtime_core cortical_loop -- --nocapture
