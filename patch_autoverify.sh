#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
F=crates/runtime_core/src/cortical_loop/strategize.rs
python3 - "$F" <<'PY'
import sys
p = sys.argv[1]; s = open(p).read()
if "AUTO_VERIFY_READ" in s:
    print("[skip] already patched"); sys.exit(0)

a1 = "        let mut active_strategy = if write_phase_ready {\n"
a2 = "            strategy\n        } else if session.contract.is_mutation_task\n            && unwritten_targets.is_empty()\n"
for a in (a1, a2):
    if s.count(a) != 1:
        sys.exit(f"[FAIL] anchor count {s.count(a)} for:\n{a}")

pre = '''        // AUTO-VERIFY: after a report artifact is written, read it back once so the
        // verifier has a READ_OK to pair claims against, before any halt is forced.
        let auto_verify_path: Option<String> = session
            .contract
            .required_artifact_paths
            .iter()
            .map(|a| a.path.to_string())
            .find(|p| {
                (p.ends_with(".md") || p.ends_with(".txt"))
                    && !session.local_evidence.contains(&format!("[READ_OK] {}", p))
                    && !session.local_evidence.contains(&format!("[AUTO_VERIFY_READ] {}", p))
            });

'''
s = s.replace(a1, pre + a1)

new_branch = '''            strategy
        } else if auto_verify_path.is_some()
            && session.contract.is_mutation_task
            && unwritten_targets.is_empty()
            && crate::evidence_markers::has_mutation_marker(&session.local_evidence)
        {
            let p = auto_verify_path.clone().unwrap_or_default();
            self.log(format!(
                "\\x1b[1;35m>> [CORTEX PHASE TRANSITION]\\x1b[0m Artifact written; forcing verification read `read_file:{}` before halt.",
                p
            ));
            session.local_evidence.push_str(&format!(
                "\\n[AUTO_VERIFY_READ] {}\\n[NODE PHASE] The artifact was written. Read it back now so its claims can be paired with READ_OK evidence; halt only after that.\\n",
                p
            ));
            format!("read_file:{}", p)
        } else if session.contract.is_mutation_task
            && unwritten_targets.is_empty()
'''
s = s.replace(a2, new_branch)
open(p, "w").write(s)
print("[ok] strategize.rs patched")
PY
CARGO_TERM_COLOR=never cargo +nightly check --workspace --all-targets 2>&1 | grep -E '^error' -A10 | head -40 || true
echo "--- done: empty above means clean ---"
