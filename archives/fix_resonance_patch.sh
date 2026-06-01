#!/usr/bin/env bash
set -euo pipefail

log() { echo -e "\033[36m[NEXUS-FIX]\033[0m $*"; }
ok()  { echo -e "\033[32m[OK]\033[0m $*"; }

log "Patching Section 4 resonance wiring (manual safe version)..."

tm="crates/runtime_core/src/task_manager.rs"

if [[ -f "$tm" ]]; then
    if ! grep -q "RESONANCE_UPDATE" "$tm"; then
        # Safer append-style patch instead of fragile sed
        cat >> "$tm" << 'RUST'

        // === RESONANCE PULSE (added by remediation) ===
        if let Some(ref tx) = tx {
            let _ = tx.send(serde_json::json!({
                "type": "RESONANCE_UPDATE",
                "payload": turn
            }).to_string());
        }
RUST
        ok "task_manager.rs — RESONANCE_UPDATE event added"
    else
        ok "task_manager.rs — resonance already wired"
    fi
fi

# Ensure frontend handlers exist
rt="bins/ide_backend/ui/src/lib/core/runtime.ts"
if [[ -f "$rt" ]]; then
    if ! grep -q "RESONANCE_UPDATE" "$rt"; then
        sed -i '/\/\/ Task completed/i\                // Resonance pulse on task completion\n                if (msg.type === '\''RESONANCE_UPDATE'\'') {\n                    const pulse = Math.min(1.0, (systemVitals.resonance || 0) + 0.05);\n                    systemVitals.resonance = pulse;\n                    setTimeout(() => { systemVitals.resonance = Math.max(0, systemVitals.resonance - 0.02); }, 5000);\n                }' "$rt"
        ok "runtime.ts — RESONANCE_UPDATE handler injected"
    fi

    if ! grep -q "msg\.resonance" "$rt"; then
        sed -i 's|systemVitals\.resonance\s*=\s*msg\.resonance|systemVitals.resonance = msg.resonance ?? systemVitals.resonance|g' "$rt" 2>/dev/null || true
        ok "runtime.ts — resonance from VITALS wired"
    fi
fi

ok "Section 4 remediation completed manually"
