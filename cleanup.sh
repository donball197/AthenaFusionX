#!/usr/bin/env bash
set -euo pipefail

ROOT="${HOME}/AthenaFusionx"
CORTEX="crates/runtime_core/src/cortical_loop.rs"
GUARDS="crates/runtime_core/src/guards.rs"

cd "$ROOT"

# =============================================================================
# 1. Silence the dead_code warning on CortexState::Fail::evidence.
#    The field is retained (removing it would touch the enum constructor in
#    handle_strategize and every match site) but no longer read after the
#    fix that propagates session.local_evidence instead of the diagnostic.
# =============================================================================
python3 - "$CORTEX" <<'PY'
import sys
from pathlib import Path

p = Path(sys.argv[1])
s = p.read_text()

old = """    Fail {
        belief: f32,
        evidence: String,
    },"""

new = """    Fail {
        belief: f32,
        /// Diagnostic-only. The active propagation path returns
        /// `session.local_evidence` instead of this field (see the
        /// `CortexState::Fail` arm in `ignite_with_evidence`), so it is
        /// retained for constructors but never read.
        #[allow(dead_code)]
        evidence: String,
    },"""

if "#[allow(dead_code)]\n        evidence: String," in s:
    print("SKIP: warning already silenced")
    sys.exit(0)

if old not in s:
    print("REFUSED: CortexState::Fail declaration anchor not found")
    sys.exit(2)

p.write_text(s.replace(old, new, 1))
print("OK: CortexState::Fail::evidence warning silenced")
PY

# =============================================================================
# 2. Remove the orphaned guards.rs.
#    No `pub mod guards;` exists in lib.rs, so the file is never compiled.
#    Its functionality (SecurityGuard + AntiLoopGuard) duplicates what
#    arbiter_agent::validate_action and cortical_loop::action_key already do
#    on live paths. Keeping it invites future readers to wire it in and end
#    up with two parallel anti-loop mechanisms.
#
#    Only delete if the user has agreed. This script refuses to delete if
#    any workspace file references the module, in case the tree has drifted.
# =============================================================================
if [[ ! -f "$GUARDS" ]]; then
    echo "SKIP: guards.rs already removed"
else
    # Refuse to delete if any other source file references crate::guards or
    # the module's public types, in case someone wired it in since the audit.
    if rg -q --glob '!target/**' --glob '!.git/**' --glob '!guards.rs' \
        '(crate::guards|mod guards\b|AntiLoopGuard|GuardPipeline|SecurityGuard)' \
        "$ROOT/crates" "$ROOT/apps" "$ROOT/bins" 2>/dev/null; then
        echo "REFUSED: guards.rs is referenced elsewhere; not deleting."
        echo "Run the following to see the references:"
        echo "  rg -n 'crate::guards|mod guards|AntiLoopGuard|GuardPipeline|SecurityGuard' crates apps bins"
        exit 2
    fi
    cp "$GUARDS" "/tmp/guards.rs.removed.$$"
    rm "$GUARDS"
    echo "OK: guards.rs removed (backup at /tmp/guards.rs.removed.$$)"
fi

# =============================================================================
# 3. Verification gates
# =============================================================================
cargo check -p nexus_runtime_core
cargo test -p nexus_runtime_core cortical_loop -- --nocapture
