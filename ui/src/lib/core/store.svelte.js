// ui/src/lib/core/store.svelte.js — v3.7 Aligned Reactive Store
// FIX: wsState.ready was a getter method on $state — Svelte 5 proxies
//      do not support getter methods reactively. Replaced with $derived.
// FIX: log buffer now uses slice(-180) instead of shift() to avoid
//      triggering a full 200-node DOM re-render on every new message.
import { nexusWS } from './NexusWS';

export const layout = $state({
    sidebarOpen:         true,
    explorerVisible:     true,
    ghostVisible:        true,
    ghostCortexVisible:  true,
    activeTab:           'WORKSPACE',
    showVitals:          true,
    autopilotActive:     false,
    visorVisible:        false,
});

// FIX: removed getter method — use exported $derived instead
export const wsState  = $state({ status: 'DISCONNECTED' });
export const wsReady  = $derived(wsState.status === 'CONNECTED');

export const systemVitals = $state({
    cpu_load:     0,
    ram_gb:       0,
    total_ram_gb: 4,
    temperature:  0,
    resonance:    0,
});

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
        ide.openTabs   = ide.openTabs.filter(t => t.path !== path);
        if (ide.activeTab === path) {
            ide.activeTab = ide.openTabs[0]?.path || null;
        }
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

export const terminalState = $state({
    logs:          [],
    buffers:       { GHOST_CORTEX: '' },
    isGhostTyping: false,
});

export const fileTreeTrigger = $state({ value: 0 });

export const neuralInventory = $state({
    models: [{ name: 'Gemma 4', tier: 'PRO', task: 'REASONING' }],
});

export const vmoTelemetry = $state({ active_vmos: 0 });

// ─── HANDLER REGISTRATION ────────────────────────────────────────────
nexusWS.registerLogHandler((message, cls) => {
    // FIX: slice(-180) instead of shift() — avoids full DOM re-render
    terminalState.logs = [
        ...terminalState.logs.slice(-179),
        { timestamp: new Date().toLocaleTimeString(), message, cls },
    ];
});

nexusWS.onMessage((msg) => {
    wsState.status = 'CONNECTED';

    switch (msg.type) {
        case 'NEURAL_STREAM':
            terminalState.buffers.GHOST_CORTEX += msg.payload ?? '';
            terminalState.isGhostTyping = true;
            break;

        case 'NEURAL_THOUGHT':
            nexusWS.logHandlers.forEach(
                h => h(msg.payload, 'text-blue-400')
            );
            break;

        case 'TASK_STATUS':
            nexusWS.logHandlers.forEach(
                h => h(`>> [FSM] ${msg.payload}`, 'text-emerald-400')
            );
            // When a task completes, stop the ghost typing indicator
            if (String(msg.payload).includes('COMPLETE') ||
                String(msg.payload).includes('HALTED')) {
                terminalState.isGhostTyping = false;
            }
            break;

        default:
            // HEARTBEAT uses type_id not type
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
