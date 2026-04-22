// ui/src/lib/core/store.svelte.js — v3.6 Aligned Reactive Store
import { nexusWS } from './NexusWS';

export const layout = $state({
    sidebarOpen: true, explorerVisible: true, ghostVisible: true, 
    ghostCortexVisible: true, activeTab: 'WORKSPACE', showVitals: true
});

export const wsState = $state({
    status: 'DISCONNECTED',
    get ready() { return this.status === 'CONNECTED'; }
});

export const systemVitals = $state({
    cpu_load: 0, ram_gb: 0, total_ram_gb: 4, temperature: 0, resonance: 0
});

export const ide = $state({
    activeTab: null, openTabs: [],
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

export const terminalState = $state({ 
    logs: [], 
    buffers: { GHOST_CORTEX: '' }, 
    isGhostTyping: false 
});

export const fileTreeTrigger = $state({ value: 0 });
export const neuralInventory = $state({ models: [{name: 'Gemma 4', tier: 'PRO', task: 'REASONING'}] });
export const vmoTelemetry = $state({ active_vmos: 0 });

// ─── HANDLER REGISTRATION ────────────────────────────────────────
nexusWS.registerLogHandler((message, cls) => {
    terminalState.logs.push({ timestamp: new Date().toLocaleTimeString(), message, cls });
    if (terminalState.logs.length > 200) terminalState.logs.shift();
});

nexusWS.onMessage((msg) => {
    wsState.status = 'CONNECTED';

    if (msg.type === 'NEURAL_STREAM') {
        // ⚡ FIX: Actively append streaming payload tokens to the Cortex buffer
        terminalState.buffers.GHOST_CORTEX += msg.payload;
    } else if (msg.type === 'NEURAL_THOUGHT') {
        // Log thoughts to the system terminal in blue
        nexusWS.logHandlers.forEach(h => h(msg.payload, 'text-blue-400'));
    } else if (msg.type === 'TASK_STATUS') {
        // Log status changes
        nexusWS.logHandlers.forEach(h => h(`>> [FSM] ${msg.payload}`, 'text-emerald-400'));
    } else if (msg.type_id === 'HEARTBEAT') {
        systemVitals.cpu_load = msg.cpu_load;
        systemVitals.temperature = msg.temperature;
        systemVitals.ram_gb = msg.ram_gb;
        vmoTelemetry.active_vmos = msg.active_vmos;
    }
});
