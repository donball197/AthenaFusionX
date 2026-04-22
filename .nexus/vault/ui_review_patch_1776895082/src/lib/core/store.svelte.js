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
