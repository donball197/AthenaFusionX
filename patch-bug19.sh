#!/usr/bin/env bash
# patch-bug19.sh — widen the critic bypass from "satisfies full contract"
# to "advances >=1 unmet condition deterministically".
set -euo pipefail

usage() {
    cat <<'USAGE_EOF'
Usage: patch-bug19.sh [--apply] [--check] [REPO_ROOT]

  (no flags)   Dry-run: print unified diffs.
  --apply      Apply changes to working tree.
  --check      After applying, run RUSTFLAGS="-D warnings" cargo check -p nexus_runtime_core
USAGE_EOF
    exit 1
}

APPLY=0; CHECK=0; REPO_ROOT=""
for arg in "$@"; do
    case "$arg" in
        --apply) APPLY=1 ;;
        --check) CHECK=1 ;;
        -h|--help) usage ;;
        -*) echo "Unknown flag: $arg" >&2; usage ;;
        *) REPO_ROOT="$arg" ;;
    esac
done
REPO_ROOT="${REPO_ROOT:-$PWD}"
cd "$REPO_ROOT"

if [ ! -f crates/runtime_core/src/cortical_loop/critic.rs ]; then
    echo "error: not at repo root (crates/runtime_core/ not found)" >&2; exit 1
fi

python3 - "$APPLY" <<'PY'
import sys, difflib
from pathlib import Path
from collections import OrderedDict

APPLY = sys.argv[1] == "1"
files = OrderedDict()
statuses = []

def load(path):
    if path not in files:
        files[path] = [Path(path).read_text()] * 2
    return files[path]

def apply_edit(path, desc, old, new):
    if not Path(path).exists():
        statuses.append(("MISSING FILE", f"{path}: {desc}")); return
    orig, final = load(path)
    if new in final:
        statuses.append(("already applied", desc)); return
    if old not in final:
        statuses.append(("PREIMAGE NOT FOUND", f"{path}: {desc}")); return
    files[path][1] = final.replace(old, new, 1)
    statuses.append(("will apply", desc))

CS = "crates/runtime_core/src/contract_status.rs"
CR = "crates/runtime_core/src/cortical_loop/critic.rs"

PREDICATE_BLOCK = (
    "\n"
    "/// BUG-19: deterministic contract-progress predicate used by the\n"
    "/// critic gate. True iff at least one proposed action would satisfy\n"
    "/// a currently-UNMET completion condition. No LLM involvement — the\n"
    "/// question \"does this proposal advance the contract?\" has a\n"
    "/// deterministic answer, and the LLM critic answered it\n"
    "/// non-deterministically (task 2: identical 1-of-6 proposals were\n"
    "/// approved on turns 13/24/28 and vetoed on 19/45/56).\n"
    "pub(crate) fn proposal_advances_contract(\n"
    "    proposal: &[crate::agent_protocol::AgentAction],\n"
    "    contract: &crate::completion_engine::SubgoalContract,\n"
    "    local_evidence: &str,\n"
    ") -> bool {\n"
    "    proposal.iter().any(|action| {\n"
    "        contract.conditions.iter().any(|cond| {\n"
    "            !crate::cortical::contract_advance::condition_is_met(cond, local_evidence)\n"
    "                && action_would_satisfy(action, cond)\n"
    "        })\n"
    "    })\n"
    "}\n"
    "\n"
    "fn action_would_satisfy(\n"
    "    action: &crate::agent_protocol::AgentAction,\n"
    "    cond: &crate::completion_engine::CompletionCondition,\n"
    ") -> bool {\n"
    "    use crate::agent_protocol::AgentAction as A;\n"
    "    use crate::completion_engine::CompletionCondition as C;\n"
    "    match (action, cond) {\n"
    "        // A SearchCode emits `[SEARCH_CODE_OK] Path: <p> | Query: '<q>'`,\n"
    "        // which search_checked_satisfied() accepts as proof that\n"
    "        // SearchCheckedAcquired{target:\"<leaf>:<term>\"} was checked.\n"
    "        (A::SearchCode { path, query, .. }, C::SearchCheckedAcquired { target }) => {\n"
    "            let leaf = path.trim_end_matches('/').rsplit('/').next().unwrap_or(path);\n"
    "            target == &format!(\"{}:{}\", leaf, query)\n"
    "        }\n"
    "        // Path-bound reads emit [READ_OK] / [READ_FUNCTION_OK] which\n"
    "        // satisfies ReadPathAcquired{path}.\n"
    "        (A::ReadFile { path, .. }, C::ReadPathAcquired { path: want })\n"
    "        | (A::ReadFunction { path, .. }, C::ReadPathAcquired { path: want }) => {\n"
    "            path == want || path.ends_with(want.as_str())\n"
    "        }\n"
    "        // ExecuteCommand is handled earlier by is_required_verification_command\n"
    "        // in the existing bypass; do not duplicate here.\n"
    "        _ => false,\n"
    "    }\n"
    "}\n"
)

if not Path(CS).exists():
    statuses.append(("MISSING FILE", f"{CS}: proposal_advances_contract"))
else:
    orig, final = load(CS)
    if "fn proposal_advances_contract(" in final:
        statuses.append(("already applied", "Add proposal_advances_contract"))
    else:
        files[CS][1] = final.rstrip() + "\n" + PREDICATE_BLOCK
        statuses.append(("will apply", "Add proposal_advances_contract"))

CR_OLD = '        self.log("\\x1b[31m>> [RED TEAM] Criticizing proposal...\\x1b[0m".to_string());\n'
CR_NEW = (
    "        // BUG-19: a proposal that deterministically resolves at\n"
    "        // least one unmet completion condition is contract-\n"
    "        // progressing by construction. The LLM critic's role is\n"
    "        // scope/safety review; \"is this enough progress?\" has a\n"
    "        // deterministic answer (task 2: six parallel criteria\n"
    "        // flattened into one node; each 1-of-6 search was approved\n"
    "        // or vetoed at the LLM's whim).\n"
    "        if crate::contract_status::proposal_advances_contract(\n"
    "            &actions,\n"
    "            &session.contract,\n"
    "            &session.local_evidence,\n"
    "        ) {\n"
    "            self.log(\n"
    "                \"\\x1b[1;36m>> [CRITIC BYPASS] Proposal advances contract deterministically; LLM critic skipped.\\x1b[0m\"\n"
    "                    .to_string(),\n"
    "            );\n"
    "            session.last_critic_rejected_actions = None;\n"
    "            return Ok(CortexState::Execute { actions, proposal, belief });\n"
    "        }\n"
    "\n"
    + CR_OLD
)
apply_edit(CR, "Add deterministic contract-progress bypass before LLM critic", CR_OLD, CR_NEW)

print(); print("=" * 72); print("Edit summary"); print("=" * 72)
bad = False
for s, d in statuses:
    print(f"  [{s:^16}] {d}")
    if s in ("PREIMAGE NOT FOUND", "MISSING FILE"):
        bad = True
if bad:
    print("\nSome edits could not be prepared. Nothing was written.", file=sys.stderr)
    sys.exit(1)

print()
if APPLY:
    print("=" * 72); print("Writing files"); print("=" * 72)
    for p, (o, f) in files.items():
        if o != f:
            Path(p).write_text(f); print(f"  wrote {p}")
else:
    print("=" * 72); print("Dry-run — unified diff (pass --apply to write)"); print("=" * 72)
    for p, (o, f) in files.items():
        if o != f:
            sys.stdout.writelines(difflib.unified_diff(
                o.splitlines(keepends=True), f.splitlines(keepends=True),
                fromfile=f"a/{p}", tofile=f"b/{p}"))
sys.exit(0)
PY

if [ "$APPLY" = "1" ] && [ "$CHECK" = "1" ]; then
    echo
    echo "======================================================================"
    echo "Running cargo check"
    echo "======================================================================"
    RUSTFLAGS="-D warnings" cargo check -p nexus_runtime_core; echo "exit: $?"
fi
