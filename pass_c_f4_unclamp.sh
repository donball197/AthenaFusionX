#!/usr/bin/env bash
# pass_c_f4_unclamp.sh — F4: REMOVE THE FLOOR CLAMP (Pass C of 3).
#
# The single behavior change of the A/B/C series:
#
#   C1  decay_hypothesis_belief_with_floor -> decay_hypothesis_belief:
#       body delegates to graph.decay(); the clamp
#       `if pre >= FLOOR && post < FLOOR { FLOOR } else { post }`
#       and `const FLOOR` are removed. Doc rewritten to match.
#   C2  call-site renames: 2x Self::, 1x CorticalLoop:: (test).
#   C3  test updates:
#       - clamps_at_threshold -> crosses_selection_floor, expectation
#         0.15 -> (0.30_f32 - 0.20_f32).max(0.0)   [bit-exact vs decay]
#       - leaves_dead_hypotheses_alone renamed (old fn name is gone)
#
# DELIBERATELY UNCHANGED (must stay green):
#   - floor_hypothesis_belief_after_failure + its 2 tests (rescue path)
#   - cognition_graph.rs (checksum-verified)
#
# Dry run:  ./pass_c_f4_unclamp.sh
# Apply:    APPLY=1 ./pass_c_f4_unclamp.sh

set -euo pipefail

[[ $# -le 1 ]] || {
    echo "usage: $0 [REPO_ROOT]" >&2
    exit 2
}

command -v python3 >/dev/null || {
    echo "ERROR: python3 required." >&2
    exit 2
}

REPO_ROOT="${1:-${REPO_ROOT:-$HOME/AthenaFusionX}}"
[[ -d "$REPO_ROOT" ]] || {
    echo "ERROR: not a directory: $REPO_ROOT" >&2
    exit 2
}

export APPLY="${APPLY:-0}"

python3 - "$REPO_ROOT" <<'PYEOF'
import difflib
import hashlib
import os
import pathlib
import shutil
import subprocess
import sys
import time

root = pathlib.Path(sys.argv[1])
APPLY = os.environ.get("APPLY", "0") == "1"
TS = time.strftime("%Y%m%d-%H%M%S")

L_REL = "crates/runtime_core/src/cortical_loop.rs"
G_REL = "crates/runtime_core/src/cognition_graph.rs"

OLD_FN = "fn decay_hypothesis_belief_with_floor("
NEW_FN = "fn decay_hypothesis_belief("


def die(msg):
    print("  [abort] %s" % msg)
    print("  [abort] nothing written.")
    sys.exit(1)


def ok(msg):
    print("  [ok]   %s" % msg)


def skip(msg):
    print("  [skip] %s" % msg)


lp = root / L_REL
gp = root / G_REL

if not lp.is_file():
    die("target not found: %s" % lp)
if not gp.is_file():
    die("graph file missing: %s" % gp)

src0 = lp.read_text(encoding="utf-8")
gsha_before = hashlib.sha256(gp.read_bytes()).hexdigest()
lines = src0.split("\n")


# ------------------------------------------------- preconditions
print("=== PRECONDITIONS ===")

gsrc = gp.read_text(encoding="utf-8")
if "pub fn decay(" not in gsrc:
    die("graph.decay missing — Pass A not applied")
ok("graph.decay present (Pass A applied)")

if OLD_FN in src0:
    ok("F4 not yet applied (clamped helper present)")
elif NEW_FN in src0:
    skip("F4 already applied")
else:
    die("neither helper signature found — verify manually")


# ------------------------------------------------- engine
def seq_hits(seq):
    out = []
    n = len(seq)
    for i in range(0, len(lines) - n + 1):
        if all(lines[i + k].strip() == seq[k] for k in range(n)):
            out.append(i)
    return out


def replace_block(tag, old_seq, new_seq):
    if not seq_hits(old_seq) and seq_hits(new_seq):
        skip(tag + " (already applied)")
        return
    hits = seq_hits(old_seq)
    if len(hits) != 1:
        print("  [diag] %s: expected 1 match, found %d" % (tag, len(hits)))
        for i in hits[:5]:
            print("       start line %d: %s" % (i + 1, lines[i]))
        die("%s: block match ambiguity" % tag)
    i = hits[0]
    base = len(lines[i]) - len(lines[i].lstrip())
    out = [" " * base + t for t in new_seq]
    lines[i:i + len(old_seq)] = out
    ok("%s (lines %d..%d)" % (tag, i + 1, i + len(old_seq)))


def replace_line(tag, old, new, count):
    if not any(l.strip() == old for l in lines) and any(l.strip() == new for l in lines):
        skip(tag + " (already applied)")
        return
    hits = [i for i, l in enumerate(lines) if l.strip() == old]
    if len(hits) != count:
        die("%s: expected %d exact line(s), found %d" % (tag, count, len(hits)))
    for i in hits:
        base = len(lines[i]) - len(lines[i].lstrip())
        lines[i] = " " * base + new
    ok("%s (x%d)" % (tag, count))


def replace_token(tag, old, new, count):
    joined = "\n".join(lines)
    if old not in joined and new in joined:
        skip(tag + " (already applied)")
        return
    n = joined.count(old)
    if n != count:
        die("%s: expected %d token occurrence(s), found %d" % (tag, count, n))
    lines[:] = joined.replace(old, new).split("\n")
    ok("%s (x%d)" % (tag, count))


def replace_fn(tag, fn_marker, new_lines, done_probe=None):
    """Replace doc block + fn signature + body (brace-counted span)."""
    joined = "\n".join(lines)
    if done_probe and done_probe in joined:
        skip(tag + " (already applied)")
        return
    hits = [i for i, l in enumerate(lines) if fn_marker in l]
    if len(hits) != 1:
        die("%s: fn marker matched %dx" % (tag, len(hits)))
    start = hits[0]

    doc_start = start
    while doc_start > 0 and lines[doc_start - 1].strip().startswith("///"):
        doc_start -= 1

    depth = 0
    started = False
    end = -1
    for j in range(start, min(len(lines), start + 60)):
        depth += lines[j].count("{") - lines[j].count("}")
        if "{" in lines[j]:
            started = True
        if started and depth == 0:
            end = j
            break
    if end < 0:
        die("%s: fn body did not close within 60 lines" % tag)

    base = len(lines[doc_start]) - len(lines[doc_start].lstrip())
    out = [" " * (base + 4 * d) + t for d, t in new_lines]
    lines[doc_start:end + 1] = out
    ok("%s (lines %d..%d -> %d line(s))" % (tag, doc_start + 1, end + 1, len(out)))


# ------------------------------------------------- C3a
print()
print("=== [C3a] clamp test: rename + new expectation ===")
# The call line doubles as the disambiguator: the OTHER Some(0.15)
# assert belongs to floor_hypothesis_belief_after_failure and must
# stay untouched.
replace_block(
    "C3a crosses_selection_floor",
    [
        "CorticalLoop::decay_hypothesis_belief_with_floor(&mut session, h_id, 0.20);",
        "assert_eq!(session.graph.belief_of(h_id), Some(0.15));",
    ],
    [
        "CorticalLoop::decay_hypothesis_belief(&mut session, h_id, 0.20);",
        "assert_eq!(session.graph.belief_of(h_id), Some((0.30_f32 - 0.20_f32).max(0.0)));",
    ])


# ------------------------------------------------- C1
print()
print("=== [C1] unclamp the decay chokepoint ===")
NEW_BODY = [
    (0, "/// Apply a direct decay to the active hypothesis's belief."),
    (0, "///"),
    (0, "/// This is the chokepoint for direct belief subtraction"),
    (0, "/// (`belief - delta`) used by strategy-violation and anti-loop"),
    (0, "/// pivots."),
    (0, "///"),
    (0, "/// F4: no floor clamp is applied. The belief may cross SELECT_FLOOR"),
    (0, "/// and become unselectable, which is exactly what forced-pivot"),
    (0, "/// callers intend. Decay never resurrects: it only floors at 0.0."),
    (0, "/// The evaluation-failure rescue path remains"),
    (0, "/// `floor_hypothesis_belief_after_failure`."),
    (0, "fn decay_hypothesis_belief("),
    (1, "session: &mut EpistemicSession,"),
    (1, "h_id: uuid::Uuid,"),
    (1, "delta: f32,"),
    (0, ") {"),
    (1, "let _ = session.graph.decay(h_id, delta);"),
    (0, "}"),
]
replace_fn("C1 unclamped decay chokepoint", OLD_FN, NEW_BODY, done_probe=NEW_FN)


# ------------------------------------------------- C2
print()
print("=== [C2] call-site renames ===")
replace_line("C2a Self:: call sites",
             "Self::decay_hypothesis_belief_with_floor(session, h_id, 0.20);",
             "Self::decay_hypothesis_belief(session, h_id, 0.20);", 2)
replace_line("C2b remaining test call site",
             "CorticalLoop::decay_hypothesis_belief_with_floor(&mut session, h_id, 0.20);",
             "CorticalLoop::decay_hypothesis_belief(&mut session, h_id, 0.20);", 1)


# ------------------------------------------------- C3b
print()
print("=== [C3b] test name renames ===")
replace_token("C3b clamp test name",
              "decay_hypothesis_belief_with_floor_clamps_at_threshold",
              "decay_hypothesis_belief_crosses_selection_floor", 1)
replace_token("C3b dead test name",
              "decay_hypothesis_belief_with_floor_leaves_dead_hypotheses_alone",
              "decay_hypothesis_belief_leaves_dead_hypotheses_alone", 1)


# ------------------------------------------------- assertions
print()
print("=== SAFETY ASSERTIONS ===")
src = "\n".join(lines)

checks = [
    ("no with_floor identifier remains",
     src.count("decay_hypothesis_belief_with_floor") == 0),
    ("const FLOOR removed",
     src.count("const FLOOR") == 0),
    ("clamp line removed",
     "if pre >= FLOOR && post < FLOOR { FLOOR } else { post }" not in src),
    ("new helper present exactly once",
     src.count(NEW_FN) == 1),
    ("helper delegates to graph.decay",
     src.count("session.graph.decay(h_id, delta)") == 1),
    ("Self:: call sites renamed x2",
     src.count("Self::decay_hypothesis_belief(session, h_id, 0.20);") == 2),
    ("test call sites renamed x2",
     src.count("CorticalLoop::decay_hypothesis_belief(&mut session, h_id, 0.20);") == 2),
    ("crosses_selection_floor test present",
     src.count("decay_hypothesis_belief_crosses_selection_floor") == 1),
    ("dead-hypotheses test renamed",
     src.count("decay_hypothesis_belief_leaves_dead_hypotheses_alone") == 1),
    ("new expectation expression present",
     src.count("(0.30_f32 - 0.20_f32).max(0.0)") == 1),
    ("floor rescue fn definition untouched",
     "fn floor_hypothesis_belief_after_failure(" in src),
    ("floor rescue tests untouched",
     src.count("floor_hypothesis_belief_after_failure_restores_threshold") == 1
     and src.count("floor_hypothesis_belief_after_failure_leaves_dead_hypotheses_alone") == 1),
]

bad = [n for n, g in checks if not g]
for n, g in checks:
    print("  [%s] %s" % ("ok" if g else "FAIL", n))
if bad:
    die("safety assertions failed: %s" % bad)

if hashlib.sha256(gp.read_bytes()).hexdigest() != gsha_before:
    die("cognition_graph.rs changed during run")
ok("cognition_graph.rs unchanged")


# ------------------------------------------------- diff
print()
print("=== DIFF: %s ===" % L_REL)
if "\n".join(lines) == src0:
    print("(no changes pending)")
else:
    print("\n".join(difflib.unified_diff(
        src0.split("\n"), lines,
        fromfile="a/" + L_REL, tofile="b/" + L_REL, lineterm="")))


# ------------------------------------------------- dry-run gate
src = "\n".join(lines)
if src == src0:
    print()
    print("File already in target state; nothing to apply.")
    sys.exit(0)

if not APPLY:
    print()
    print("============================================================")
    print("DRY RUN COMPLETE — NOTHING WRITTEN")
    print("============================================================")
    print()
    print("THIS PASS CHANGES BEHAVIOR (F4, by design):")
    print("  - clamped decay now crosses SELECT_FLOOR (belief may fall")
    print("    below 0.15 and become unselectable on forced pivots)")
    print("  - exactly one test expectation flips: 0.15 -> (0.30-0.20).max(0)")
    print("All other tests must pass unchanged.")
    print()
    print("If correct, apply with:")
    print()
    print("  APPLY=1 ./pass_c_f4_unclamp.sh")
    sys.exit(0)


# ------------------------------------------------- apply
print()
print("=== APPLY ===")

backup = lp.with_name(lp.name + ".bak.passc." + TS)
shutil.copy2(lp, backup)
lp.write_text(src, encoding="utf-8")
print("[write]  %s" % lp)
print("[backup] %s" % backup)


def restore_and_die(msg):
    shutil.copy2(backup, lp)
    die("%s — %s restored from backup" % (msg, lp.name))


written = lp.read_text(encoding="utf-8")
post = [
    ("no with_floor remains", "decay_hypothesis_belief_with_floor" not in written),
    ("const FLOOR gone", "const FLOOR" not in written),
    ("new helper present", NEW_FN in written),
]
for name, good in post:
    if not good:
        restore_and_die("post-write check failed: %s" % name)
    print("[ok] %s" % name)

if hashlib.sha256(gp.read_bytes()).hexdigest() != gsha_before:
    restore_and_die("cognition_graph.rs changed during apply")

print()
print("=== COMPILE CHECK ===")
rc = subprocess.run(["cargo", "check", "-p", "nexus_runtime_core"], cwd=str(root)).returncode
if rc != 0:
    restore_and_die("cargo check failed (exit %d)" % rc)
print("[ok] cargo check passed")

print()
print("=== TESTS ===")
rc = subprocess.run(["cargo", "test", "-p", "nexus_runtime_core"], cwd=str(root)).returncode
if rc != 0:
    restore_and_die("cargo test failed (exit %d)" % rc)
print("[ok] cargo test passed")

print()
print("============================================================")
print(" PASS C COMPLETE — F4 APPLIED")
print("============================================================")
print()
print("Behavioral delta: clamped decay is now unclamped (crosses")
print("SELECT_FLOOR on forced pivots). Floor rescue untouched.")
PYEOF

echo ""
echo "=== Verify: F4 applied ==="
grep -n "decay_hypothesis_belief\|const FLOOR" \
  "$REPO_ROOT/crates/runtime_core/src/cortical_loop.rs" || true

echo ""
echo "=== Done ==="
