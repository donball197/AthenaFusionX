#!/usr/bin/env bash
# patch_critic_citation_grounding.sh — stop critic from inventing line ranges.
#
# B1: Extend CRITIC_GROUNDING_PREAMBLE (exact const at epistemic_engine.rs ~1553)
# B2: In-band [CRITIC CITATION DISCIPLINE] before criticize_proposal
# B3: sanitize_ungrounded_citation_veto on LLM veto text when spans absent from evidence
#
# Usage:
#   ./patch_critic_citation_grounding.sh
#   ./patch_critic_citation_grounding.sh --apply
#   ./patch_critic_citation_grounding.sh --apply --full

set -euo pipefail

ROOT="$(pwd)"
ENGINE="$ROOT/crates/runtime_core/src/epistemic_engine.rs"
CRITIC="$ROOT/crates/runtime_core/src/cortical_loop/critic.rs"
APPLY=0
FULL=0

for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    --full)  FULL=1 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

for f in "$ENGINE" "$CRITIC"; do
  [[ -f "$f" ]] || { echo "ERROR: $f not found (run from repo root)" >&2; exit 1; }
done

STAMP="$(date +%Y%m%d-%H%M%S)"
cp "$ENGINE" "$ENGINE.bak.$STAMP"
cp "$CRITIC" "$CRITIC.bak.$STAMP"
echo "backups:"
echo "  $ENGINE.bak.$STAMP"
echo "  $CRITIC.bak.$STAMP"

PATCH_EN="$(mktemp)"
PATCH_CR="$(mktemp)"
trap 'rm -f "$PATCH_EN" "$PATCH_CR"' EXIT

python3 - "$ENGINE" "$PATCH_EN" "$CRITIC" "$PATCH_CR" <<'PY'
import sys

en_in, en_out, cr_in, cr_out = sys.argv[1:5]

def die(msg):
    sys.stderr.write(msg + "\n")
    sys.exit(3)

def expect_one(src, needle, label):
    n = src.count(needle)
    if n != 1:
        die(f"ERROR[{label}]: expected 1 match, found {n}\n  needle[:120]={needle[:120]!r}")

# ═══════════════════════════════════════════════════════════
# B1 — exact preamble body (from tree at 1553)
# ═══════════════════════════════════════════════════════════
src = open(en_in, encoding="utf-8").read()

if "CITATION DISCIPLINE (MANDATORY)" in src:
    die("ERROR[EN]: citation block already present — already patched?")

old_preamble = '''pub const CRITIC_GROUNDING_PREAMBLE: &str = r#"
[GROUNDING]
You are the Adversarial Critic for the CURRENT mission only.
- Evaluate ONLY the proposal against the ORIGINAL DIRECTIVE and the EVIDENCE block below.
- IGNORE any prior missions, calibration, video-recording, or buffer-size topics unless they appear in the current directive or evidence.
- Do NOT invent quantitative claims. If the Strategist makes a quantitative claim that is not backed by a [Measured] line in EVIDENCE, veto it.
- Prefer constructive vetoes that name the exact next tool call.
"#;'''

new_preamble = '''pub const CRITIC_GROUNDING_PREAMBLE: &str = r#"
[GROUNDING]
You are the Adversarial Critic for the CURRENT mission only.
- Evaluate ONLY the proposal against the ORIGINAL DIRECTIVE and the EVIDENCE block below.
- IGNORE any prior missions, calibration, video-recording, or buffer-size topics unless they appear in the current directive or evidence.
- Do NOT invent quantitative claims. If the Strategist makes a quantitative claim that is not backed by a [Measured] line in EVIDENCE, veto it.
- Prefer constructive vetoes that name the exact next tool call.

CITATION DISCIPLINE (MANDATORY):
- Assert line numbers or symbol spans ONLY when those exact numbers are visible in a READ_OK or READ_CHUNK_OK body already in evidence. Count from that body; do not estimate from memory.
- If a span is not visible, write unverified. Never invent lines N-M and never fill a replacement report with guessed ranges.
- When rejecting a halt or report for bad citations, name the unsupported claim and demand re-derivation from the existing READ_OK body. Do not hand the strategist a full replacement body unless every line number in it appears in evidence.
"#;'''

expect_one(src, old_preamble, "EN-preamble-exact")
src = src.replace(old_preamble, new_preamble, 1)
open(en_out, "w", encoding="utf-8").write(src)

# ═══════════════════════════════════════════════════════════
# B2 + B3 — critic.rs
# ═══════════════════════════════════════════════════════════
src = open(cr_in, encoding="utf-8").read()

if "sanitize_ungrounded_citation_veto" in src:
    die("ERROR[CR]: sanitize helper already present — already patched?")

# B2: after supersession_note (correction-cycle patch) or after contract_facts
note = '''        let citation_note = concat!(
            "\\n[CRITIC CITATION DISCIPLINE] ",
            "Assert line numbers or symbol spans ONLY when those exact numbers ",
            "are visible in a READ_OK / READ_CHUNK_OK body already in evidence. ",
            "If a span is not visible, say unverified. Do NOT invent lines N-M ",
            "and do NOT hand the strategist a full replacement report filled with ",
            "guessed line ranges. Demand re-derivation from the traced source body.\\n",
        );
        let critic_evidence = format!("{}{}", critic_evidence, citation_note);
'''

anchor_super = (
    '        let critic_evidence = format!("{}{}", critic_evidence, supersession_note);\n'
    '        match EpistemicEngine::criticize_proposal(\n'
)
anchor_facts = (
    '            contract_facts(&session.contract.conditions, &session.local_evidence)\n'
    '        );\n'
    '        match EpistemicEngine::criticize_proposal(\n'
)

if src.count(anchor_super) == 1:
    src = src.replace(
        anchor_super,
        '        let critic_evidence = format!("{}{}", critic_evidence, supersession_note);\n'
        + note
        + '        match EpistemicEngine::criticize_proposal(\n',
        1,
    )
elif src.count(anchor_facts) == 1:
    src = src.replace(
        anchor_facts,
        '            contract_facts(&session.contract.conditions, &session.local_evidence)\n'
        '        );\n'
        + note
        + '        match EpistemicEngine::criticize_proposal(\n',
        1,
    )
else:
    die("ERROR[CR-note]: no supersession_note/contract_facts anchor before criticize_proposal")

helpers = r'''
fn sanitize_ungrounded_citation_veto(verdict: &str, evidence: &str) -> String {
    let spans = regex_lite_spans(verdict);
    if spans.is_empty() {
        return verdict.to_string();
    }
    if spans.iter().all(|span| evidence.contains(span)) {
        return verdict.to_string();
    }
    let mut out = String::from(verdict);
    out.push_str(
        "\n[CITATION_GUARD] One or more prescribed line ranges in this veto do not \
appear in local evidence. Ignore those numbers. Re-derive symbol spans from the \
existing READ_OK / READ_CHUNK_OK body and rewrite the report from that body only. \
Do not trust invented line ranges.",
    );
    out
}

fn regex_lite_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < n && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let a = &text[start..i];
            if i < n && bytes[i] == b'-' {
                let j = i + 1;
                let mut k = j;
                while k < n && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                if k > j {
                    let ctx_from = start.saturating_sub(12);
                    let ctx = text[ctx_from..k].to_ascii_lowercase();
                    if ctx.contains("line") {
                        spans.push(format!("{}-{}", a, &text[j..k]));
                    }
                    i = k;
                    continue;
                }
            }
        }
        i += 1;
    }
    spans
}

'''

needle = "fn patch_signature_needle(path: &str, name: &str) -> String {\n"
expect_one(src, needle, "CR-helpers-anchor")
src = src.replace(needle, helpers + needle, 1)

# B3: LLM reject path — unique assignment from correction-cycle patch
llm_assign = (
    "                    session.last_critic_rejected_actions = Some(rejected_action_keys);\n"
    "                    session.last_critic_rejected_at_evidence_len =\n"
    "                        Some(session.local_evidence.len());\n"
)
expect_one(src, llm_assign, "CR-llm-assign")

# Insert a sanitize of the human-readable reject string that is fed back.
# In this tree the next feedback push often uses a local like `reject_msg` /
# `verdict` / `msg`. We also sanitize any immediate format! that still uses
# rejected keys only for identity; the textual veto body is what matters.
#
# Conservative, compile-safe approach: after the assignment, bind
# `guarded_reject_feedback` only if we find a known pattern. Otherwise leave
# a documented call site comment is NOT enough — scan for the LLM-path
# push_str that follows within ~40 lines.

pos = src.find(llm_assign)
assert pos >= 0
tail = src[pos + len(llm_assign) : pos + len(llm_assign) + 3500]

# Patterns seen across revisions for the textual veto body variable name.
import re
wired = False
# Match: session.local_evidence.push_str(&format!( "\n[CRITIC_VETO] {} ...", VAR ));
pat = re.compile(
    r'(session\.local_evidence\.push_str\(&format!\(\s*\n'
    r'(\s*)("(?:\\.|[^"\\])*\[CRITIC_VETO\](?:\\.|[^"\\])*")\s*,\s*\n'
    r'(\s*)([A-Za-z_][A-Za-z0-9_]*)\s*\n'
    r'(\s*)\)\);)',
    re.MULTILINE,
)
m = pat.search(tail)
if m:
    full, indent, fmt, ind2, var, ind3 = m.group(0), m.group(2), m.group(3), m.group(4), m.group(5), m.group(6)
    # Do not wrap action-key-only paths; only when format mentions CRITIC_VETO (already required).
    replacement = (
        f"let guarded_verdict = sanitize_ungrounded_citation_veto(&{var}, &session.local_evidence);\n"
        f"{indent}session.local_evidence.push_str(&format!(\n"
        f"{indent}    {fmt},\n"
        f"{indent}    guarded_verdict\n"
        f"{indent}));"
    )
    # Keep indentation of the inserted let aligned with surrounding block (20 spaces typical).
    # Use the indent of the push_str line.
    let_indent = indent
    replacement = (
        f"{let_indent}let guarded_verdict = sanitize_ungrounded_citation_veto(&{var}, &session.local_evidence);\n"
        f"{let_indent}session.local_evidence.push_str(&format!(\n"
        f"{let_indent}    {fmt},\n"
        f"{let_indent}    guarded_verdict\n"
        f"{let_indent}));"
    )
    tail2 = tail[: m.start()] + replacement + tail[m.end() :]
    src = src[: pos + len(llm_assign)] + tail2 + src[pos + len(llm_assign) + len(tail) :]
    wired = True
    print(f"wired sanitize on LLM veto var `{var}`")
else:
    print(
        "WARN: could not auto-wire LLM CRITIC_VETO push; helpers + B1/B2 still apply.\n"
        "      After rejected_action_keys assignment, wrap the veto body with:\n"
        "      sanitize_ungrounded_citation_veto(&VERDICT, &session.local_evidence)"
    )

open(cr_out, "w", encoding="utf-8").write(src)
print("python transforms completed")
PY

echo
echo "===== diff: epistemic_engine.rs ====="
diff -u "$ENGINE" "$PATCH_EN" || true
echo "====================================="
echo
echo "===== diff: cortical_loop/critic.rs ====="
diff -u "$CRITIC" "$PATCH_CR" || true
echo "========================================="
echo

# Hard guardrails
if grep -n 'CITATION DISCIPLINE' "$PATCH_EN" | grep -E 'command_only_mission|Execute cargo check|tokio::test' >/dev/null 2>&1; then
  echo "ERROR: citation block leaked into tests" >&2
  exit 1
fi
if ! grep -q 'CITATION DISCIPLINE (MANDATORY)' "$PATCH_EN"; then
  echo "ERROR: preamble citation block missing" >&2
  exit 1
fi
if ! grep -q 'concat!' "$PATCH_CR"; then
  echo "ERROR: citation_note concat! missing" >&2
  exit 1
fi
# Preamble-only hunk check: test module marker must be unchanged relative to presence
if ! grep -q 'Prefer constructive vetoes that name the exact next tool call\.' "$PATCH_EN"; then
  echo "ERROR: original preamble bullet missing after transform" >&2
  exit 1
fi

if [[ "$APPLY" -ne 1 ]]; then
  echo "dry-run only. re-run with --apply to install."
  exit 0
fi

install -m 0644 "$PATCH_EN" "$ENGINE"
install -m 0644 "$PATCH_CR" "$CRITIC"
echo "patched both files"

rollback() {
  echo >&2
  echo "!! verification failed; rolling back" >&2
  cp "$ENGINE.bak.$STAMP" "$ENGINE"
  cp "$CRITIC.bak.$STAMP" "$CRITIC"
  exit 1
}
trap 'rollback' ERR

echo ">> cargo check -p nexus_runtime_core ..."
cargo check -p nexus_runtime_core

echo ">> cargo test -p nexus_runtime_core --lib 'cortical_loop::critic' ..."
cargo test -p nexus_runtime_core --lib 'cortical_loop::critic'

echo ">> cargo test -p nexus_runtime_core --lib 'command_only_mission_still_prefers_execute_command' ..."
cargo test -p nexus_runtime_core --lib 'command_only_mission_still_prefers_execute_command'

if [[ "$FULL" -eq 1 ]]; then
  echo ">> cargo test -p nexus_runtime_core --lib ..."
  cargo test -p nexus_runtime_core --lib
fi

trap - ERR
echo
echo "OK: citation grounding patch applied and verified."
echo "Next: cargo fmt -p nexus_runtime_core"
