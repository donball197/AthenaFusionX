// ui/src/lib/core/store — v2.2 [AUTHORITATIVE]
import { system } from '../stores/system.svelte';
import { terminal, addLog, clearGhost } from '../stores/terminal.svelte';
import { ui } from '$lib/stores/ui.svelte';
import { ide as ideStore } from '$lib/stores/ide.svelte';

export const systemVitals = system;
export const terminalState = terminal;
export const layout = ui;
export const ide = ideStore;
export { addLog as storeAddLog, clearGhost };

export const wsState = { get ready() { return system.online; } };

// Reactive VMO counts
export const vmoTelemetry = $state({ active_vmos: 0 });
export function setActiveVmos(n: number) { vmoTelemetry.active_vmos = n; }

export const fileTreeTrigger = $state({ value: 0 });
