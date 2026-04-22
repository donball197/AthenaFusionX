// ui/src/lib/core/store.svelte.js — v4.1
// FIX: GHOST_CORTEX buffer capped at 8KB to prevent terminal lag.
// FIX: storeAddLog exported so TaskService.js import resolves.
// FIX: wsState.ready updated on both connect and disconnect events.
import { onSocketMessage } from '$lib/nexus_socket.svelte';

// ── Layout ─────────────────────────────────────────────────────────────
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

// ── WS state ────────────────────────────────────────────────────────────
export const wsState = $state({ status: 'DISCONNECTED', ready: false });

// ── System vitals ───────────────────────────────────────────────────────
export const systemVitals = $state({
    cpu_load:     0,
    ram_gb:       0,
    total_ram_gb: 4,
    temperature:  0,
    resonance:    0,
});

// ── IDE ─────────────────────────────────────────────────────────────────
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

// ── Terminal ─────────────────────────────────────────────────────────────
const GHOST_CAP = 8 * 1024; // 8KB max before we trim the oldest half

export const terminalState = $state({
    logs:          [],
    buffers:       { GHOST_CORTEX: '' },
    isGhostTyping: false,
});

function _addLog(message, cls = 'text-gray-400') {
    terminalState.logs = [
        ...terminalState.logs.slice(-199),
        { timestamp: new Date().toLocaleTimeString(), message, cls },
    ];
}

// FIX: exported so TaskService.js and NexusWS.js shim can import it
export function storeAddLog(message, cls = 'text-gray-400') {
    _addLog(message, cls);
}

export const fileTreeTrigger = $state({ value: 0 });
export const neuralInventory = $state({
    models: [{ name: 'Gemma 4', tier: 'PRO', task: 'REASONING' }],
});
export const vmoTelemetry = $state({ active_vmos: 0 });

// ── Socket → reactive state wiring ─────────────────────────────────────
onSocketMessage((msg) => {
    wsState.status = 'CONNECTED';
    wsState.ready  = true;

    switch (msg.type) {
        case 'NEURAL_STREAM': {
            const token = msg.payload ?? '';
            // FIX: cap buffer at GHOST_CAP to prevent memory growth
            const next = terminalState.buffers.GHOST_CORTEX + token;
            terminalState.buffers.GHOST_CORTEX = next.length > GHOST_CAP
                ? next.slice(next.length - GHOST_CAP)
                : next;
            terminalState.isGhostTyping = true;
            _addLog(token, 'text-gray-400');
            break;
        }
        case 'NEURAL_THOUGHT':
            _addLog(msg.payload ?? '', 'text-blue-400');
            break;

        case 'TASK_STATUS': {
            const s = String(msg.payload ?? '');
            _addLog(`>> [FSM] ${s}`, 'text-emerald-400');
            if (s.includes('COMPLETE') || s.includes('HALTED')) {
                terminalState.isGhostTyping = false;
            }
            break;
        }
        case 'LOG_EVENT':
            _addLog(msg.payload ?? '', msg.cls ?? 'text-gray-400');
            break;

        default:
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

// Mark WS offline when socket disconnects (fired by nexus_socket.svelte.ts)
onSocketMessage((msg) => {
    if (msg.__disconnect) {
        wsState.status = 'DISCONNECTED';
        wsState.ready  = false;
        terminalState.isGhostTyping = false;
    }
});
