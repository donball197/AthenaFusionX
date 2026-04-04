import { writable, derived } from 'svelte/store';

// Core Navigation & Tabs
export const openTabs = writable([{ path: 'src/main.rs', content: '// NEXUS_READY\n', isDirty: false }]);
export const activeTab = writable('src/main.rs');
export const activeTabObj = derived([openTabs, activeTab], ([$tabs, $path]) => $tabs.find(t => t.path === $path) ?? null);

// Workspace Layout
export const fileTree = writable([]);
export const fileTreeTrigger = writable(0);
export const layout = writable({ 
    sidebarVisible: true, 
    ghostPanelVisible: true, 
    visionActive: false, 
    editorWidth: 0.6, 
    terminalHeight: 0.3 
});

// AI & Task State
export const isGhostTyping = writable(false);
export const ghostBuffer = writable('');
export const ghostThinking = writable(false);
export const currentMission = writable(''); // Tracks the active Zenith Directive

// Hardware & System
export const terminalLogs = writable([]);
export const pendingPatch = writable(null);
export const systemVitals = writable({ 
    status: 'OFFLINE', 
    cpu_usage: 0, 
    ram_usage: '0/0', 
    resonance: 1.0, 
    temperature: 0 
});
export const wsReady = writable(false);
