import { writable } from 'svelte/store';

// Array of open files: { path: string, content: string, isDirty: boolean }
export const openTabs = writable([]);

// String path of the active tab
export const activeTab = writable(null);

// Trigger to refresh file tree (increment to force reload)
export const fileTreeTrigger = writable(0);
