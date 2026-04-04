import { writable, derived } from 'svelte/store';

// Navigation & Tab State
export const activeWorkspace = writable('forge'); 
export const openTabs = writable([{ path: 'src/main.rs', content: '// NEXUS READY\n', isDirty: false }]);
export const activeTab = writable('src/main.rs');
export const activeTabObj = derived([openTabs, activeTab], ([$tabs, $path]) => {
    return $tabs.find(t => t.path === $path) || $tabs[0];
});

// Telemetry & Hardware Vitals
export const systemVitals = writable({ 
    cpu_usage: 0, 
    ram_usage: '0/4GB', 
    temperature: 0, 
    status: 'OFFLINE',
    resonance: 1.0 
});

// AI Streaming & Ghosting
export const ghostBuffer = writable('');
export const isGhostTyping = writable(false);
export const ghostThinking = writable(false);

// Global OS State Triggers
export const terminalLogs = writable([]);
export const fileTreeTrigger = writable(0); // <-- THE MISSING SUTURE
export const wsReady = writable(false);     // <-- REQUIRED BY NEXUSWS
export const pendingPatch = writable(null); // <-- REQUIRED BY NEXUSWS

// Layout Config
export const layout = writable({
    sidebarVisible: true,
    ghostPanelVisible: true,
    visionActive: false
});
