#!/usr/bin/env bash
set -euo pipefail
EVAL="crates/runtime_core/src/cortical_loop/evaluate.rs"
cp "$EVAL" "$EVAL.bak.$(date +%s)"
python3 - "$EVAL" << 'PY'
import re, sys
from pathlib import Path

p = Path(sys.argv[1])
src = p.read_text()

# 1. Insert physical_completion_ready before `let command_only_contract`
anchor = "let command_only_contract = "
if src.count(anchor) != 1:
    print(f"FATAL: anchor count = {src.count(anchor)}"); sys.exit(1)
line_start = src.rfind('\n', 0, src.find(anchor)) + 1
indent = src[line_start:src.find(anchor)]
ins = (
    f"// Physical-only completion: allow halt when all non-belief conditions\n"
    f"{indent}// are met even if MinBeliefScore is unmet.\n"
    f"{indent}let physical_completion_ready = is_halt_action\n"
    f"{indent}    && !deterministic_contract_satisfied\n"
    f"{indent}    && crate::outcome_verifier::OutcomeVerifier::physical_contract_satisfied(\n"
    f"{indent}        &session.local_evidence,\n"
    f"{indent}        current_belief,\n"
    f"{indent}        &session.contract.conditions,\n"
    f"{indent}    );\n\n{indent}"
)
src = src.replace(anchor, ins + anchor, 1)

# 2. Second guard: allow either full satisfaction OR physical completion
pat = r'if is_halt_action(\s*)&& deterministic_contract_satisfied(\s*)&& \(eval\.confidence_modifier >= 0\.0 \|\| session\.contract\.read_only\)'
rep = r'if is_halt_action\1&& (deterministic_contract_satisfied || physical_completion_ready)\2&& (eval.confidence_modifier >= 0.0 || session.contract.read_only)'
if len(re.findall(pat, src)) != 1:
    print("FATAL: guard 2 pattern count != 1"); sys.exit(1)
src = re.sub(pat, rep, src, count=1)

# 3. Reject guard: suppress when physical completion ready
old3 = "if is_halt_action && !deterministic_contract_satisfied"
new3 = "if is_halt_action && !deterministic_contract_satisfied && !physical_completion_ready"
if src.count(old3) != 1:
    print(f"FATAL: guard 3 count = {src.count(old3)}"); sys.exit(1)
src = src.replace(old3, new3, 1)

# Sanity
assert src.count("physical_completion_ready") == 3, f"unexpected count {src.count('physical_completion_ready')}"
p.write_text(src)
print("OK: 3 edits applied")
PY
cargo +nightly check -p nexus_runtime_core
cargo +nightly test -p nexus_runtime_core --lib cortical_loop 2>&1 | tail -5
