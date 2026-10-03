#!/usr/bin/env bash
# pass_b_graph_mechanical_subst.sh — MECHANICAL SUBSTITUTIONS (Pass B of 3).
#
# Migrates raw cognition-graph access in cortical_loop.rs onto the
# Pass A verb surface. Behavior-neutral.
#
#   B1  nodes.values().any(content == x)      -> find_by_content()
#   B2  get_mut + content check + set 0.05    -> content_of + demote_hypothesis (x2)
#   B3  .iter().find + get_mut promote 0.56   -> find_by_content + promote_hypothesis
#   B4  values_mut read_file cap loop         -> cap_matching
#   B5  parse-fault inline decay (unclamped)  -> decay()          [same arithmetic]
#   B6  floor_hypothesis_belief_after_failure -> restore_floor()  [same semantics]
#   B8  nodes.get().map().unwrap_or_default() -> content_of()     (x2)
#   B9  belief fold chain                     -> best_hypothesis_belief()
#   B10 test asserts nodes[&h_id]             -> belief_of()      (x4)
#   B12 code-shift observation bundle         -> observe_contradiction()
#
# NOT in this pass (see deferred manifest at the end):
#   - 6 executor observation bundles (variable content expressions)
#   - 3 log-only nodes.get() reads, 1 belief fold in handle_evaluate
#   - best_hypothesis_id migration (separate design pass)
#   - F4 clamp removal (Pass C, deliberately untouched here)
#
# Dry run:  ./pass_b_graph_mechanical_subst.sh
# Apply:    APPLY=1 ./pass_b_graph_mechanical_subst.sh

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


def die(msg):
    print("  [abort] %s" % msg)
    print("  [abort] nothing written.")
    sys.exit(1)


applied = []


def ok(msg):
    applied.append(msg)
    print("  [ok]   %s" % msg)


def skip(msg):
    print("  [skip] %s" % msg)


lp = root / L_REL
gp = root / G_REL

if not lp.is_file():
    die("target not found: %s" % lp)
if not gp.is_file():
    die("Pass A target missing: %s" % gp)

src0 = lp.read_text(encoding="utf-8")
src = src0
gsha_before = hashlib.sha256(gp.read_bytes()).hexdigest()

lines = src.split("\n")


# ------------------------------------------------- preconditions
print("=== PRECONDITIONS ===")

gsrc = gp.read_text(encoding="utf-8")
missing = [m for m in (
    "pub fn find_by_content", "pub fn content_of", "pub fn belief_of",
    "pub fn demote_hypothesis", "pub fn promote_hypothesis",
    "pub fn cap_matching", "pub fn decay(", "pub fn restore_floor",
    "pub fn best_hypothesis_belief", "pub fn observe_contradiction",
) if m not in gsrc]
if missing:
    die("Pass A API missing from cognition_graph.rs: %s" % missing)
ok("Pass A verb surface present in cognition_graph.rs")

if "const FLOOR: f32 = 0.15;" not in src:
    die("clamped decay helper missing — F4 appears to be applied; Pass B expects it intact")
if "if pre >= FLOOR && post < FLOOR { FLOOR } else { post }" not in src:
    die("clamp line missing — F4 appears to be applied; Pass B expects it intact")
ok("F4 not yet applied (clamped decay helper intact)")


# ------------------------------------------------- engine
ANY = "<ANY>"


def _match_seq(i, seq):
    for k, want in enumerate(seq):
        if want == ANY:
            continue
        if lines[i + k].strip() != want:
            return False
    return True


def seq_hits(seq):
    n = len(seq)
    out = []
    for i in range(0, len(lines) - n + 1):
        if _match_seq(i, seq):
            out.append(i)
    return out


def present(seq):
    return len(seq_hits(seq)) == 1


def replace_block(tag, old_seq, new_lines, done_probe=None):
    """new_lines: list of (delta, text). Delta = indent steps of 4 vs first line."""
    joined = "\n".join(lines)
    if done_probe and done_probe in joined:
        skip(tag + " (already applied)")
        return
    if "<OLD:" not in "".join(t for _, t in new_lines) and present([t for _, t in new_lines]):
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
    matched = [lines[i + k].strip() for k in range(len(old_seq))]
    out = []
    for delta, text in new_lines:
        while "<OLD:" in text:
            a = text.index("<OLD:")
            b = text.index(">", a)
            idx = int(text[a + 5:b])
            text = text[:a] + matched[idx] + text[b + 1:]
        out.append(" " * (base + 4 * delta) + text)
    lines[i:i + len(old_seq)] = out
    ok("%s (lines %d..%d -> %d line(s))" % (tag, i + 1, i + len(old_seq), len(out)))


def replace_line(tag, old, new, count):
    if any(l.strip() == new for l in lines):
        skip(tag + " (already applied)")
        return
    hits = [i for i, l in enumerate(lines) if l.strip() == old]
    if len(hits) != count:
        die("%s: expected %d exact line(s), found %d" % (tag, count, len(hits)))
    for i in hits:
        base = len(lines[i]) - len(lines[i].lstrip())
        lines[i] = " " * base + new
    ok("%s (x%d)" % (tag, count))


# ------------------------------------------------- B1
print()
print("=== [B1] find_by_content (4 sites) ===")
replace_line("B1 regeneration dedup (1269)",
             "let already = session.graph.nodes.values().any(|n| n.content == h);",
             "let already = session.graph.find_by_content(&h).is_some();", 1)
replace_line("B1 post-read dedup (1576)",
             "let already = session.graph.nodes.values().any(|n| n.content == *strat);",
             "let already = session.graph.find_by_content(strat).is_some();", 1)
replace_line("B1 halt-veto dedup (1680)",
             "let already = session.graph.nodes.values().any(|n| n.content == recon);",
             "let already = session.graph.find_by_content(recon).is_some();", 1)
replace_line("B1 post-search dedup (1723)",
             "let already = session.graph.nodes.values().any(|n| n.content == strat);",
             "let already = session.graph.find_by_content(&strat).is_some();", 1)


# ------------------------------------------------- B2a
print()
print("=== [B2a] demote_hypothesis (patch_function, 1439) ===")
replace_block(
    "B2a demote dead patch_function hypothesis",
    [
        "if let Some(node) = session.graph.nodes.get_mut(&h_id) {",
        "if node.content.starts_with(\"patch_function:\") {",
        "node.belief_score = 0.05;",
        "}",
        "}",
    ],
    [
        (0, "if session.graph.content_of(h_id).starts_with(\"patch_function:\") {"),
        (1, "session.graph.demote_hypothesis(h_id, 0.05);"),
        (0, "}"),
    ])


# ------------------------------------------------- B2b
print()
print("=== [B2b] demote_hypothesis (halt:, 1673) ===")
replace_block(
    "B2b demote stuck halt hypothesis",
    [
        "if let Some(node) = session.graph.nodes.get_mut(&h_id) {",
        "if node.content.trim().starts_with(\"halt:\") {",
        "node.belief_score = 0.05;",
        "}",
        "}",
    ],
    [
        (0, "if session.graph.content_of(h_id).trim().starts_with(\"halt:\") {"),
        (1, "session.graph.demote_hypothesis(h_id, 0.05);"),
        (0, "}"),
    ])


# ------------------------------------------------- B3
print()
print("=== [B3] find_by_content + promote_hypothesis (1582) ===")
replace_block(
    "B3 promote existing mutation strategy",
    [
        "let existing_id = session",
        ".graph",
        ".nodes",
        ".iter()",
        ".find(|(_, n)| n.content == *strat)",
        ".map(|(id, _)| *id);",
        "if let Some(id) = existing_id {",
        "if let Some(node) = session.graph.nodes.get_mut(&id) {",
        "if node.belief_score < 0.56 {",
        "node.belief_score = 0.56;",
        "}",
        "}",
    ],
    [
        (0, "let existing_id = session.graph.find_by_content(strat);"),
        (0, "if let Some(id) = existing_id {"),
        (1, "session.graph.promote_hypothesis(id, 0.56);"),
    ])


# ------------------------------------------------- B4
print()
print("=== [B4] cap_matching (1626) ===")
replace_block(
    "B4 cap competing read_file strategies",
    [
        "for node in session.graph.nodes.values_mut() {",
        "if node.content.starts_with(\"read_file:\")",
        "&& node.content.contains(path_str.as_str())",
        "&& node.belief_score > 0.20",
        "{",
        "node.belief_score = 0.20;",
        "}",
        "}",
    ],
    [
        (0, "session.graph.cap_matching("),
        (1, "|c| c.starts_with(\"read_file:\") && c.contains(path_str.as_str()),"),
        (1, "0.20,"),
        (0, ");"),
    ])


# ------------------------------------------------- B5
print()
print("=== [B5] decay (parse-fault, 1894) - same arithmetic ===")
# The log call carries an ANSI escape string literal. The pattern never
# matches its content: old-block line 5 is <ANY>, and the replacement
# re-emits it verbatim via <OLD:4>. No escape sequence appears anywhere
# in this script.
replace_block(
    "B5 parse-fault decay via graph.decay",
    [
        "if let Some(h_id) = session.best_hypothesis_id {",
        "if let Some(node) = session.graph.nodes.get_mut(&h_id) {",
        "node.belief_score = (node.belief_score - 0.20).max(0.0);",
        "self.log(format!(",
        "<ANY>",
        "node.belief_score",
        "));",
        "}",
        "}",
    ],
    [
        (0, "if let Some(h_id) = session.best_hypothesis_id {"),
        (1, "if let Some(new_belief) = session.graph.decay(h_id, 0.20) {"),
        (2, "self.log(format!("),
        (3, "<OLD:4>"),
        (3, "new_belief"),
        (2, "));"),
        (1, "}"),
        (0, "}"),
    ],
    done_probe="session.graph.decay(h_id, 0.20)")


# ------------------------------------------------- B6
print()
print("=== [B6] restore_floor delegate (2989) — same semantics ===")
replace_block(
    "B6 floor rescue via restore_floor",
    [
        "if pre_eval_belief < floor {",
        "return;",
        "}",
        "if let Some(node) = session.graph.nodes.get_mut(&h_id) {",
        "if node.belief_score < floor {",
        "node.belief_score = floor;",
        "}",
        "}",
    ],
    [
        (0, "session.graph.restore_floor(h_id, pre_eval_belief, floor);"),
    ])


# ------------------------------------------------- B8
print()
print("=== [B8] content_of (2053, 3045) ===")
replace_line("B8 handle_critic h_content (2053)",
             "session.graph.nodes.get(&h_id).map(|n| n.content.clone()).unwrap_or_default()",
             "session.graph.content_of(h_id)", 1)
replace_line("B8 handle_evaluate h_content (3045)",
             "session.graph.nodes.get(&h_id).map(|n| n.content.clone()).unwrap_or_default();",
             "session.graph.content_of(h_id);", 1)


# ------------------------------------------------- B9
print()
print("=== [B9] best_hypothesis_belief (1286) ===")
replace_block(
    "B9 belief fold via best_hypothesis_belief",
    [
        "let best_belief = session",
        ".graph",
        ".nodes",
        ".values()",
        ".filter(|n| matches!(n.kind, NodeKind::Hypothesis))",
        ".map(|n| n.belief_score)",
        ".fold(0.0_f32, f32::max);",
    ],
    [
        (0, "let best_belief = session.graph.best_hypothesis_belief();"),
    ])


# ------------------------------------------------- B10
print()
print("=== [B10] test asserts -> belief_of ===")
replace_line("B10a", "assert_eq!(session.graph.nodes[&h_id].belief_score, 0.15);",
             "assert_eq!(session.graph.belief_of(h_id), Some(0.15));", 2)
replace_line("B10b", "assert_eq!(session.graph.nodes[&h_id].belief_score, 0.05);",
             "assert_eq!(session.graph.belief_of(h_id), Some(0.05));", 1)
replace_line("B10c", "assert_eq!(session.graph.nodes[&h_id].belief_score, 0.0);",
             "assert_eq!(session.graph.belief_of(h_id), Some(0.0));", 1)


# ------------------------------------------------- B12
print()
print("=== [B12] observe_contradiction (code-shift bundle, 433) ===")
replace_block(
    "B12 code-shift observation bundle",
    [
        "let obs_id = session.graph.add_node(NodeKind::Observation, &shift_obs, 1.0, 85);",
        "session.graph.add_edge(obs_id, h_id, EdgeRelation::Contradicts, 0.40);",
        "session.graph.propagate_beliefs();",
    ],
    [
        (0, "session.graph.observe_contradiction(&shift_obs, h_id, 85, 0.40);"),
    ])


# ------------------------------------------------- safety assertions
print()
print("=== SAFETY ASSERTIONS (behavior-neutrality) ===")

src = "\n".join(lines)

checks = [
    ("only deferred clamped-decay get_mut remains",
     src.count("session.graph.nodes.get_mut") == 1
     and src.count("if let Some(node) = session.graph.nodes.get_mut(&h_id) {") == 1
     and "fn decay_hypothesis_belief_with_floor(" in src),
    ("values_mut fully migrated", src.count("session.graph.nodes.values_mut") == 0),
    ("test asserts migrated", src.count("session.graph.nodes[") == 0),
    ("3 log-only nodes.get reads deferred", src.count("session.graph.nodes.get(") == 3),
    ("clamped decay helper still intact (F4 is Pass C)",
     "const FLOOR: f32 = 0.15;" in src
     and "if pre >= FLOOR && post < FLOOR { FLOOR } else { post }" in src),
    ("no decay_hypothesis_belief_with_floor call sites changed",
     src.count("Self::decay_hypothesis_belief_with_floor(session, h_id, 0.20);") == 2),
    ("find_by_content x5", src.count("session.graph.find_by_content(") == 5),
    ("demote_hypothesis x2", src.count("session.graph.demote_hypothesis(") == 2),
    ("promote_hypothesis x1", src.count("session.graph.promote_hypothesis(") == 1),
    ("cap_matching x1", src.count("session.graph.cap_matching(") == 1),
    ("decay(h_id, 0.20) x1", src.count("session.graph.decay(h_id, 0.20)") == 1),
    ("restore_floor x1", src.count("session.graph.restore_floor(") == 1),
    ("content_of x4", src.count("session.graph.content_of(") == 4),
    ("best_hypothesis_belief x1", src.count("session.graph.best_hypothesis_belief()") == 1),
    ("belief_of x4 (tests)", src.count("session.graph.belief_of(") == 4),
    ("observe_contradiction x1", src.count("session.graph.observe_contradiction(") == 1),
]

bad = [name for name, good in checks if not good]
for name, good in checks:
    print("  [%s] %s" % ("ok" if good else "FAIL", name))
if bad:
    die("safety assertions failed: %s" % bad)

if hashlib.sha256(gp.read_bytes()).hexdigest() != gsha_before:
    die("cognition_graph.rs changed during run")
ok("cognition_graph.rs unchanged")


# ------------------------------------------------- diff
print()
print("=== DIFF: %s ===" % L_REL)
if src == src0:
    print("(no changes pending)")
else:
    print("\n".join(difflib.unified_diff(
        src0.split("\n"), src.split("\n"),
        fromfile="a/" + L_REL, tofile="b/" + L_REL, lineterm="")))


# ------------------------------------------------- deferred manifest
print()
print("=== DEFERRED (Pass B2 / later) ===")
for item in [
    "6 executor observe_contradiction bundles: 2333-2340, 2408-2415, 2530-2537, 2605-2612, 2799-2817, 2916-2934",
    "3 log-only nodes.get(&h_id) reads: 1951, 2127, 3288",
    "1 belief fold in handle_evaluate (~3132)",
    "best_hypothesis_id migration onto the graph (promote_or_add)",
    "F4 clamp removal (Pass C)",
]:
    print("  - " + item)


# ------------------------------------------------- dry-run gate
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
    print("Review the diff above. Every change is behavior-neutral:")
    print("identical arithmetic, identical predicates, identical logs.")
    print()
    print("If correct, apply with:")
    print()
    print("  APPLY=1 ./pass_b_graph_mechanical_subst.sh")
    sys.exit(0)


# ------------------------------------------------- apply (atomic)
print()
print("=== APPLY ===")

backup = lp.with_name(lp.name + ".bak.passb." + TS)
shutil.copy2(lp, backup)
lp.write_text(src, encoding="utf-8")
print("[write]  %s" % lp)
print("[backup] %s" % backup)


def restore_and_die(msg):
    shutil.copy2(backup, lp)
    die("%s — %s restored from backup" % (msg, lp.name))


written = lp.read_text(encoding="utf-8")

post = [
    ("only deferred clamped-decay get_mut remains",
     written.count("session.graph.nodes.get_mut") == 1
     and written.count("if let Some(node) = session.graph.nodes.get_mut(&h_id) {") == 1
     and "fn decay_hypothesis_belief_with_floor(" in written),
    ("values_mut residual", "session.graph.nodes.values_mut" not in written),
    ("test assert residual", "session.graph.nodes[" not in written),
    ("F4 still unapplied", "const FLOOR: f32 = 0.15;" in written),
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
print(" PASS B COMPLETE — MECHANICAL SUBSTITUTIONS GREEN")
print("============================================================")
print()
print("All migrated sites are behavior-neutral.")
print("Clamped decay helper untouched — F4 is Pass C.")
PYEOF

echo ""
echo "=== Verify: residual raw access (deferred only) ==="
grep -n "graph\.nodes" "$REPO_ROOT/crates/runtime_core/src/cortical_loop.rs" || true

echo ""
echo "=== Done ==="
