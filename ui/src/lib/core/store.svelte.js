// ui/src/lib/core/store.svelte.js — v3.4 Decoupled Reactive Store
import { nexusWS } from './NexusWS';

// ─── STATE OBJECTS ──────────────────────────────────────────────
export const layout = $state({
    sidebarOpen: true,
    explorerVisible: true,
    ghostVisible: true,
    ghostCortexVisible: true,
    activeTab: 'WORKSPACE',
    showVitals: true
});

export const wsState = $state({
    status: 'DISCONNECTED',
    get ready() { return this.status === 'CONNECTED'; }
});

export const systemVitals = $state({
    cpu_load: 0,
    ram_gb: 0,
    total_ram_gb: 4,
    temperature: 0,
    load: 0,
    resonance: 0
});

export const terminalState = $state({
    logs: [],
    buffers: { GHOST_CORTEX: '' },
    isGhostTyping: false
});

export const ide = $state({
    activeTab: null,
    openTabs: [],
    mountFile: (path, content) => {
        if (!ide.openTabs.find(t => t.path === path)) {
            ide.openTabs.push({ path, content, isDirty: false });
        }
        ide.activeTab = path;
    },
    closeTab: (path) => {
        ide.openTabs = ide.openTabs.filter(t => t.path !== path);
        if (ide.activeTab === path) ide.activeTab = ide.openTabs[0]?.path || null;
    },
    updateContent: (path, content) => {
        const tab = ide.openTabs.find(t => t.path === path);
        if (tab) { tab.content = content; tab.isDirty = true; }
    },
    markSaved: (path) => {
        const tab = ide.openTabs.find(t => t.path === path);
        if (tab) tab.isDirty = false;
    }
});

export const neuralInventory = $state({ 
    models: [{name: 'Gemma 4-31b', tier: 'SOVEREIGN', task: 'REASONING'}] 
});
export const vmoTelemetry = $state({ active_vmos: 0 });
export const fileTreeTrigger = $state({ value: 0 });

// ─── LOG RELAY HOOK ─────────────────────────────────────────────
// This registers our state-mutating function into the socket relay
// without requiring NexusWS.js to import us.
nexusWS.registerLogHandler((message, cls) => {
    terminalState.logs.push({
        timestamp: new Date().toLocaleTimeString(),
        message,
        cls
    });
    if (terminalState.logs.length > 100) terminalState.logs.shift();
});

// ─── SOVEREIGN FSM LOGIC ─────────────────────────────────────────
function createNexusStore() {
    let fsmState = $state('IDLE'); 
    let alignment = $state({ alpha: 0, beta: 0, gamma: 0 });

    nexusWS.onMessage((msg) => {
        wsState.status = 'CONNECTED';

        if (msg.type === 'NEURAL_THOUGHT') {
            terminalState.buffers.GHOST_CORTEX = msg.payload;
        } else if (msg.type === 'NEURAL_STREAM') {
            // Use the internal relay logic
            const cls = msg.payload.includes('ERROR') ? 'text-rose-400' : '';
            // Manually push to trigger the same logic as pushTermLine
            terminalState.logs.push({
                timestamp: new Date().toLocaleTimeString(),
                message: msg.payload,
                cls
            });
            if (terminalState.logs.length > 100) terminalState.logs.shift();

            if (msg.payload.includes('EXECUTOR Turn')) fsmState = 'EXECUTING';
            if (msg.payload.includes('SEQUENCE_HALTED')) fsmState = 'COMPLETE';
            if (msg.payload.includes('ERROR') || msg.payload.includes('FAULT')) fsmState = 'FAULT';
        } else if (msg.type === 'VITALS_PULSE') {
            systemVitals.cpu_load = msg.payload.cpu || 0;
            systemVitals.temperature = msg.payload.temp || 0;
            systemVitals.ram_gb = msg.payload.ram || 0;
        }
    });

    return {
        get fsmState() { return fsmState; },
        get alignment() { return alignment; }
    };
}

export const nexusStore = createNexusStore();
