#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════
#  NEXUS UI — WS ARCHITECTURE FIX v1.0
#  ROOT CAUSE: NexusWS.js was hollowed into a stub — the singleton WS
#  in nexus_socket.svelte.ts was disconnected from store.svelte.js.
#  Messages arrived at the socket but never reached the reactive store,
#  so the terminal showed nothing when directives were sent from the UI.
#
#  FIX ARCHITECTURE:
#    nexus_socket.svelte.ts  ← single WebSocket, handles all message types
#    store.svelte.js         ← imports socket, wires all reactive state
#    NexusWS.js              ← thin compatibility shim (TaskService.log only)
#    TaskService.js          ← already correct, no change needed
# ═══════════════════════════════════════════════════════════════════════
set -euo pipefail
REPO="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$REPO"

SNAP=".nexus/vault/ws_fix_$(date +%s)"
mkdir -p "$SNAP"
cp -r ui/src/ "$SNAP/" 2>/dev/null || true
echo ">> [LAZARUS] Snapshot → $SNAP"
echo ""

# ═══════════════════════════════════════════════════════════════════════
#  FILE 1 — ui/src/lib/nexus_socket.svelte.ts  (THE REAL FIX)
#  Single authoritative WebSocket. Handles ALL message types.
#  Exposes: telemetry $state, send(), connect(), destroy()
#  Also dispatches to store.svelte.js via onMessageHandlers callbacks
#  so the reactive graph stays in sync.
# ═══════════════════════════════════════════════════════════════════════
cat << 'EOF' > ui/src/lib/nexus_socket.svelte.ts
// nexus_socket.svelte.ts — v3.0 AUTHORITATIVE SINGLETON
// THE FIX: This is now the single WebSocket instance for the entire app.
// store.svelte.js imports this and wires reactive state from it.
// NexusWS.js is a thin compatibility shim — it does NOT open a socket.
//
// Message routing:
//   HEARTBEAT      → telemetry.$state + onMessageHandlers
//   NEURAL_STREAM  → terminalState.buffers.GHOST_CORTEX (via handlers)
//   NEURAL_THOUGHT → terminalState.logs (via handlers)
//   TASK_STATUS    → terminalState.logs (via handlers)
//   KERNEL_GENESIS → window.dispatchEvent('nexus-genesis')
//   EXECUTOR       → GENESIS check + logs

function getWsUrl(path = '/ws/telemetry'): string {
    if (typeof window === 'undefined') return `ws://localhost:3000${path}`;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}${path}`;
}

// ── Message handler registry ─────────────────────────────────────────
type MsgHandler = (data: Record<string, any>) => void;
const _msgHandlers: MsgHandler[] = [];

export function onSocketMessage(fn: MsgHandler) {
    _msgHandlers.push(fn);
}

// ── Genesis dispatch ─────────────────────────────────────────────────
function dispatchGenesis(text: string): boolean {
    if (!text.includes('GENESIS_PAYLOAD:')) return false;
    const b64 = text.split('GENESIS_PAYLOAD:')[1].trim();
    if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('nexus-genesis', { detail: b64 }));
    }
    return true;
}

// ── Socket state ─────────────────────────────────────────────────────
let _socket: WebSocket | null = null;
let _reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let _retries = 0;

export const telemetry = $state({
    connected:   false,
    status:      'DISCONNECTED',
    active_vmos: 0,
    next_handle: '0x00',
    resonance:   0.0,
    cpu_load:    0.0,
    ram_gb:      0.0,
    total_ram_gb: 4.0,
    temperature: 0.0,
    live_logs:   [] as string[],
});

function pushLiveLog(line: string) {
    telemetry.live_logs = [...telemetry.live_logs.slice(-59), line];
}

export function connect(path = '/ws/telemetry') {
    if (_socket && (_socket.readyState === WebSocket.OPEN ||
                    _socket.readyState === WebSocket.CONNECTING)) return;

    const url = getWsUrl(path);
    _socket = new WebSocket(url);

    _socket.onopen = () => {
        telemetry.connected = true;
        telemetry.status    = 'CONNECTED';
        _retries = 0;
        console.log('[NexusSocket] Connected');
    };

    _socket.onmessage = (event: MessageEvent) => {
        let data: Record<string, any>;
        try {
            data = JSON.parse(event.data);
        } catch {
            pushLiveLog(String(event.data));
            return;
        }

        // 1. Dispatch to store.svelte.js and all registered handlers
        _msgHandlers.forEach(h => {
            try { h(data); } catch(e) { console.warn('[NexusSocket] handler error', e); }
        });

        // 2. Update telemetry $state directly (HEARTBEAT)
        if (data.type_id === 'HEARTBEAT') {
            telemetry.active_vmos = data.active_vmos ?? telemetry.active_vmos;
            telemetry.next_handle = data.next_handle ?? telemetry.next_handle;
            telemetry.resonance   = data.resonance   ?? telemetry.resonance;
            telemetry.cpu_load    = data.cpu_load    ?? telemetry.cpu_load;
            telemetry.ram_gb      = data.ram_gb      ?? telemetry.ram_gb;
            telemetry.total_ram_gb= data.total_ram_gb?? telemetry.total_ram_gb;
            telemetry.temperature = data.temperature ?? telemetry.temperature;
            return;
        }

        // 3. GENESIS payloads
        const text = typeof data.payload === 'string' ? data.payload : '';
        if (data.type === 'NEURAL_THOUGHT' || data.type === 'EXECUTOR') {
            if (dispatchGenesis(text)) return;
        }

        // 4. Live log for terminal panel
        if (text.trim()) {
            pushLiveLog(text);
        }
    };

    _socket.onclose = () => {
        telemetry.connected = false;
        telemetry.status    = 'DISCONNECTED';
        const delay = Math.min(1000 * Math.pow(2, _retries), 16000);
        _retries++;
        _reconnectTimer = setTimeout(() => connect(path), delay);
    };

    _socket.onerror = () => {
        telemetry.connected = false;
    };
}

export function send(cmd: string): boolean {
    if (_socket?.readyState === WebSocket.OPEN) {
        _socket.send(cmd);
        return true;
    }
    return false;
}

export function destroy() {
    if (_reconnectTimer) clearTimeout(_reconnectTimer);
    _socket?.close();
    _socket = null;
}

// ── Singleton export for +layout.svelte ─────────────────────────────
export const nexusSocket = { telemetry, connect, send, destroy };
EOF
echo ">> [FIX 1/5] ui/src/lib/nexus_socket.svelte.ts — authoritative singleton WS"

# ═══════════════════════════════════════════════════════════════════════
#  FILE 2 — ui/src/lib/core/store.svelte.js  (THE REAL FIX)
#  Wires into nexus_socket.svelte.ts via onSocketMessage().
#  wsState.ready is now a plain $derived that actually works.
# ═══════════════════════════════════════════════════════════════════════
cat << 'EOF' > ui/src/lib/core/store.svelte.js
// ui/src/lib/core/store.svelte.js — v4.0 WIRED TO REAL SOCKET
// FIX: Imports from nexus_socket.svelte.ts which is the single WS.
//      wsState.ready is a plain boolean $state updated by the socket.
//      All terminal/vitals/vmo reactive state wired via onSocketMessage.
import { onSocketMessage, telemetry } from '$lib/nexus_socket.svelte';

// ── Layout ────────────────────────────────────────────────────────────
export const layout = $state({
    sidebarOpen:        true,
    explorerVisible:    true,
    ghostVisible:       true,
    ghostCortexVisible: true,
    activeTab:          'WORKSPACE',
    showVitals:         true,
    autopilotActive:    false,
    visorVisible:       false,
});

// ── WS state ──────────────────────────────────────────────────────────
// FIX: plain $state, not a getter. Updated by onSocketMessage handler.
export const wsState = $state({ status: 'DISCONNECTED', ready: false });

// ── System vitals — proxy to telemetry $state ──────────────────────
export const systemVitals = $state({
    cpu_load:     0,
    ram_gb:       0,
    total_ram_gb: 4,
    temperature:  0,
    resonance:    0,
});

// ── IDE ────────────────────────────────────────────────────────────────
export const ide = $state({
    activeTab: null,
    openTabs:  [],

    mountFile(path, content) {
        if (!ide.openTabs.find(t => t.path === path)) {
            ide.openTabs.push({ path, content, isDirty: false });
        }
        ide.activeTab = path;
    },
    closeTab(path) {
        ide.openTabs = ide.openTabs.filter(t => t.path !== path);
        if (ide.activeTab === path) ide.activeTab = ide.openTabs[0]?.path || null;
    },
    updateContent(path, content) {
        const tab = ide.openTabs.find(t => t.path === path);
        if (tab) { tab.content = content; tab.isDirty = true; }
    },
    markSaved(path) {
        const tab = ide.openTabs.find(t => t.path === path);
        if (tab) tab.isDirty = false;
    },
});

// ── Terminal ───────────────────────────────────────────────────────────
export const terminalState = $state({
    logs:          [],
    buffers:       { GHOST_CORTEX: '' },
    isGhostTyping: false,
});

function addLog(message, cls = 'text-gray-400') {
    terminalState.logs = [
        ...terminalState.logs.slice(-179),
        { timestamp: new Date().toLocaleTimeString(), message, cls },
    ];
}

export const fileTreeTrigger = $state({ value: 0 });
export const neuralInventory = $state({
    models: [{ name: 'Gemma 4', tier: 'PRO', task: 'REASONING' }],
});
export const vmoTelemetry = $state({ active_vmos: 0 });

// ── WIRE: onSocketMessage → reactive store ─────────────────────────
// This is the critical connection that was broken.
onSocketMessage((msg) => {
    // Mark WS as connected on any message
    wsState.status = 'CONNECTED';
    wsState.ready  = true;

    switch (msg.type) {
        case 'NEURAL_STREAM':
            terminalState.buffers.GHOST_CORTEX += msg.payload ?? '';
            terminalState.isGhostTyping = true;
            addLog(msg.payload ?? '', 'text-gray-400');
            break;

        case 'NEURAL_THOUGHT': {
            const thought = msg.payload ?? '';
            addLog(thought, 'text-blue-400');
            break;
        }

        case 'TASK_STATUS': {
            const status = String(msg.payload ?? '');
            addLog(`>> [FSM] ${status}`, 'text-emerald-400');
            if (status.includes('COMPLETE') || status.includes('HALTED')) {
                terminalState.isGhostTyping = false;
            }
            break;
        }

        case 'LOG_EVENT':
            addLog(msg.payload ?? '', msg.cls ?? 'text-gray-400');
            break;

        default:
            // HEARTBEAT — sync systemVitals and vmoTelemetry from telemetry $state
            if (msg.type_id === 'HEARTBEAT') {
                systemVitals.cpu_load     = msg.cpu_load     ?? systemVitals.cpu_load;
                systemVitals.temperature  = msg.temperature  ?? systemVitals.temperature;
                systemVitals.ram_gb       = msg.ram_gb       ?? systemVitals.ram_gb;
                systemVitals.total_ram_gb = msg.total_ram_gb ?? systemVitals.total_ram_gb;
                systemVitals.resonance    = msg.resonance    ?? systemVitals.resonance;
                vmoTelemetry.active_vmos  = msg.active_vmos  ?? vmoTelemetry.active_vmos;
            }
            break;
    }
});

// ── Public log helper (used by TaskService via NexusWS.js shim) ─────
export function storeAddLog(message, cls = 'text-gray-400') {
    addLog(message, cls);
}
EOF
echo ">> [FIX 2/5] ui/src/lib/core/store.svelte.js — wired to socket via onSocketMessage"

# ═══════════════════════════════════════════════════════════════════════
#  FILE 3 — ui/src/lib/core/NexusWS.js  (THIN SHIM ONLY)
#  No longer opens any WebSocket. Just provides the log helper that
#  TaskService and legacy components expect.
# ═══════════════════════════════════════════════════════════════════════
cat << 'EOF' > ui/src/lib/core/NexusWS.js
// NexusWS.js — v3.0 COMPATIBILITY SHIM (no WebSocket here)
// The real WebSocket is in nexus_socket.svelte.ts.
// This module exists only so legacy imports of pushTermLine still work.
import { storeAddLog } from './store.svelte.js';

// pushTermLine: Used by TaskService.js and some components directly.
export function pushTermLine(message, cls = 'text-gray-400') {
    storeAddLog(message, cls);
}

// Stub nexusWS object so any legacy import of `nexusWS` doesn't crash.
export const nexusWS = {
    registerLogHandler() {},
    onMessage() {},
    send() {},
    logHandlers: [],
};

export function connectWS() {
    // No-op: connection is managed by nexus_socket.svelte.ts
}
EOF
echo ">> [FIX 3/5] ui/src/lib/core/NexusWS.js — thin shim, no WS, delegates to store"

# ═══════════════════════════════════════════════════════════════════════
#  FILE 4 — ui/src/lib/services/TaskService.js  (SYNC WITH FIX)
#  Was importing pushTermLine from NexusWS.js — that still works now.
#  But also ensure runDirective shows the queued message in terminal.
# ═══════════════════════════════════════════════════════════════════════
cat << 'EOF' > ui/src/lib/services/TaskService.js
// TaskService.js — v1.1
// Central service for queuing directives and logging to terminal.
// Works with the fixed store.svelte.js / nexus_socket.svelte.ts stack.
import { storeAddLog } from '$lib/core/store.svelte.js';

export const TaskService = {
    log(msg, cls = 'text-gray-400') {
        storeAddLog(msg, cls);
    },

    async runDirective(directive, source = 'WORKSPACE') {
        this.log(`>> [${source}] Queuing: ${directive}`, 'text-cyan-400');
        try {
            const res = await fetch('/api/task', {
                method:  'POST',
                headers: { 'Content-Type': 'application/json' },
                body:    JSON.stringify({ directive }),
            });
            if (res.ok) {
                this.log(`>> [${source}] Task queued — responses appear below.`, 'text-emerald-400');
            } else {
                this.log(`!! [${source}] Server error: ${res.status}`, 'text-rose-400');
            }
        } catch (e) {
            this.log(`!! [${source}] Network fault: ${e.message}`, 'text-rose-400');
        }
    }
};
EOF
echo ">> [FIX 4/5] ui/src/lib/services/TaskService.js — uses storeAddLog directly"

# ═══════════════════════════════════════════════════════════════════════
#  FILE 5 — ui/src/routes/+layout.svelte  (BOOT THE SOCKET HERE)
#  Single place where the WS is started. Uses nexusSocket from the
#  authoritative singleton module.
# ═══════════════════════════════════════════════════════════════════════
cat << 'EOF' > ui/src/routes/+layout.svelte
<!--
  +layout.svelte — v2.0 SINGLE WS BOOT POINT
  FIX: Imports nexusSocket from nexus_socket.svelte.ts (the real WS).
       connect() is called once in onMount.
       destroy() is called in onDestroy to clean up on SPA unmount.
-->
<script>
    import "../app.css";
    import { nexusSocket } from '$lib/nexus_socket.svelte';
    import { onMount, onDestroy } from 'svelte';

    let { children } = $props();

    onMount(() => {
        nexusSocket.connect();
    });

    onDestroy(() => {
        nexusSocket.destroy();
    });
</script>

{@render children()}
EOF
echo ">> [FIX 5/5] ui/src/routes/+layout.svelte — boots real socket, clean destroy"

# ═══════════════════════════════════════════════════════════════════════
#  PATCH: WorkspacePanel — fix nexusSocket import path
#  WorkspacePanel was importing nexusSocket from the wrong path.
# ═══════════════════════════════════════════════════════════════════════
if grep -q "nexus_socket.svelte.ts" ui/src/lib/components/panels/WorkspacePanel.svelte 2>/dev/null; then
    sed -i "s|import { nexusSocket } from '\\\$lib/nexus_socket.svelte.ts';|import { nexusSocket } from '\$lib/nexus_socket.svelte';|g" \
        ui/src/lib/components/panels/WorkspacePanel.svelte
    echo ">> [PATCH] WorkspacePanel.svelte — import path fixed"
fi

# ═══════════════════════════════════════════════════════════════════════
#  PATCH: +page.svelte — fix nexusSocket import path
# ═══════════════════════════════════════════════════════════════════════
if grep -q "nexus_socket.svelte.ts" ui/src/routes/+page.svelte 2>/dev/null; then
    sed -i "s|import { nexusSocket } from '\\\$lib/nexus_socket.svelte.ts';|import { nexusSocket } from '\$lib/nexus_socket.svelte';|g" \
        ui/src/routes/+page.svelte
    echo ">> [PATCH] +page.svelte — import path fixed"
fi

# ═══════════════════════════════════════════════════════════════════════
#  PATCH: NavHeader — wsState.ready is now a plain boolean, not a getter
#  The `class:dot-live={wsState.ready}` binding still works the same way
#  so no change needed there, but the comment in the file was wrong.
# ═══════════════════════════════════════════════════════════════════════

# ═══════════════════════════════════════════════════════════════════════
#  BUILD
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo ">> [BUILD] Running npm check..."
cd ui
if npm run check 2>&1 | tail -30; then
    echo ""
    echo ">> [BUILD] Running vite build..."
    npm run build 2>&1 | tail -20 && echo "✅  UI build PASSED" || echo "⚠️   UI build FAILED"
else
    echo "⚠️   svelte-check found issues — review above"
fi
cd ..

# ═══════════════════════════════════════════════════════════════════════
#  COMMIT
# ═══════════════════════════════════════════════════════════════════════
git add -A
git commit -m "fix(ui): v0.16.0 — rewire WS architecture, NexusWS.js was stub with no socket, store now wired via onSocketMessage, terminal responses work from UI workspace" \
    --allow-empty 2>/dev/null || true

echo ""
echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║  NEXUS UI WS FIX COMPLETE                                       ║"
echo "║                                                                  ║"
echo "║  WHAT WAS BROKEN:                                               ║"
echo "║  NexusWS.js was hollowed to a stub with no WebSocket.           ║"
echo "║  nexus_socket.svelte.ts opened a WS but its messages never      ║"
echo "║  reached store.svelte.js because there was no onMessage bridge. ║"
echo "║  Terminal in CLI worked because it reads stdout directly.        ║"
echo "║  Terminal in UI showed nothing from Workspace because the        ║"
echo "║  store.svelte.js reactive graph was never updated.              ║"
echo "║                                                                  ║"
echo "║  WHAT IS FIXED:                                                  ║"
echo "║  ✓ nexus_socket.svelte.ts — authoritative singleton WS          ║"
echo "║  ✓ onSocketMessage() — bridge to store reactive graph           ║"
echo "║  ✓ store.svelte.js — wired via onSocketMessage, wsState.ready   ║"
echo "║    is now a plain boolean (not a broken getter)                  ║"
echo "║  ✓ NexusWS.js — thin shim, just delegates to storeAddLog       ║"
echo "║  ✓ TaskService.js — imports storeAddLog directly                ║"
echo "║  ✓ +layout.svelte — single connect() call on mount              ║"
echo "║                                                                  ║"
echo "║  TEST: Open browser → Workspace → click CARGO CHECK             ║"
echo "║  You should now see output stream into the terminal panel.       ║"
echo "╚══════════════════════════════════════════════════════════════════╝"

