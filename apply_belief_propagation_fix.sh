#!/usr/bin/env bash
# apply_belief_propagation_fix.sh
# Applies the accumulation fix to CognitiveGraph::propagate_beliefs (no backup)
# Usage: ./apply_belief_propagation_fix.sh [path-to-AthenaFusionX]

set -euo pipefail

ROOT="${1:-.}"
TARGET="$ROOT/crates/runtime_core/src/cognition_graph.rs"

if [[ ! -f "$TARGET" ]]; then
  echo "ERROR: $TARGET not found"
  echo "Usage: $0 [/path/to/AthenaFusionX]"
  exit 1
fi

NEW_METHOD=$(cat << 'EOF'
    pub fn propagate_beliefs(&mut self) {
        // Accumulate signed impacts first so multiple edges targeting the same
        // hypothesis are combined instead of overwriting each other.
        let mut delta: HashMap<Uuid, f32> = HashMap::new();
        const DAMPING: f32 = 0.85;

        for edge in &self.edges {
            let Some(source_node) = self.nodes.get(&edge.source) else {
                continue;
            };
            let impact = source_node.belief_score * edge.weight;
            let signed = match edge.relation {
                EdgeRelation::Supports => impact,
                EdgeRelation::Contradicts => -impact,
                // Prerequisite / DerivesFrom intentionally ignored for score updates
                _ => continue,
            };
            *delta.entry(edge.target).or_insert(0.0) += signed;
        }

        for (id, d) in delta {
            if let Some(node) = self.nodes.get_mut(&id) {
                let candidate = (node.belief_score + d).clamp(0.0, 1.0);
                // Keep a fraction of the previous score (PageRank-style damping)
                node.belief_score = DAMPING * candidate + (1.0 - DAMPING) * node.belief_score;
            }
        }
    }
EOF
)

python3 - "$TARGET" "$NEW_METHOD" << 'PY'
import sys, re

path = sys.argv[1]
new_method = sys.argv[2]

with open(path, "r", encoding="utf-8") as f:
    src = f.read()

pattern = re.compile(
    r"    pub fn propagate_beliefs\(&mut self\) \{.*?\n    \}",
    re.DOTALL,
)

if not pattern.search(src):
    print("ERROR: could not locate existing propagate_beliefs method", file=sys.stderr)
    sys.exit(1)

updated = pattern.sub(new_method.rstrip(), src, count=1)

with open(path, "w", encoding="utf-8") as f:
    f.write(updated)

print(">> propagate_beliefs successfully replaced")
PY

echo ">> Done. Rebuild with:"
echo "   cargo check -p nexus_runtime_core"
