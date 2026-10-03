#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPLY="${APPLY:-0}"
TARGET="$ROOT/crates/runtime_core/src/cognition_graph.rs"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "============================================================"
echo " HypothesisGraph boundary — Step 1 (additive API only)"
echo " root:   $ROOT"
echo " APPLY:  $APPLY"
echo "============================================================"
echo

if [[ ! -f "$TARGET" ]]; then
    echo "[FAIL] missing target: $TARGET"
    exit 1
fi

SRC="$TMP/cognition_graph.rs"
cp "$TARGET" "$SRC"

python3 - "$SRC" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
src = path.read_text()

if "pub const SELECT_FLOOR: f32 = 0.15;" in src:
    raise SystemExit("[FAIL] SELECT_FLOOR already exists")

anchor = """pub struct CognitiveGraph {
"""

if anchor not in src:
    raise SystemExit("[FAIL] CognitiveGraph struct anchor not found")

# Put the graph invariant immediately before CognitiveGraph.
src = src.replace(
    anchor,
    """/// Minimum belief required for a hypothesis to remain selectable.
pub const SELECT_FLOOR: f32 = 0.15;

""" + anchor,
    1,
)

anchor = """    pub fn select_best_hypothesis(&self) -> Option<&CognitiveNode> {
"""

if anchor not in src:
    raise SystemExit("[FAIL] select_best_hypothesis anchor not found")

methods = r'''
    /// Find a node whose content exactly matches `content`.
    ///
    /// The graph owns node lookup; callers do not need direct access to
    /// the node map for this common orchestration operation.
    pub fn find_by_content(&self, content: &str) -> Option<Uuid> {
        self.nodes
            .iter()
            .find(|(_, node)| node.content == content)
            .map(|(id, _)| *id)
    }

    /// Return a node's content, or an empty string when the node is absent.
    pub fn content_of(&self, id: Uuid) -> String {
        self.nodes
            .get(&id)
            .map(|node| node.content.clone())
            .unwrap_or_default()
    }

    /// Return a node's current belief score.
    pub fn belief_of(&self, id: Uuid) -> Option<f32> {
        self.nodes.get(&id).map(|node| node.belief_score)
    }

    /// Explicitly demote a hypothesis to a policy-selected score.
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

    /// Cap matching hypothesis beliefs without exposing the node map.
    pub fn cap_matching(
        &mut self,
        pred: impl Fn(&str) -> bool,
        cap: f32,
    ) {
        for node in self.nodes.values_mut() {
            if pred(&node.content) && node.belief_score > cap {
                node.belief_score = cap;
            }
        }
    }

    /// Apply unclamped belief decay.
    ///
    /// This operation intentionally does not preserve SELECT_FLOOR.
    /// Floor-preserving behavior remains a separate policy operation.
    pub fn decay(&mut self, id: Uuid, delta: f32) -> Option<f32> {
        self.nodes.get_mut(&id).map(|node| {
            node.belief_score = (node.belief_score - delta).max(0.0);
            node.belief_score
        })
    }

    /// Restore the selection floor after evaluation failure when the
    /// hypothesis was already selectable before evaluation.
    pub fn restore_floor(
        &mut self,
        id: Uuid,
        pre_eval_belief: f32,
        floor: f32,
    ) -> bool {
        if pre_eval_belief < floor {
            return false;
        }

        if let Some(node) = self.nodes.get_mut(&id) {
            if node.belief_score < floor {
                node.belief_score = floor;
            }
            true
        } else {
            false
        }
    }

    /// Return the belief of the currently strongest hypothesis.
    pub fn best_hypothesis_belief(&self) -> f32 {
        self.nodes
            .values()
            .filter(|node| matches!(node.kind, NodeKind::Hypothesis))
            .map(|node| node.belief_score)
            .fold(0.0_f32, f32::max)
    }

'''

src = src.replace(anchor, methods + anchor, 1)

# Replace the hard-coded selection threshold only.
old = """.filter(|n| {
                matches!(n.kind, NodeKind::Hypothesis)
                    && n.is_active
                    && n.belief_score >= 0.15
            })
"""

new = """.filter(|n| {
                matches!(n.kind, NodeKind::Hypothesis)
                    && n.is_active
                    && n.belief_score >= SELECT_FLOOR
            })
"""

if old not in src:
    raise SystemExit(
        "[FAIL] select_best_hypothesis floor anchor not found; "
        "refusing to guess"
    )

src = src.replace(old, new, 1)

path.write_text(src)
PY

echo "=== STEP 1 PRE-FLIGHT ==="

grep -n "SELECT_FLOOR" "$SRC"
grep -n "pub fn find_by_content" "$SRC"
grep -n "pub fn demote_hypothesis" "$SRC"
grep -n "pub fn promote_hypothesis" "$SRC"
grep -n "pub fn cap_matching" "$SRC"
grep -n "pub fn decay(" "$SRC"
grep -n "pub fn restore_floor" "$SRC"
grep -n "pub fn best_hypothesis_belief" "$SRC"

echo
echo "=== FORBIDDEN STEP-1 APIs ==="

if grep -q "pub fn set_hypothesis_belief" "$SRC"; then
    echo "[FAIL] generic set_hypothesis_belief present"
    exit 1
fi

if grep -q "pub fn decay_clamped" "$SRC"; then
    echo "[FAIL] decay_clamped present — F4 must remain separate"
    exit 1
fi

echo "[OK] no generic setter"
echo "[OK] no F4 decay API"

echo
echo "=== DIFF ==="
diff -u "$TARGET" "$SRC" || true

echo
if [[ "$APPLY" != "1" ]]; then
    echo "DRY RUN ONLY — no files modified."
    echo "Review the diff above."
    echo
    echo "To apply Step 1:"
    echo "  APPLY=1 ./apply_hypothesis_boundary.sh"
    exit 0
fi

echo "=== APPLY ==="

BACKUP="$TARGET.bak.step1.$(date +%Y%m%d%H%M%S)"
cp "$TARGET" "$BACKUP"
cp "$SRC" "$TARGET"

echo "[OK] wrote $TARGET"
echo "[OK] backup: $BACKUP"

echo
echo "=== VERIFY ==="

cargo check -p nexus_runtime_core

echo
echo "=== TEST ==="

cargo test -p nexus_runtime_core

echo
echo "============================================================"
echo " STEP 1 COMPLETE"
echo "============================================================"
