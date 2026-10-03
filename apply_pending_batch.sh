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
echo "=== pending batch (RULE 6 threshold/FIX, fallbacks, overrides, veto wording, parse fix) ==="
echo "=== root: $WORKSPACE_ROOT | dry-run: $DRY_RUN ==="

EP="crates/runtime_core/src/epistemic_engine.rs"
CL="crates/runtime_core/src/cortical_loop.rs"
AI="crates/runtime_core/src/action_intercept.rs"
CE="crates/runtime_core/src/completion_engine.rs"
for f in "$EP" "$CL" "$AI" "$CE"; do
    if [[ ! -f "$WORKSPACE_ROOT/$f" ]]; then
        echo "[PREFLIGHT FAILED] missing $f" >&2
        exit 1
    fi
done

NEXUS_ROOT="$WORKSPACE_ROOT" \
NEXUS_DRY="$($DRY_RUN && echo 1 || echo 0)" \
EP_REL="$EP" CL_REL="$CL" AI_REL="$AI" CE_REL="$CE" \
python3 - <<'PYEOF'
import difflib, os, pathlib, sys

root = pathlib.Path(os.environ["NEXUS_ROOT"])
dry  = os.environ["NEXUS_DRY"] == "1"
EP, CL, AI, CE = (os.environ[k] for k in ("EP_REL", "CL_REL", "AI_REL", "CE_REL"))

files = {rel: (root / rel, (root / rel).read_text(encoding="utf-8")) for rel in (EP, CL, AI, CE)}
s_ep, s_cl, s_ai, s_ce = (files[r][1] for r in (EP, CL, AI, CE))

def step(label, status):
    print(f"[STATUS] {label:56s} : {status}")

print()
print("[PRE-AUDIT] post-check probes verify INSERTION only (text presence in real")
print("            code). All presence-test probes are fragment-assembled at runtime")
print("            so no test can satisfy itself. Behavior is proven by the stress")
print("            run, not by these checks. Every count below is derived from the")
print("            plan itself (applied-subtarget tracking), never hard-coded ahead.")

# =====================================================================
# EP-1: RULE 6 threshold + FIX-claim simulation
# =====================================================================
R6_OLD = "VETO with a negative confidence_modifier and name the contradicted claim."
R6_NEW = ("VETO with a confidence_modifier of -0.30 or lower (the execution veto threshold "
          "is -0.25, so a merely negative value such as -0.10 will NOT block execution) and "
          "name the contradicted claim. If the FIX section proposes a change, apply it "
          "mentally to the case in the ORIGINAL DIRECTIVE and state the result; if the "
          "answer's stated outcome does not follow, VETO.")

if "confidence_modifier of -0.30 or lower" in s_ep:
    step("EP-1 RULE 6 threshold + FIX simulation", "SKIP (already present)")
elif s_ep.count(R6_OLD) == 1:
    s_ep = s_ep.replace(R6_OLD, R6_NEW, 1)
    step("EP-1 RULE 6 threshold + FIX simulation", "APPLY")
else:
    step("EP-1 RULE 6 threshold + FIX simulation", f"FAIL (anchor count={s_ep.count(R6_OLD)})")
    sys.exit(1)

# =====================================================================
# EP-2: RULE 6.1 (deliberate-is-not-compliant)
# =====================================================================
R61_ANCHOR = '  OUTPUT ONLY VALID JSON: {{"critique": "", "confidence_modifier": <float from'
R61_TEXT = (
    "CRITICAL RULE 6.1 [DELIBERATE IS NOT COMPLIANT]: You may NOT dismiss a finding merely "
    "because the flagged behavior looks deliberate, consistent, or strict. Deliberate design "
    "is not the same as contract compliance: a defect exists when behavior violates an "
    "identifiable requirement, such as an enum state that can never be produced, a documented "
    "invariant, or an inconsistent classification of the same evidence. Claiming that no "
    "defect exists is itself an absolute claim -- back it with source lines that prove the "
    "invariant, or do not veto on that basis.\n"
)

if "DELIBERATE IS NOT COMPLIANT" in s_ep:
    step("EP-2 RULE 6.1 deliberate-is-not-compliant", "SKIP (already present)")
elif s_ep.count(R61_ANCHOR) == 1:
    s_ep = s_ep.replace(R61_ANCHOR, R61_TEXT + R61_ANCHOR, 1)
    step("EP-2 RULE 6.1 deliberate-is-not-compliant", "APPLY")
else:
    step("EP-2 RULE 6.1 deliberate-is-not-compliant", f"FAIL (anchor count={s_ep.count(R61_ANCHOR)})")
    sys.exit(1)

# =====================================================================
# EP-3: fail-closed critic fallback + evaluator fallback tag (one step)
# =====================================================================
FB_OLD = '''            Err(_) => serde_json::json!({
                "critique": clean_text.trim(),
                "confidence_modifier": 0.0
            }),'''

FB_CRITIC = r'''            Err(_) => {
                // FAIL-CLOSED for halt proposals: a non-JSON critique must not
                // become a silent 0.0 approval. Non-halt actions keep the
                // permissive fallback; post-action evaluation still gates them.
                let halt_like = proposed_action.contains("\"halt\"");
                let modifier = if halt_like { -0.30 } else { 0.0 };
                serde_json::json!({
                    "critique": format!("[CRITIC_UNPARSEABLE] Raw critique retained.\n{}", clean_text.trim()),
                    "confidence_modifier": modifier
                })
            },'''

FB_EVAL = r'''            Err(_) => serde_json::json!({
                "critique": format!("[EVALUATOR_UNPARSEABLE] Raw reply retained.\n{}", clean_text.trim()),
                "confidence_modifier": 0.0
            }),'''

critic_done = "[CRITIC_UNPARSEABLE]" in s_ep
eval_done = "[EVALUATOR_UNPARSEABLE]" in s_ep
fb_count = s_ep.count(FB_OLD)
if critic_done and eval_done:
    step("EP-3a critic fail-closed fallback", "SKIP (already present)")
    step("EP-3b evaluator fallback tag", "SKIP (already present)")
elif critic_done and fb_count == 1:
    s_ep = s_ep.replace(FB_OLD, FB_EVAL, 1)
    step("EP-3a critic fail-closed fallback", "SKIP (already present)")
    step("EP-3b evaluator fallback tag", "APPLY")
elif not critic_done and fb_count == 2:
    assert s_ep.index(FB_OLD) < s_ep.index("pub async fn evaluate_evidence"), "first block must be the critic's"
    s_ep = s_ep.replace(FB_OLD, FB_CRITIC, 1)
    s_ep = s_ep.replace(FB_OLD, FB_EVAL, 1)
    step("EP-3a critic fail-closed fallback", "APPLY")
    step("EP-3b evaluator fallback tag", "APPLY")
else:
    step("EP-3a/3b fallbacks", f"FAIL (unrecognized state: critic_done={critic_done} eval_done={eval_done} blocks={fb_count})")
    sys.exit(1)

# =====================================================================
# EP-4: fragment-built presence test for EP-1..EP-3
# =====================================================================
TEST_ANCHOR = "    #[tokio::test]\n    async fn compound_read_plus_search_code_pushes_search_first() {"
EP_TEST = r'''    #[test]
    fn pending_batch_prompts_and_guards_are_present() {
        // Unforgeable probes: assembled from fragments at runtime; this test
        // never contains the full strings it verifies.
        let source = include_str!("epistemic_engine.rs");
        let p1 = ["confidence_modifier of -0.30", " or lower"].concat();
        assert!(source.contains(p1.as_str()), "RULE 6 must name a workable veto threshold");
        let p2 = ["apply it mentally to the case", " in the ORIGINAL DIRECTIVE"].concat();
        assert!(source.contains(p2.as_str()), "RULE 6 must simulate FIX claims");
        let p3 = ["[CRITIC", "_UNPARSEABLE]"].concat();
        assert!(source.contains(p3.as_str()), "critic parse failure must be marked");
        let p4 = ["DELIBERATE IS NOT", " COMPLIANT"].concat();
        assert!(source.contains(p4.as_str()), "RULE 6.1 must forbid deliberate-is-compliant dismissal");
    }

'''

if "fn pending_batch_prompts_and_guards_are_present" in s_ep:
    step("EP-4 fragment presence test", "SKIP (already present)")
elif s_ep.count(TEST_ANCHOR) == 1:
    s_ep = s_ep.replace(TEST_ANCHOR, EP_TEST + TEST_ANCHOR, 1)
    step("EP-4 fragment presence test", "APPLY")
else:
    step("EP-4 fragment presence test", f"FAIL (anchor count={s_ep.count(TEST_ANCHOR)})")
    sys.exit(1)

# =====================================================================
# CL-1: FACT-OVERRIDE guards (count-derived post-check)
# =====================================================================
pre_guards = s_cl.count("critic_unparseable")
OV_TARGETS = [
    ("CL-1a search-override guard", "&& critic_veto_is_redundancy", "                    && !critic_text.contains(\"critic_unparseable\")\n"),
    ("CL-1b write-override guard",  "&& critic_veto_is_pre_recon",  "                    && !critic_text.contains(\"critic_unparseable\")\n                    && !critic_text.contains(\"contradict\")\n"),
    ("CL-1c noop-override guard",   "&& critic_claims_noop",        "                    && !critic_text.contains(\"critic_unparseable\")\n"),
]
ov_applied = 0
for label, anchor, insert in OV_TARGETS:
    n = s_cl.count(anchor)
    if n == 0:
        step(label, "SKIP (anchor absent -- drifted? send grep of override arms)")
        continue
    if n != 1:
        step(label, f"FAIL (anchor count={n})")
        sys.exit(1)
    s_cl = s_cl.replace(anchor, anchor + "\n" + insert.rstrip("\n"), 1)
    ov_applied += 1
    step(label, "APPLY")
step("CL-1 override guards inserted", f"{ov_applied}/3")

# =====================================================================
# CL-2: veto feedback -- forbid the ACTION, not the FINDING
# =====================================================================
VF_OLD = "The rejected proposal MUST NOT be repeated. Select a materially different, mission-compliant action."
VF_NEW = ("The rejected ACTION must not be repeated. Select a materially different, mission-compliant action. "
          "A finding from the rejected proposal that is factually correct and grounded in quoted source lines "
          "MAY be re-proposed in a materially different, better-evidenced form; do not abandon a correct finding "
          "merely because the veto disputed it.")

if "A finding from the rejected proposal" in s_cl:
    step("CL-2 veto-feedback finding-preservation wording", "SKIP (already present)")
elif s_cl.count(VF_OLD) == 1:
    s_cl = s_cl.replace(VF_OLD, VF_NEW, 1)
    step("CL-2 veto-feedback finding-preservation wording", "APPLY")
else:
    step("CL-2 veto-feedback finding-preservation wording", f"FAIL (anchor count={s_cl.count(VF_OLD)})")
    sys.exit(1)

# =====================================================================
# CL-3: parse-failure fix (shape-tolerant) + fragment guard
# =====================================================================
parse_state = "none"
if "!raw_proposal" in s_cl:
    step("CL-3a parse-failure fix", "SKIP (already present)")
    parse_state = "present"
else:
    ridx = [i for i, ln in enumerate(s_cl.splitlines()) if "Model declared task complete" in ln]
    if len(ridx) == 0:
        step("CL-3a parse-failure fix", "SKIP (no synthesis site found -- already removed?)")
        parse_state = "absent"
    else:
        lines = s_cl.splitlines(keepends=True)
        i = ridx[0]
        j = None
        for k in range(i, max(-1, i - 25), -1):
            if 'contains("halt")' in lines[k]:
                j = k
                break
        if j is not None:
            old_line = lines[j].rstrip("\n")
            head = old_line.rstrip().rstrip("{").rstrip()
            lines[j] = head + ' && !raw_proposal.contains("\\"type\\"") {\n'
            s_cl = "".join(lines)
            step("CL-3a parse-failure fix", f"APPLY (condition guarded at line {j + 1})")
            parse_state = "applied"
        else:
            step("CL-3a parse-failure fix", "SKIP (shape unknown -- diagnosis below)")
            parse_state = "diagnosis"
            for j in range(max(0, i - 25), min(len(lines), i + 12)):
                print(f"      {j + 1:5d} | {lines[j].rstrip()}")
            print("      [paste this region back and the fix will be retargeted]")

GUARD_OLD_SHAPE = 'source.contains("Synthesize a halt ONLY from prose-only output")'
CL_GUARD = r'''    #[test]
    fn parse_failure_never_manufactures_a_typed_halt() {
        // Unforgeable probe: fragments are split, so this test never contains
        // the full string it verifies. The fix's own comment mentions the
        // prose-only phrase, so the probe targets the guard CONDITION instead.
        let source = include_str!("cortical_loop.rs");
        let probe = ["!raw_proposal", ".contains("].concat();
        assert!(
            source.contains(probe.as_str()),
            "typed parse failures must fall through to schema-hint recovery; never synthesize a halt"
        );
    }

'''

if "fn parse_failure_never_manufactures_a_typed_halt" in s_cl:
    if GUARD_OLD_SHAPE in s_cl:
        s_cl = s_cl.replace(GUARD_OLD_SHAPE, 'source.contains(["!raw_proposal", ".contains("].concat().as_str())', 1)
        step("CL-3b parse-failure guard", "APPLY (converted to fragment probe)")
    else:
        step("CL-3b parse-failure guard", "SKIP (already present)")
elif parse_state == "diagnosis":
    step("CL-3b parse-failure guard", "SKIP (deferred until CL-3a shape is known)")
else:
    n = s_cl.count("mod tests {")
    if n != 1:
        step("CL-3b parse-failure guard", f"FAIL (mod tests count={n})")
        sys.exit(1)
    s_cl = s_cl.replace("mod tests {", "mod tests {\n" + CL_GUARD, 1)
    step("CL-3b parse-failure guard", "APPLY (inserted at tests module head)")

# =====================================================================
# OPT-1/OPT-2: warning cleanups
# =====================================================================
if "let _lower = text.to_lowercase();" in s_ai:
    step("OPT-1 action_intercept unused `lower`", "SKIP (already present)")
elif s_ai.count("let lower = text.to_lowercase();") == 1:
    s_ai = s_ai.replace("let lower = text.to_lowercase();", "let _lower = text.to_lowercase();", 1)
    step("OPT-1 action_intercept unused `lower`", "APPLY")
else:
    step("OPT-1 action_intercept unused `lower`", f"SKIP (count={s_ai.count('let lower = text.to_lowercase();')})")

if "#[allow(dead_code)]\n    pub lineage" in s_ce:
    step("OPT-2 completion_engine dead `lineage` field", "SKIP (already present)")
elif s_ce.count("    pub lineage: &'a str,") == 1:
    s_ce = s_ce.replace("    pub lineage: &'a str,", "    #[allow(dead_code)]\n    pub lineage: &'a str,", 1)
    step("OPT-2 completion_engine dead `lineage` field", "APPLY")
else:
    step("OPT-2 completion_engine dead `lineage` field", f"SKIP (count={s_ce.count(chr(32) * 4 + 'pub lineage')})")

# =====================================================================
# post checks -- every count derived from the plan above
# =====================================================================
assert s_ep.count("confidence_modifier of -0.30 or lower") == 1, "EP-1 probe count must be 1"
assert s_ep.count("apply it mentally to the case in the ORIGINAL DIRECTIVE") == 1, "EP-1 FIX-sim probe count must be 1"
assert s_ep.count("[CRITIC_UNPARSEABLE]") == 1, "EP-3a marker count must be 1"
assert s_ep.count("[EVALUATOR_UNPARSEABLE]") == 1, "EP-3b marker count must be 1"
assert s_ep.count("DELIBERATE IS NOT COMPLIANT") == 1, "EP-2 probe count must be 1"
assert s_ep.count("fn pending_batch_prompts_and_guards_are_present") == 1
assert s_cl.count("critic_unparseable") == pre_guards + ov_applied, "CL-1 guard count must equal pre-existing + applied"
assert s_cl.count("A finding from the rejected proposal") == 1
if parse_state in ("present", "applied"):
    assert "!raw_proposal" in s_cl, "CL-3a condition missing despite reported state"
if parse_state == "applied":
    assert s_cl.count("fn parse_failure_never_manufactures_a_typed_halt") == 1
print("[POST-PATCH OK] all counts derived from the plan; probes vacuity-audited")

# =====================================================================
# diff + write
# =====================================================================
print()
print("=" * 60)
print(" PROPOSED DIFF")
print("=" * 60)
changed = []
for rel, after in ((EP, s_ep), (CL, s_cl), (AI, s_ai), (CE, s_ce)):
    before = files[rel][1]
    if after != before:
        print("".join(difflib.unified_diff(before.splitlines(keepends=True), after.splitlines(keepends=True),
                                          fromfile=f"a/{rel}", tofile=f"b/{rel}")))
        changed.append(rel)

if dry:
    print()
    print("[DRY-RUN] ZERO FILES WRITTEN.")
    print("[DRY-RUN] Re-run without --dry-run to apply exactly this diff.")
    sys.exit(0)

for rel, after in ((EP, s_ep), (CL, s_cl), (AI, s_ai), (CE, s_ce)):
    if after != files[rel][1]:
        files[rel][0].write_text(after, encoding="utf-8")
        print(f"[PATCH_OK] {rel}")
PYEOF

echo
if $DRY_RUN; then
    echo "[DRY-RUN] complete. Review status lines and diff above."
else
    echo "[OK] pending batch applied."
    echo ">> Targeted tests:"
    echo "     cargo test -p nexus_runtime_core --lib"
    echo ">> Full suite:"
    echo "     cargo test -p nexus_runtime_core"
fi
