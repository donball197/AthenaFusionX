// ════════════════════════════════════════════════════════════════
//  NEXUS SINGULARITY — Reactive Store v6.1.0 (Dynamic Hardware)
// ════════════════════════════════════════════════════════════════

export const wsState = $state({ ready: false, latency: 0 });

export const systemVitals = $state({
    cpu_load:    0,
    ram_gb:      0,
    total_ram_gb: 4.0, // Dynamic baseline
    temperature: 0,
    resonance:   0,
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

export const layout = $state({ explorerVisible: true, ghostVisible: true });
export const fileTreeTrigger = $state({ value: 0 });
