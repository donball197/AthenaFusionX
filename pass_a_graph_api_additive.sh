#!/usr/bin/env bash
# pass_a_graph_api_additive.sh — PURE ADDITIVE boundary (Pass A).
#
# A1: Add SELECT_FLOOR.
# A2: Make select_best_hypothesis consume SELECT_FLOOR.
# A3: Add explicit graph-owned belief operations (10 methods).
#
# Guarantees:
#   - NO cortical_loop.rs call-site changes (checksum-verified).
#   - NO behavior change. F4 (floor-clamp removal) is Pass C, separate.
#   - NO generic belief setter. The caller decides WHY; the graph mutates.
#   - Apply is atomic: any failed gate restores the pre-edit file.
#
# Dry run:
#   ./pass_a_graph_api_additive.sh
#
# Apply:
#   APPLY=1 ./pass_a_graph_api_additive.sh

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

G_REL = "crates/runtime_core/src/cognition_graph.rs"
L_REL = "crates/runtime_core/src/cortical_loop.rs"

NUMERIC_TYPES = {
    "u8", "u16", "u32", "u64", "u128", "usize",
    "i8", "i16", "i32", "i64", "i128", "isize",
    "f32", "f64",
}

FORBIDDEN = (
    "pub fn set_hypothesis_belief",
    "pub fn set_belief",
    "pub fn set_belief_score",
    "pub fn decay_clamped",
)


def die(msg):
    print("  [abort] %s" % msg)
    print("  [abort] nothing written.")
    sys.exit(1)


def ok(msg):
    print("  [ok]   %s" % msg)


def skip(msg):
    print("  [skip] %s" % msg)


gp = root / G_REL
lp = root / L_REL

if not gp.is_file():
    die("target not found: %s" % gp)

if not lp.is_file():
    die("checksum target not found: %s" % lp)

src0 = gp.read_text(encoding="utf-8")
src = src0

cortical_sha_before = hashlib.sha256(lp.read_bytes()).hexdigest()

lines = src.split("\n")


def unique_line(marker):
    hits = [i for i, line in enumerate(lines) if marker in line]
    if len(hits) != 1:
        die("anchor %r matched %dx" % (marker, len(hits)))
    return hits[0]


def fn_span(marker):
    """Line span of a fn body via brace counting. No regex."""
    start = unique_line(marker)
    depth = 0
    started = False

    for j in range(start, min(len(lines), start + 200)):
        depth += lines[j].count("{") - lines[j].count("}")
        if "{" in lines[j]:
            started = True
        if started and depth == 0:
            return start, j

    die("function span for %r did not close within 200 lines" % marker)


def fn_signature(marker):
    """(params_text, return_type) via paren counting. No regex."""
    start = unique_line(marker)
    raw = ""
    depth = 0
    started = False
    open_at = -1
    close_at = -1

    for j in range(start, min(len(lines), start + 40)):
        for ci, ch in enumerate(lines[j]):
            if ch == "(":
                if not started:
                    started = True
                    open_at = len(raw) + ci
                depth += 1
            elif ch == ")" and started:
                depth -= 1
                if depth == 0:
                    close_at = len(raw) + ci
        raw += lines[j] + "\n"
        if started and depth == 0:
            break

    if not started or close_at < 0:
        die("could not parse signature of %r" % marker)

    params = raw[open_at + 1:close_at].strip()
    after = raw[close_at + 1:].split("{", 1)[0]

    if "->" not in after:
        die("no return type on %r; cannot derive composite return" % marker)

    ret = after.replace("->", "", 1).strip()
    return params, ret


# ============================================================
# A1 — SELECT_FLOOR
# ============================================================

print("=== [A1] SELECT_FLOOR ===")

if "pub const SELECT_FLOOR: f32 = 0.15;" in src:
    skip("SELECT_FLOOR already present")
else:
    use_lines = [i for i, line in enumerate(lines) if line.startswith("use ")]
    if not use_lines:
        die("A1: no top-level use lines found")

    insert_at = use_lines[-1] + 1

    block = [
        "",
        "/// Selection floor shared by hypothesis selection and belief policy.",
        "/// Hypotheses below this score are not selectable.",
        "pub const SELECT_FLOOR: f32 = 0.15;",
    ]

    lines[insert_at:insert_at] = block
    src = "\n".join(lines)
    ok("SELECT_FLOOR inserted after the use block")


# ============================================================
# A2 — select_best_hypothesis uses SELECT_FLOOR
# ============================================================

print()
print("=== [A2] select_best_hypothesis consumes SELECT_FLOOR ===")

start, end = fn_span("pub fn select_best_hypothesis")
function_text = "\n".join(lines[start:end + 1])

if "SELECT_FLOOR" in function_text:
    skip("select_best_hypothesis already uses SELECT_FLOOR")
else:
    hits = []
    for j in range(start, end + 1):
        stripped = lines[j].strip()
        if stripped.startswith("//") or stripped.startswith("///") or stripped.startswith("*"):
            continue
        if "0.15" in lines[j]:
            hits.append(j)

    if len(hits) != 1:
        print("  [diag] non-comment 0.15 occurrences in span %d..%d:" % (start + 1, end + 1))
        for j in range(start, end + 1):
            print("       %5d: %s" % (j + 1, lines[j]))
        die("A2: expected exactly 1 non-comment 0.15 in select_best_hypothesis, found %d" % len(hits))

    lines[hits[0]] = lines[hits[0]].replace("0.15", "SELECT_FLOOR", 1)
    src = "\n".join(lines)
    ok("literal 0.15 -> SELECT_FLOOR (line %d)" % (hits[0] + 1))


# ============================================================
# A3 — explicit graph-owned operations
# ============================================================

print()
print("=== [A3] explicit HypothesisGraph operations ===")

if "pub fn find_by_content" in src:
    skip("A3 methods already present")
else:
    params, ret = fn_signature("pub fn add_node")
    parts = [x.strip() for x in params.split(",") if ":" in x]

    # add_node has:
    #   &mut self,
    #   kind: NodeKind,
    #   content: &str,
    #   initial_belief: f32,
    #   cost: u32,
    #
    # &mut self is filtered out above, so the four typed parameters are:
    #   parts[0] = kind
    #   parts[1] = content
    #   parts[2] = initial_belief
    #   parts[3] = cost
    if len(parts) != 4 or ":" not in parts[3]:
        die("A3: add_node params unexpected: %s" % params)

    cost_name, cost_type = parts[3].split(":", 1)
    cost_name = cost_name.strip()
    cost_type = cost_type.strip()
    ret = ret.strip()

    print("  [diag] add_node: params=%s" % params)
    print("  [diag] cost param: %s: %s ; returns: %s" % (cost_name, cost_type, ret))

    if cost_type not in NUMERIC_TYPES:
        die(
            "A3: cost param type %r is not a primitive numeric; verify manually"
            % cost_type
        )

    if "Uuid" not in ret:
        die("A3: add_node return %r is not Uuid-like; verify manually" % ret)

    start, end = fn_span("pub fn select_best_hypothesis")

    METHODS = r'''
    /// Return the id of the first node whose content exactly matches
    /// `content`.
    pub fn find_by_content(&self, content: &str) -> Option<Uuid> {
        self.nodes
            .iter()
            .find(|(_, node)| node.content == content)
            .map(|(id, _)| *id)
    }

    /// Return node content, or an empty string when the id is unknown.
    pub fn content_of(&self, id: Uuid) -> String {
        self.nodes
            .get(&id)
            .map(|node| node.content.clone())
            .unwrap_or_default()
    }

    /// Return the current belief score for a node.
    pub fn belief_of(&self, id: Uuid) -> Option<f32> {
        self.nodes
            .get(&id)
            .map(|node| node.belief_score)
    }

    /// Demote a hypothesis to an explicit policy-selected score.
    ///
    /// The caller decides WHY the hypothesis should be demoted
    /// (dead symbol, critic veto, strategy violation, ...).
    /// The graph owns the actual belief-state mutation.
    pub fn demote_hypothesis(&mut self, id: Uuid, score: f32) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.belief_score = score;
            true
        } else {
            false
        }
    }

    /// Promote a hypothesis to at least `minimum`.
    pub fn promote_hypothesis(&mut self, id: Uuid, minimum: f32) -> bool {
        if let Some(node) = self.nodes.get_mut(&id) {
            if node.belief_score < minimum {
                node.belief_score = minimum;
            }
            true
        } else {
            false
        }
    }

    /// Cap beliefs for every node whose content matches `pred`.
    /// Bulk filter-and-cap only: the policy for WHICH strategies are
    /// capped stays with the caller.
    pub fn cap_matching(&mut self, pred: impl Fn(&str) -> bool, cap: f32) {
        for node in self.nodes.values_mut() {
            if pred(&node.content) && node.belief_score > cap {
                node.belief_score = cap;
            }
        }
    }

    /// Apply unclamped belief decay.
    ///
    /// This intentionally permits crossing SELECT_FLOOR, which is what
    /// forced-pivot callers intend. The clamped decay helper in
    /// cortical_loop.rs is untouched here; unclamping it is F4 and lands
    /// in Pass C.
    pub fn decay(&mut self, id: Uuid, delta: f32) -> Option<f32> {
        let node = self.nodes.get_mut(&id)?;
        node.belief_score = (node.belief_score - delta).max(0.0);
        Some(node.belief_score)
    }

    /// Restore a hypothesis to `floor` after evaluation failure, but only
    /// if it was already at or above that floor before evaluation: the
    /// floor is not a resurrection mechanism.
    pub fn restore_floor(&mut self, id: Uuid, pre_eval_belief: f32, floor: f32) {
        if pre_eval_belief < floor {
            return;
        }
        if let Some(node) = self.nodes.get_mut(&id) {
            if node.belief_score < floor {
                node.belief_score = floor;
            }
        }
    }

    /// Return the highest belief among hypothesis nodes.
    pub fn best_hypothesis_belief(&self) -> f32 {
        self.nodes
            .values()
            .filter(|node| matches!(node.kind, NodeKind::Hypothesis))
            .map(|node| node.belief_score)
            .fold(0.0_f32, f32::max)
    }

    /// Record a contradicting observation against `target` and propagate
    /// the resulting graph beliefs. Collapses the add_node + add_edge +
    /// propagate_beliefs triple previously typed at seven call sites.
    pub fn observe_contradiction(
        &mut self,
        content: &str,
        target: Uuid,
        ___COST_NAME___: ___COST_TYPE___,
        weight: f32,
    ) -> __RET__ {
        let observation_id = self.add_node(
            NodeKind::Observation,
            content,
            1.0,
            ___COST_NAME___,
        );

        self.add_edge(
            observation_id,
            target,
            EdgeRelation::Contradicts,
            weight,
        );

        self.propagate_beliefs();

        observation_id
    }
'''

    METHODS = (METHODS
               .replace("___COST_NAME___", cost_name)
               .replace("___COST_TYPE___", cost_type)
               .replace("__RET__", ret))

    if "___COST_" in METHODS or "__RET__" in METHODS:
        die("A3: unsubstituted token left in METHOD block")

    block = METHODS.split("\n")
    lines[end + 1:end + 1] = block
    src = "\n".join(lines)

    ok("added 10 explicit operations: find_by_content, content_of, "
       "belief_of, demote_hypothesis, promote_hypothesis, cap_matching, "
       "decay, restore_floor, best_hypothesis_belief, observe_contradiction")


# ============================================================
# SAFETY ASSERTIONS
# ============================================================

print()
print("=== SAFETY ASSERTIONS ===")

for name in FORBIDDEN:
    if name in src:
        die("forbidden API present: %s" % name)

ok("no generic belief setter, no decay_clamped / F4 API")

if src.count("pub const SELECT_FLOOR: f32 = 0.15;") != 1:
    die("SELECT_FLOOR must occur exactly once")

ok("SELECT_FLOOR occurs exactly once")

if hashlib.sha256(lp.read_bytes()).hexdigest() != cortical_sha_before:
    die("cortical_loop.rs changed unexpectedly")

ok("cortical_loop.rs unchanged")


# ============================================================
# DIFF
# ============================================================

print()
print("=== DIFF: %s ===" % G_REL)

if src == src0:
    print("(no changes pending)")
else:
    diff = difflib.unified_diff(
        src0.split("\n"),
        src.split("\n"),
        fromfile="a/" + G_REL,
        tofile="b/" + G_REL,
        lineterm="",
    )
    print("\n".join(diff))


# ============================================================
# DRY RUN GATE
# ============================================================

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
    print("Review the diff above (especially the [diag] add_node line:")
    print("the 4th param and return type were DERIVED, not hardcoded).")
    print()
    print("If correct, apply with:")
    print()
    print("  APPLY=1 ./pass_a_graph_api_additive.sh")
    sys.exit(0)


# ============================================================
# APPLY (atomic)
# ============================================================

print()
print("=== APPLY ===")

backup = gp.with_name(gp.name + ".bak.passa." + TS)
shutil.copy2(gp, backup)
gp.write_text(src, encoding="utf-8")

print("[write]  %s" % gp)
print("[backup] %s" % backup)


def restore_and_die(msg):
    shutil.copy2(backup, gp)
    die("%s — %s restored from backup" % (msg, gp.name))


# ============================================================
# POST-WRITE VERIFICATION
# ============================================================

print()
print("=== POST-WRITE VERIFICATION ===")

written = gp.read_text(encoding="utf-8")

required = [
    "pub const SELECT_FLOOR: f32 = 0.15;",
    "pub fn find_by_content",
    "pub fn content_of",
    "pub fn belief_of",
    "pub fn demote_hypothesis",
    "pub fn promote_hypothesis",
    "pub fn cap_matching",
    "pub fn decay(",
    "pub fn restore_floor",
    "pub fn best_hypothesis_belief",
    "pub fn observe_contradiction",
]

for marker in required:
    if marker not in written:
        restore_and_die("post-write verification failed: %s" % marker)
    print("[ok] %s" % marker)

for name in FORBIDDEN:
    if name in written:
        restore_and_die("post-write: forbidden API exists: %s" % name)

if hashlib.sha256(lp.read_bytes()).hexdigest() != cortical_sha_before:
    restore_and_die("POST-WRITE: cortical_loop.rs changed")

print("[ok] no forbidden APIs")
print("[ok] cortical_loop.rs unchanged")


# ============================================================
# COMPILE + TEST GATES (atomic)
# ============================================================

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

if hashlib.sha256(lp.read_bytes()).hexdigest() != cortical_sha_before:
    restore_and_die("FINAL: cortical_loop.rs changed during gates")

print()
print("============================================================")
print(" PASS A COMPLETE — ADDITIVE BOUNDARY GREEN")
print("============================================================")
print()
print("Pass B (mechanical substitutions) remains separate.")
print("Pass C (F4 clamp removal) remains separate.")
print("No cortical_loop.rs graph mutations were migrated.")
PYEOF

echo ""
echo "=== Verify markers ==="
grep -n "pub const SELECT_FLOOR\|pub fn find_by_content\|pub fn demote_hypothesis\|pub fn promote_hypothesis\|pub fn observe_contradiction" \
  "$REPO_ROOT/crates/runtime_core/src/cognition_graph.rs" || true

echo ""
echo "=== Verify forbidden APIs absent ==="
if grep -nE "pub fn (set_belief|set_hypothesis_belief|set_belief_score|decay_clamped)" \
  "$REPO_ROOT/crates/runtime_core/src/cognition_graph.rs"; then
    echo "ERROR: forbidden API found" >&2
    exit 1
fi
echo "none found"

echo ""
echo "=== Done ==="
