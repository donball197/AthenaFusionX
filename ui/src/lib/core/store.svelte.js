// ════════════════════════════════════════════════════════════════
//  NEXUS SINGULARITY — Commercial Reactive Store v7.0.0
// ════════════════════════════════════════════════════════════════

export const wsState = $state({ ready: false, latency: 0 });

export const systemVitals = $state({
    cpu_load:     0,
    ram_gb:       0,
    total_ram_gb: 4.0,
    temperature:  0,
    resonance:    0,
});

export const neuralInventory = $state({
    // ⚡ SEATED FROM SCOUT_MODELS.SH
    models: [
        { id: 'gemini-2.5-pro', name: 'ORCHESTRATOR_PRO', limit: '1M', tier: 'ELITE', task: 'Deep Architecture' },
        { id: 'gemma-4-31b-it', name: 'SOVEREIGN_DENSE', limit: '256K', tier: 'INDUSTRIAL', task: 'Kernel Surgery' },
        { id: 'gemma-4-26b-a4b-it', name: 'SOVEREIGN_MOE', limit: '256K', tier: 'PRODUCTION', task: 'Logic/Coding' },
        { id: 'gemini-2.0-flash-lite', name: 'COURIER_LITE', limit: '1M', tier: 'UTILITY', task: 'Rapid Scraping' },
        { id: 'gemini-robotics-er-1.5', name: 'ROBOTICS_CORE', limit: '1M', tier: 'HARDWARE', task: 'ROS/Motion Control' }
    ]
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

export const layout = $state({
    explorerVisible:    true,
    ghostVisible:       true,
    ghostCortexVisible: false,
    visorVisible:       false,
    autopilotActive:    false,
});

export const fileTreeTrigger = $state({ value: 0 });

export const vmoTelemetry = $state({
    connected:   false,
    status:      'DISCONNECTED',
    active_vmos: 0,
    next_handle: '0x00',
    resonance:   0.0,
    logs:        [],
});
