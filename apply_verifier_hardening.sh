#!/usr/bin/env bash
set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
elif [[ -n "${1:-}" ]]; then
    echo "[ERROR] unknown argument: $1" >&2
    exit 2
fi

WORKSPACE_ROOT="$(realpath -m "${WORKSPACE_ROOT:-$(pwd)}")"
echo "=== verifier hardening (claim-check input + grounding + heavy route) | root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

EP="crates/runtime_core/src/epistemic_engine.rs"
if [[ ! -f "$WORKSPACE_ROOT/$EP" ]]; then
    echo "[PREFLIGHT FAILED] missing $EP" >&2
    exit 1
fi

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
EP_REL="$EP" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
EP   = os.environ["EP_REL"]

files = {EP: (root / EP, (root / EP).read_text(encoding="utf-8"))}
patched = {EP: files[EP][1]}

# ============================ anchors =====================================

# ---- 1. halt claims must reach the verifier intact -----------------------
ACT_CAP_OLD = '''        let truncated_action: String = action.chars().take(2000).collect();'''

ACT_CAP_NEW = '''        // The halt text IS the claim source for read-only analysis halts; the
        // claim-check below must see the full claims. The old 2000-char cap
        // silently truncated ROOT_CAUSE/TRACE/FIX answers mid-claim, leaving
        // the verifier to judge only their openings.
        let truncated_action: String = action.chars().take(6000).collect();'''

# ---- 2. grounding enforcement in the claim-check -------------------------
QUOTE_OLD = 'name the contradicted claim in the rationale.'

QUOTE_NEW = (
    'name the contradicted claim in the rationale. '
    'Before deciding, quote the exact source lines from EVIDENCE that support '
    'or contradict each numbered claim; any claim that cannot be paired with '
    'source lines is unverified and must fail.'
)

# ---- 3. JSON Schema Reordering (Chain-of-Thought Enabler) ----------------
SCHEMA_OLD = '{{"goal_satisfied": true, "evidence_confidence": 0.9, "independent_check_passed": true, "rationale": "<reason>"}}"#,'

SCHEMA_NEW = '{{"rationale": "<reasoning and exact source quotes>", "goal_satisfied": true, "evidence_confidence": 0.9, "independent_check_passed": true}}"#,'

# ---- 4. pin the verification route to the heavy tier ---------------------
HEAVY_OLD = '''        let models = match nexus_algorithms::Router::select("verify") {
            Some(nexus_algorithms::ModelTier::Heavy) => crate::model_config::get_heavy_models(),
            Some(nexus_algorithms::ModelTier::Light) => crate::model_config::get_light_models(),
            Some(nexus_algorithms::ModelTier::Planner) => crate::model_config::get_planner_models(),
            None => crate::model_config::get_heavy_models(),
        };'''

HEAVY_NEW = '''        // Verification is the last gate before mission authorization: serve
        // it from the heavy tier. A lightweight verifier has been observed
        // rubber-stamping halt text that contradicts the source in evidence,
        // and the NVIDIA fallback tier is unreachable from this host, which
        // silently drops the cascade to its weakest member.
        // To restore Router routing, replace the line below with the former
        // `match nexus_algorithms::Router::select("verify") { ... }` block.
        let models = crate::model_config::get_heavy_models();
        eprintln!("[VERIFIER] model cascade: {:?}", models);'''

# ---- 5. source-inspection guard (Corrected Anchor) -----------------------
TEST_ANCHOR = '''    #[tokio::test]
    async fn combined_mission_root_context_cannot_contaminate_node_strategy() {'''

TEST_INSERT = '''    // Source-inspection guard 
    // The verifier's claim-check depends on BOTH the untruncated halt text
    // and the grounding instruction; losing either silently reverts the
    // verifier to rubber-stamping.
    #[test]
    fn verifier_claim_check_needs_full_claims_and_grounded_quotes() {
        let source = include_str!("epistemic_engine.rs");
        assert!(
            source.contains("action.chars().take(6000)"),
            "halt claims must not be truncated at 6000 chars"
        );
        assert!(
            source.contains("quote the exact source lines from EVIDENCE"),
            "claim-check must require grounded quotes before the verdict"
        );
        assert!(
            source.contains("eprintln!(\\"[VERIFIER] model cascade"),
            "verify model cascade must be logged"
        );
    }

''' + TEST_ANCHOR

# ====================== targets ==========================================
targets = [
    ("ACT_CAP",  EP, "epistemic_engine: untruncated halt claims (2000->6000)",
     ACT_CAP_OLD, ACT_CAP_NEW,
     "action.chars().take(6000)", "action.chars().take(6000)",
     "truncated_action", 6),
    ("QUOTE",    EP, "epistemic_engine: grounded quote-before-decide clause",
     QUOTE_OLD, QUOTE_NEW,
     "quote the exact source lines from EVIDENCE",
     "quote the exact source lines from EVIDENCE",
     "name the contradicted claim", 6),
    ("SCHEMA",   EP, "epistemic_engine: CoT schema reordering (rationale first)",
     SCHEMA_OLD, SCHEMA_NEW,
     '"rationale": "<reasoning and exact source quotes>"',
     '"rationale": "<reasoning and exact source quotes>"',
     'evidence_confidence', 3),
    ("HEAVY",    EP, "epistemic_engine: verify route pinned to heavy tier",
     HEAVY_OLD, HEAVY_NEW,
     'eprintln!("[VERIFIER] model cascade', 'eprintln!("[VERIFIER] model cascade',
     'Router::select("verify")', 10),
    ("GUARD",    EP, "epistemic_engine: source-inspection guard test",
     TEST_ANCHOR, TEST_INSERT,
     "fn verifier_claim_check_needs_full_claims_and_grounded_quotes",
     "fn verifier_claim_check_needs_full_claims_and_grounded_quotes",
     "combined_mission_root_context_cannot_contaminate_node_strategy", 5),
]

def dump_region(rel, probe, span):
    lines = files[rel][1].splitlines()
    idx = next((i for i, ln in enumerate(lines) if probe in ln), None)
    if idx is None:
        print(f"    [REGION DUMP FAILED] probe not found in {rel}: {probe!r}")
        return
    lo, hi = max(0, idx - span), min(len(lines), idx + span)
    print(f"    [REGION] {rel} around line {idx + 1} (probe: {probe!r}):")
    for i in range(lo, hi):
        print(f"      {i + 1:5d} | {lines[i]}")

plan = []
failures = []

for key, rel, label, old, new, already, final_probe, region_probe, span in targets:
    text = patched[rel]
    old_count = text.count(old)
    if already in text:
        status = "SKIP (already present)"
        plan.append((key, rel, None))
    elif old_count == 1:
        status = "APPLY"
        plan.append((key, rel, (old, new)))
    elif old_count > 1:
        status = f"FAIL (anchor x{old_count}, want exactly 1)"
        failures.append((key, rel, label, region_probe, span))
        plan.append((key, rel, None))
    else:
        status = "FAIL (anchor not found)"
        failures.append((key, rel, label, region_probe, span))
        plan.append((key, rel, None))
    print(f"[STATUS] {key:7s} {label:56s} : {status}")

for rel, label, text in (
    (EP, "claim-check sentence (1108)", "Reject a halt that only claims the work is done."),
    (EP, "verifier math-prove criterion", "Does this evidence mathematically prove"),
    (EP, "sanitize_json_escapes", "sanitize_json_escapes("),
    (EP, "root-context contamination regression", "fn combined_mission_root_context_cannot_contaminate_node_strategy"),
):
    if text not in files[rel][1]:
        print(f"[INVARIANT VIOLATED] {label} is absent in {rel}")
        failures.append(("INVARIANT", rel, label, text, 5))

if failures:
    print()
    print("[DIAGNOSTIC] region dumps for failed targets:")
    for key, rel, label, region_probe, span in failures:
        print(f"  -- {key}: {label}")
        dump_region(rel, region_probe, span)
    print()
    print("[PREFLIGHT FAILED] ZERO FILES MODIFIED.")
    sys.exit(1)

# ------------------------- survey (read-only) -----------------------------
print()
print("[SURVEY] follow-up regions (read-only):")
dump_region(EP, "MIDDLE EVIDENCE COMPRESSED", 12)
dump_region(EP, "fn safe_api_call", 25)

# ------------------------- transformations -------------------------------
changed = []
for key, rel, edit in plan:
    if edit is None:
        continue
    old, new = edit
    patched[rel] = patched[rel].replace(old, new, 1)
    changed.append((key, rel))

if not changed:
    print()
    print("[NO-OP] every target is already in its final form. Nothing to write.")
    sys.exit(0)

for key, rel, label, old, new, already, final_probe, region_probe, span in targets:
    if final_probe not in patched[rel]:
        print(f"[POST-PATCH FAILED] {label}: final probe absent: {final_probe!r}")
        sys.exit(1)
if HEAVY_OLD in patched[EP]:
    print("[POST-PATCH FAILED] HEAVY: verify route match block still present")
    sys.exit(1)

print()
print(f"[POST-PATCH OK] {len(changed)} target(s) transformed: {', '.join(k for k, _ in changed)}")

print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
sys.stdout.writelines(
    difflib.unified_diff(
        files[EP][1].splitlines(keepends=True),
        patched[EP].splitlines(keepends=True),
        fromfile=f"a/{EP}",
        tofile=f"b/{EP}",
    )
)

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

files[EP][0].write_text(patched[EP], encoding="utf-8")
print(f"[PATCH_OK] {EP}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete. Review the status lines, survey, and diff above."
else
    echo "[OK] verifier hardening applied."
    echo ">> Targeted tests:"
    echo "     cargo test -p nexus_runtime_core --lib epistemic_engine"
    echo ">> Full suite:"
    echo "     cargo test -p nexus_runtime_core"
fi
