export const terminalState = $state({
    logs: [],
    isGhostTyping: false,
    ghostBuffer: ""
});

export const systemVitals = $state({
    cpu_load: 0.15,
    ram_gb: 0.85,
    temperature: 45.0,
    resonance: 0.98
});

export const wsState = $state({ ready: false });
export const fileTreeTrigger = $state({ value: 0 });

export const layout = $state({
    visionActive: false,
    sidebarVisible: true,
    visorVisible: true
});

export const pendingPatch = $state({
    active: false,
    path: "",
    content: ""
});

export const ide = $state({
    activeTab: null,
    openTabs: [],
    mountFile: (path, content) => {
        const existing = ide.openTabs.find(t => t.path === path);
        if (!existing) {
            ide.openTabs.push({ path, content, isDirty: false });
        }
        ide.activeTab = path;
    }
});
