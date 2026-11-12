#!/usr/bin/env bash
# apply_cortex_fixes.sh -- AthenaFusionX cortex deadlock fixes (4 patches)
# Usage: ./apply_cortex_fixes.sh [--revert] [--no-check] [repo_root]
#   default repo_root = current dir. Backups: <file>.cortexfix.bak
set -euo pipefail

REVERT=0; CHECK=1; ROOT="."
for a in "$@"; do
  case "$a" in
    --revert) REVERT=1 ;;
    --no-check) CHECK=0 ;;
    *) ROOT="$a" ;;
  esac
done
ROOT="$(cd "$ROOT" && pwd)"
C="$ROOT/crates/runtime_core/src"
[ -d "$C" ] || { echo "!! $C not found (run from repo root)"; exit 1; }

if [ "$REVERT" = 1 ]; then
  n=0
  for b in $(find "$C" -name '*.cortexfix.bak'); do
    mv -f "$b" "${b%.cortexfix.bak}"; echo "restored ${b%.cortexfix.bak}"; n=$((n+1))
  done
  echo "reverted $n file(s)"; exit 0
fi

python3 - "$ROOT" <<'PY'
import re, sys, shutil, pathlib

ROOT = pathlib.Path(sys.argv[1])
C = ROOT / "crates/runtime_core/src"
failed = []

def load(p): return p.read_text()

def save(p, s):
    b = p.with_name(p.name + ".cortexfix.bak")
    if not b.exists():
        shutil.copy2(p, b)
    p.write_text(s)

def report(n, status, extra=""):
    print(f"[patch {n}] {status} {extra}".rstrip())
    if status.startswith("FAIL"):
        failed.append(n)

# ---- 1. reconnaissance_complete: exclude MinBeliefScore --------------------
p = C / "completion_engine.rs"; s = load(p)
i = s.find("pub fn reconnaissance_complete(")
j = s.find("mission_completion_authorized", i) if i >= 0 else -1
if i < 0 or j < 0:
    report(1, "FAIL", "reconnaissance_complete not found")
else:
    seg = s[i:j]
    anchor = "| CompletionCondition::WritePathAcquired { .. }"
    if "MinBeliefScore" in seg:
        report(1, "skip (already applied)")
    elif seg.count(anchor) != 1:
        report(1, "FAIL", f"anchor count={seg.count(anchor)}")
    else:
        new = seg.replace(anchor, anchor +
            "\n                        // Belief is an outcome of the work, not a prerequisite for"
            "\n                        // the write phase. Still enforced at halt."
            "\n                        | CompletionCondition::MinBeliefScore(_)")
        save(p, s[:i] + new + s[j:])
        report(1, "applied", str(p.relative_to(ROOT)))

# ---- 2. anti_loop: filter already-satisfied whole-file reads ---------------
p = C / "anti_loop.rs"; s = load(p)
i = s.find("pub fn classify_duplicate(")
if i < 0:
    report(2, "FAIL", "classify_duplicate not found")
elif "CORTEXFIX: satisfied whole-file read" in s:
    report(2, "skip (already applied)")
else:
    m = re.compile(r"\n    if matches!\(\s*action,\s*AgentAction::ReadFileChunk").search(s, i)
    if not m:
        report(2, "FAIL", "matches!(ReadFileChunk..) anchor not found")
    else:
        block = r'''
    // CORTEXFIX: satisfied whole-file read -- never re-admit it.
    if let AgentAction::ReadFile { path, .. } = action {
        let already = crate::outcome_verifier::OutcomeVerifier::mission_completion_authorized(
            evidence,
            1.0,
            &[CompletionCondition::ReadPathAcquired { path: path.clone() }],
        );
        if already {
            return FilterOutcome::FilterAndDecay;
        }
    }
'''
        save(p, s[:m.start()] + block + s[m.start():])
        report(2, "applied", str(p.relative_to(ROOT)))

# ---- 3. strategize: veto-admitted reads must not override write phase ------
p = C / "cortical_loop/strategize.rs"; s = load(p)
old = 'let veto_recon_ok = (session.local_evidence.contains("[CRITIC_VETO]")'
new = 'let veto_recon_ok = !write_phase_ready\n                && (session.local_evidence.contains("[CRITIC_VETO]")'
if "let veto_recon_ok = !write_phase_ready" in s:
    report(3, "skip (already applied)")
elif s.count(old) != 1:
    report(3, "FAIL", f"anchor count={s.count(old)}")
else:
    save(p, s.replace(old, new))
    report(3, "applied", str(p.relative_to(ROOT)))

# ---- 4. evaluate: deterministic progress floor -----------------------------
p = C / "cortical_loop/evaluate.rs"; s = load(p)
tag = r'"\x1b[33m>> [EVALUATION]'
if "PROGRESS FLOOR" in s:
    report(4, "skip (already applied)")
elif s.count(tag) != 1:
    report(4, "FAIL", f"[EVALUATION] log anchor count={s.count(tag)}")
else:
    k = s.find(tag)
    st = s.rfind("self.log(format!(", 0, k)
    ls = s.rfind("\n", 0, st) + 1
    indent = s[ls:st]
    block = r'''// PROGRESS FLOOR: the LLM judge compares the action to the pinned
// strategy label, not to the contract. If this action's evidence reduced
// the number of unmet non-belief conditions, it is progress and may not
// be penalized.
let mut eval = eval;
{
    use crate::completion_engine::CompletionCondition as CC;
    let conds: Vec<CC> = session
        .contract
        .conditions
        .iter()
        .filter(|c| !matches!(c, CC::MinBeliefScore(_)))
        .cloned()
        .collect();
    let before = crate::outcome_verifier::OutcomeVerifier::unmet_conditions(
        &session.local_evidence, 1.0, &conds,
    )
    .len();
    let after_ev = format!("{}\n{}", session.local_evidence, evidence);
    let after = crate::outcome_verifier::OutcomeVerifier::unmet_conditions(
        &after_ev, 1.0, &conds,
    )
    .len();
    if after < before && eval.confidence_modifier < 0.10 {
        self.log(format!(
            "\x1b[1;36m>> [PROGRESS FLOOR] unmet {} -> {}; modifier {:.2} -> 0.10\x1b[0m",
            before, after, eval.confidence_modifier
        ));
        eval.confidence_modifier = 0.10;
    }
}
'''
    block = "".join(indent + ln + "\n" if ln else "\n" for ln in block.splitlines())
    save(p, s[:ls] + block + s[ls:])
    report(4, "applied", str(p.relative_to(ROOT)))

sys.exit(1 if failed else 0)
PY

echo
if [ "$CHECK" = 1 ]; then
  echo ">> cargo check (nightly, runtime_core)..."
  (cd "$ROOT" && cargo +nightly check -p runtime_core 2>&1 | tail -40) || {
    echo "!! compile errors above. Fix or run: $0 --revert"; exit 1; }
  echo ">> OK. Next: cargo +nightly test -p runtime_core, then rerun the mission."
fi
