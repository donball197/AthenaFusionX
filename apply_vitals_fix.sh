#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
#  NEXUS SINGULARITY — Patch Script v1.0
#  Fixes:
#    1. Vitals tab locks UI  (infinite $effect reactive loop)
#    2. Vitals show all zeros (ws_bridge payload never matched)
#    3. isGhostTyping never resets
#    4. WS connects to wrong route (/ws/nexus → /ws/telemetry)
#    5. layout.ghostCortexVisible missing from store
# ═══════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UI="$SCRIPT_DIR/ui/src"

echo ">> [PATCH] Applying NEXUS UI fixes..."

# ───────────────────────────────────────────────────────────────
# 1.  store.svelte.js
# ───────────────────────────────────────────────────────────────
cat > "$UI/lib/core/store.svelte.js" << 'EOF'
// ════════════════════════════════════════════════════════════════
//  NEXUS SINGULARITY — Reactive Store v6.2.0
//  FIX: added layout.ghostCortexVisible (used by GhostCortex.svelte)
//  FIX: added vmoTelemetry state (populated by nexus_socket)
// ════════════════════════════════════════════════════════════════

export const wsState = $state({ ready: false, latency: 0 });

export const systemVitals = $state({
    cpu_load:     0,
    ram_gb:       0,
    total_ram_gb: 4.0,
    temperature:  0,
    resonance:    0,
});

export const terminalState = $state({
    logs: [],
    isGhostTyping: false,
    buffers: { GHOST_CORTEX: '', SYSTEM_LOG: '' },
});

export const ide = $state({
    openTabs: [],
    activeTab: null,
    mountFile(path, content) {
        const existing = this.openTabs.find(t => t.path === path);
        if (existing) { this.activeTab = path; return; }
        this.openTabs.push({ path, content, isDirty: false });
        this.activeTab = path;
    },
    updateContent(path, content) {
        const tab = this.openTabs.find(t => t.path === path);
        if (tab && tab.content !== content) {
            tab.content = content;
            tab.isDirty = true;
        }
    },
    markSaved(path) {
        const tab = this.openTabs.find(t => t.path === path);
        if (tab) tab.isDirty = false;
    },
    closeTab(path) {
        this.openTabs = this.openTabs.filter(t => t.path !== path);
        if (this.activeTab === path) {
            this.activeTab = this.openTabs.at(-1)?.path ?? null;
        }
    },
});

// FIX: ghostCortexVisible added so GhostCortex.svelte doesn't always read undefined
export const layout = $state({
    explorerVisible:    true,
    ghostVisible:       true,
    ghostCortexVisible: false,
});

export const fileTreeTrigger = $state({ value: 0 });

// VMO telemetry from ws_bridge.rs heartbeat — consumed by WORKSPACE panel
export const vmoTelemetry = $state({
    connected:   false,
    status:      'DISCONNECTED',
    active_vmos: 0,
    next_handle: '0x00',
    resonance:   0.0,
    logs:        [],
});
EOF

echo "   [OK] store.svelte.js"

# ───────────────────────────────────────────────────────────────
# 2.  NexusWS.js
# ───────────────────────────────────────────────────────────────
cat > "$UI/lib/core/NexusWS.js" << 'EOF'
// ════════════════════════════════════════════════════════════════
//  NEXUS SINGULARITY — WebSocket Manager v6.1.0
//  FIX: parse ws_bridge.rs heartbeat payload into systemVitals
//  FIX: clear isGhostTyping after stream silence (1.5s timeout)
//  FIX: connect to /ws/telemetry (actual registered route)
// ════════════════════════════════════════════════════════════════
import { terminalState, wsState, systemVitals, vmoTelemetry } from './store.svelte.js';

export let WS = null;
let ghostTypingTimer = null;

// Push a timestamped line into the terminal log
export function pushTermLine(message, cls = 'text-gray-400') {
    if (!terminalState.logs) terminalState.logs = [];
    terminalState.logs.push({
        message,
        cls,
        timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
    });
    if (terminalState.logs.length > 200) terminalState.logs.shift();
}

export async function connectWS() {
    if (typeof window === 'undefined') return null;
    if (WS && (WS.readyState === WebSocket.OPEN || WS.readyState === WebSocket.CONNECTING)) {
        return WS;
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    // FIX: use the route actually registered in server.rs
    WS = new WebSocket(`${protocol}//${window.location.host}/ws/telemetry`);

    WS.onopen = () => {
        wsState.ready = true;
        vmoTelemetry.connected = true;
        pushTermLine('>> [UPLINK] Neural connection established.', 'text-emerald-500');
    };

    WS.onclose = () => {
        wsState.ready = false;
        vmoTelemetry.connected = false;
        vmoTelemetry.status = 'DISCONNECTED';
        WS = null;
        pushTermLine('!! [OFFLINE] Neural link severed — reconnecting...', 'text-rose-500');
        setTimeout(connectWS, 3000);
    };

    WS.onerror = () => {
        wsState.ready = false;
    };

    WS.onmessage = (event) => {
        let data;
        try { data = JSON.parse(event.data); } catch { return; }

        // ── Heartbeat from ws_bridge.rs ──────────────────────────────
        // Shape: { status, active_vmos, next_handle, resonance }
        if (data.status && typeof data.active_vmos !== 'undefined') {
            vmoTelemetry.connected   = true;
            vmoTelemetry.status      = data.status      ?? vmoTelemetry.status;
            vmoTelemetry.active_vmos = data.active_vmos ?? vmoTelemetry.active_vmos;
            vmoTelemetry.next_handle = data.next_handle ?? vmoTelemetry.next_handle;
            vmoTelemetry.resonance   = data.resonance   ?? vmoTelemetry.resonance;
            systemVitals.resonance   = data.resonance   ?? systemVitals.resonance;
            return;
        }

        switch (data.type) {
            case 'NEURAL_STREAM': {
                // FIX: auto-clear typing indicator after 1.5 s of silence
                terminalState.isGhostTyping = true;
                clearTimeout(ghostTypingTimer);
                ghostTypingTimer = setTimeout(() => {
                    terminalState.isGhostTyping = false;
                }, 1500);

                const sector = data.payload?.sector ?? 'GHOST_CORTEX';
                const text   = data.payload?.text
                            ?? (typeof data.payload === 'string' ? data.payload : '');
                if (!terminalState.buffers[sector]) terminalState.buffers[sector] = '';
                terminalState.buffers[sector] += text;

                // Mirror to vmoTelemetry.logs for WORKSPACE panel
                if (text) {
                    vmoTelemetry.logs = [...vmoTelemetry.logs.slice(-59), text];
                }
                break;
            }

            // Legacy shape kept for future use
            case 'telemetry':
                systemVitals.cpu_load    = data.payload?.cpu_load    ?? 0;
                systemVitals.ram_gb      = data.payload?.ram_gb      ?? 0;
                systemVitals.temperature = data.payload?.temperature ?? 0;
                systemVitals.resonance   = data.payload?.resonance   ?? 0;
                terminalState.isGhostTyping = false;
                clearTimeout(ghostTypingTimer);
                break;

            case 'LOG_EVENT':
                if (data.payload?.message === 'Mission Initiated.') {
                    terminalState.buffers.GHOST_CORTEX = '';
                }
                pushTermLine(
                    `[${data.payload?.level ?? 'INFO'}] ${data.payload?.message ?? ''}`,
                    'text-sky-400'
                );
                break;
        }
    };

    return WS;
}
EOF

echo "   [OK] NexusWS.js"

# ───────────────────────────────────────────────────────────────
# 3.  VitalsView.svelte
# ───────────────────────────────────────────────────────────────
cat > "$UI/lib/components/VitalsView.svelte" << 'EOF'
<script>
    // ════════════════════════════════════════════════════════════════
    //  VitalsView — Fixed v2.0
    //  FIX 1: $effect that mutated derived arrays caused an infinite
    //         reactive loop → Svelte's scheduler locked up → tab stuck.
    //         Solution: use plain let arrays + a setInterval to push
    //         snapshots, completely outside the reactive graph.
    //  FIX 2: systemVitals never got CPU/RAM/temp because ws_bridge.rs
    //         sends { status, active_vmos, next_handle, resonance } —
    //         not a telemetry payload.  We now poll GET /api/vitals
    //         every 2 s for the real numbers.
    // ════════════════════════════════════════════════════════════════
    import { onMount, onDestroy } from 'svelte';
    import { systemVitals } from '$lib/core/store.svelte.js';

    const MAX = 40;

    // Plain (non-reactive) ring-buffer arrays — written by interval, read
    // on each render tick via $state bindings below.
    let cpuHist  = $state([]);
    let tempHist = $state([]);
    let ramHist  = $state([]);

    let pollInterval;
    let histInterval;

    // ── Fetch real vitals from Rust backend ──────────────────────────
    async function fetchVitals() {
        try {
            const r = await fetch('/api/vitals');
            if (!r.ok) return;
            const d = await r.json();
            systemVitals.cpu_load     = d.cpu_load     ?? systemVitals.cpu_load;
            systemVitals.ram_gb       = d.ram_gb       ?? systemVitals.ram_gb;
            systemVitals.total_ram_gb = d.total_ram_gb ?? systemVitals.total_ram_gb;
            systemVitals.temperature  = d.temperature  ?? systemVitals.temperature;
        } catch {
            // /api/vitals not yet wired — values stay at last known
        }
    }

    // ── Push snapshot to history arrays ─────────────────────────────
    function pushHistory() {
        const cpu  = (systemVitals.cpu_load   || 0) * 100;
        const temp =  systemVitals.temperature || 0;
        const ram  =  systemVitals.ram_gb      || 0;

        cpuHist  = [...cpuHist.slice(-(MAX - 1)),  cpu];
        tempHist = [...tempHist.slice(-(MAX - 1)), temp];
        ramHist  = [...ramHist.slice(-(MAX - 1)),  ram];
    }

    onMount(() => {
        fetchVitals();
        pollInterval = setInterval(fetchVitals, 2000);
        histInterval = setInterval(pushHistory, 2000);
    });

    onDestroy(() => {
        clearInterval(pollInterval);
        clearInterval(histInterval);
    });

    // ── Derived colours ──────────────────────────────────────────────
    let tc = $derived(
        systemVitals.temperature > 70 ? '#f87171' :
        systemVitals.temperature > 55 ? '#fb923c' : '#34d399'
    );
    let cc = $derived(
        systemVitals.cpu_load > 0.8 ? '#f87171' :
        systemVitals.cpu_load > 0.5 ? '#fbbf24' : '#22d3ee'
    );

    // ── SVG helpers ──────────────────────────────────────────────────
    function sp(data, w, h, max) {
        if (data.length < 2) return '';
        return 'M' + data.map((v, i) =>
            `${(i / (data.length - 1)) * w},${h - (v / max) * h}`
        ).join('L');
    }

    function arc(pct, r) {
        const clamped = Math.min(Math.max(pct, 0), 0.9999); // avoid full-circle SVG bug
        const a = clamped * Math.PI;
        const x = 60 + r * Math.cos(Math.PI + a);
        const y = 60 + r * Math.sin(Math.PI + a);
        const l = a > Math.PI ? 1 : 0;
        return `M ${60 - r} 60 A ${r} ${r} 0 ${l} 1 ${x} ${y}`;
    }
</script>

<div style="display:flex;flex-direction:column;height:100%;padding:16px;gap:14px;background:#000;overflow-y:auto;font-family:'JetBrains Mono',monospace;">

    <div>
        <div style="font-size:12px;font-weight:900;letter-spacing:.2em;color:#22d3ee;text-shadow:0 0 12px rgba(34,211,238,.4);">SYSTEM_VITALS</div>
        <div style="font-size:8px;color:rgba(0,255,255,.25);margin-top:2px;">REAL-TIME HARDWARE TELEMETRY // ARM64 CROSTINI</div>
    </div>

    <!-- Gauges -->
    <div style="display:flex;gap:10px;">
        {#each [
            { val: systemVitals.cpu_load   || 0, max: 1,                               label: 'CPU LOAD',  fmt: v => `${(v*100).toFixed(0)}%`,  c: cc       },
            { val: systemVitals.temperature|| 0, max: 100,                             label: 'THERMAL',   fmt: v => `${v.toFixed(1)}°C`,         c: tc       },
            { val: systemVitals.ram_gb     || 0, max: systemVitals.total_ram_gb || 4,  label: 'RAM USED',  fmt: v => `${v.toFixed(1)}G`,          c: '#a78bfa'},
        ] as g}
            <div style="flex:1;display:flex;flex-direction:column;align-items:center;gap:4px;">
                <svg viewBox="0 0 120 70" style="width:100%;max-width:120px;">
                    <path d={arc(1, 48)}             fill="none" stroke="rgba(255,255,255,.05)" stroke-width="8" stroke-linecap="round"/>
                    <path d={arc(g.val/g.max, 48)}   fill="none" stroke={g.c}                  stroke-width="8" stroke-linecap="round" style="filter:drop-shadow(0 0 4px {g.c})"/>
                    <text x="60" y="60" text-anchor="middle" fill={g.c} font-family="JetBrains Mono,monospace" font-size="13" font-weight="900">{g.fmt(g.val)}</text>
                </svg>
                <div style="font-size:8px;font-weight:900;letter-spacing:.12em;color:rgba(255,255,255,.3);">{g.label}</div>
            </div>
        {/each}
    </div>

    <!-- Sparklines -->
    {#each [
        { data: cpuHist,  max: 100,                            label: 'CPU HISTORY',     c: cc        },
        { data: tempHist, max: 100,                            label: 'THERMAL HISTORY', c: tc        },
        { data: ramHist,  max: systemVitals.total_ram_gb || 4, label: 'RAM HISTORY',     c: '#a78bfa' },
    ] as s}
        <div style="border:1px solid rgba(0,255,255,.06);padding:8px;background:rgba(0,0,0,.5);">
            <div style="font-size:8px;font-weight:900;letter-spacing:.1em;color:{s.c};margin-bottom:5px;">{s.label}</div>
            <svg viewBox="0 0 160 40" style="width:100%;height:40px;display:block;" preserveAspectRatio="none">
                {#if s.data.length >= 2}
                    <path d="{sp(s.data,160,40,s.max)} L160,40 L0,40 Z" fill={s.c} opacity=".2"/>
                    <path d={sp(s.data,160,40,s.max)} fill="none" stroke={s.c} stroke-width="1.5" style="filter:drop-shadow(0 0 3px {s.c})"/>
                {:else}
                    <text x="80" y="25" text-anchor="middle" fill="rgba(255,255,255,.15)" font-size="9" font-family="monospace">collecting data…</text>
                {/if}
            </svg>
        </div>
    {/each}

    <!-- Raw telemetry -->
    <div style="border:1px solid rgba(0,255,255,.06);padding:10px;background:rgba(0,0,0,.5);">
        <div style="font-size:8px;font-weight:900;letter-spacing:.15em;color:rgba(0,255,255,.3);margin-bottom:6px;">RAW TELEMETRY</div>
        <pre style="margin:0;font-size:10px;color:#64748b;white-space:pre;">{JSON.stringify({
  cpu:       `${((systemVitals.cpu_load   ||0)*100).toFixed(2)}%`,
  temp:      `${(systemVitals.temperature ||0).toFixed(2)}°C`,
  ram:       `${(systemVitals.ram_gb      ||0).toFixed(3)}GB`,
  resonance: (systemVitals.resonance      ||0).toFixed(4),
}, null, 2)}</pre>
    </div>

</div>
EOF

echo "   [OK] VitalsView.svelte"

# ───────────────────────────────────────────────────────────────
# 4.  Wire up /api/vitals in server.rs
#     (adds the missing GET endpoint that VitalsView polls)
# ───────────────────────────────────────────────────────────────
cat > "$SCRIPT_DIR/src/vitals_handler.rs" << 'EOF'
//! src/vitals_handler.rs
//! Thin axum handler that reads SystemMonitor and returns JSON.
//! Add to lib.rs:  pub mod vitals_handler;
//! Add to server.rs build_router:
//!     .route("/api/vitals", get(vitals_handler::handle_vitals))

use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;
use crate::server::AppState;

pub async fn handle_vitals(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut monitor = state.monitor.lock().await;
    let v = monitor.get_vitals();
    Json(json!({
        "cpu_load":     v.cpu_load,
        "ram_gb":       v.ram_gb,
        "total_ram_gb": v.total_ram_gb,
        "temperature":  v.temperature,
    }))
}
EOF

echo "   [OK] vitals_handler.rs"

# ── Reminder: wire it up ──────────────────────────────────────
cat << 'REMINDER'

─────────────────────────────────────────────────────────────────
  MANUAL STEPS STILL REQUIRED (two-line edits in existing files)
─────────────────────────────────────────────────────────────────

1. src/lib.rs  — add at the bottom of the pub mod list:
       pub mod vitals_handler;

2. src/server.rs  — inside build_router(), after the last .route():
       .route("/api/vitals", get(vitals_handler::handle_vitals))

   Also add to the imports at the top of server.rs:
       use crate::vitals_handler;

─────────────────────────────────────────────────────────────────
REMINDER

echo ">> [PATCH] Done. Rebuild with:  cd ui && npm run build && cd .. && cargo build"
